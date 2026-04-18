#!/usr/bin/env python3
"""Prove the full human -> Paw -> SRE -> Developer -> report loop."""

from __future__ import annotations

import argparse
import json
import os
import time
from typing import Any

from temperpaw_proof_support import (
    DEFAULT_BASE_URL,
    DEFAULT_MODEL,
    DEFAULT_REPO_URL,
    DEFAULT_TENANT,
    ODataClient,
    ReplyCollector,
    derive_local_sandbox_url,
    entity_id,
    nested_str,
    require,
    suffix,
    webhook_post,
)


def wait_for_setup(
    client: ODataClient,
    *,
    repo_url: str,
    timeout_secs: float,
) -> tuple[dict[str, Any], dict[str, Any] | None, dict[str, Any] | None]:
    deadline = time.time() + timeout_secs
    while time.time() < deadline:
        harnesses = client.list(
            "Harnesses",
            filter_expr=f"repo_url eq '{repo_url}'",
            orderby="sequence_nr desc",
            top=5,
        )
        harness = next(iter(harnesses), None)
        monitors = client.list("Monitors", orderby="sequence_nr desc", top=50)
        latest_monitor = next(iter(monitors), None)
        monitor_scans = client.list("MonitorScans", orderby="sequence_nr desc", top=20)
        latest_scan = next(iter(monitor_scans), None)
        if harness and (latest_monitor or latest_scan):
            return harness, latest_monitor, latest_scan
        time.sleep(2)
    raise TimeoutError("timed out waiting for Paw to set up the harness and monitoring state")


def wait_for_alert_cycle_terminal(
    client: ODataClient,
    alert_cycle_id: str,
    timeout_secs: float,
) -> dict[str, Any]:
    deadline = time.time() + timeout_secs
    while time.time() < deadline:
        alert_cycle = client.get("AlertCycles", alert_cycle_id)
        status = nested_str(alert_cycle, ["Status", "status"])
        if status in {"Fixed", "Resolved", "Tuned", "Failed"}:
            return alert_cycle
        time.sleep(5)
    raise TimeoutError(
        f"timed out waiting for AlertCycle {alert_cycle_id} to reach a terminal status"
    )


def find_latest_work_cycle(client: ODataClient, project_harness_id: str) -> dict[str, Any] | None:
    work_cycles = client.list(
        "WorkCycles",
        filter_expr=f"project_harness_id eq '{project_harness_id}'",
        orderby="sequence_nr desc",
        top=5,
    )
    return next(iter(work_cycles), None)


def find_issue_for_alert(
    client: ODataClient,
    *,
    alert_cycle_id: str,
    monitor_id: str,
) -> dict[str, Any] | None:
    issues = client.list("Issues", orderby="sequence_nr desc", top=25)
    for issue in issues:
        description = nested_str(issue, ["Description", "description"])
        if alert_cycle_id in description or monitor_id in description:
            return issue
    return None


def wait_for_thread_reply(
    collector: ReplyCollector,
    *,
    thread_id: str,
    timeout_secs: float,
    expect_substring: str | None = None,
    exclude_agent_id: str | None = None,
) -> dict[str, Any]:
    deadline = time.time() + timeout_secs
    while time.time() < deadline:
        payload = collector.wait_for_reply(
            thread_id,
            timeout_secs=max(deadline - time.time(), 0.1),
        )
        content = str(payload.get("content") or "")
        agent_entity_id = str(payload.get("agent_entity_id") or "")
        if exclude_agent_id and agent_entity_id == exclude_agent_id:
            continue
        if expect_substring and expect_substring not in content:
            continue
        return payload
    raise TimeoutError(f"timed out waiting for reply on thread {thread_id}")


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--base-url", default=os.getenv("TEMPERPAW_BASE_URL", DEFAULT_BASE_URL))
    parser.add_argument("--tenant", default=os.getenv("PAW_TENANT", DEFAULT_TENANT))
    parser.add_argument("--model", default=DEFAULT_MODEL)
    parser.add_argument("--repo-url", default=DEFAULT_REPO_URL)
    parser.add_argument("--sandbox-url", default=os.getenv("TEMPERPAW_SANDBOX_URL"))
    parser.add_argument("--secret", default=os.getenv("WEBHOOK_SECRET"))
    parser.add_argument("--setup-timeout-secs", type=float, default=360.0)
    parser.add_argument("--alert-timeout-secs", type=float, default=20 * 60.0)
    args = parser.parse_args()

    requested_sandbox_url = args.sandbox_url or ""
    client = ODataClient(
        base_url=args.base_url,
        tenant=args.tenant,
        api_key=os.getenv("TEMPER_API_KEY") or None,
    )

    collector = ReplyCollector()
    collector.start()
    channel_name = f"paw-full-loop-{suffix()}"
    thread_id = f"thread-{suffix()}"
    try:
        channel = client.create("Channels")
        channel_id = entity_id(channel)
        require(channel_id, "failed to create Channel")
        client.action(
            "Channels",
            channel_id,
            "Paw.Channel.Configure",
            {
                "channel_type": "webhook-proof",
                "channel_id": channel_name,
                "guild_id": "",
                "default_agent_config": "",
                "webhook_secret": "",
                "webhook_url": collector.url,
            },
        )
        client.action("Channels", channel_id, "Paw.Channel.Connect", {})
        client.wait_for_status("Channels", channel_id, "Connected", timeout_secs=15.0)

        route = client.create("AgentRoutes")
        route_id = entity_id(route)
        require(route_id, "failed to create AgentRoute")
        route_config = {
            "model": args.model,
            "provider": "anthropic",
            "tools_enabled": "temper_create,temper_get,temper_list,temper_action,temper_read,save_memory",
            "max_turns": "24",
            "workdir": "/tmp/paw-full-platform-proof",
            "temper_api_url": args.base_url.rstrip("/"),
            "max_follow_ups": "8",
        }
        if requested_sandbox_url:
            route_config["sandbox_url"] = requested_sandbox_url
        client.action(
            "AgentRoutes",
            route_id,
            "Paw.Channel.Register",
            {
                "binding_tier": "channel",
                "channel_id": channel_name,
                "guild_id": "",
                "match_pattern": "",
                "agent_config": json.dumps(route_config, separators=(",", ":")),
                "soul_id": "Paw",
            },
        )

        human_message = (
            "Manage deep-sci-fi for me. For this proof run, do only the minimal managed-project "
            "setup: create or reuse the harness and monitoring metadata, then reply once setup is "
            "ready. Do not start an exploratory developer investigation before alerts arrive. "
            f"The repo is {args.repo_url}."
        )
        client.action(
            "Channels",
            channel_id,
            "Paw.Channel.ReceiveMessage",
            {
                "message_id": f"msg-{suffix()}",
                "author_id": "proof-user",
                "thread_id": thread_id,
                "content": human_message,
            },
        )

        paw_reply = wait_for_thread_reply(
            collector,
            thread_id=thread_id,
            timeout_secs=args.setup_timeout_secs,
        )
        paw_agent_id = str(paw_reply.get("agent_entity_id") or "")
        require(paw_agent_id, f"missing Paw agent id in reply: {paw_reply}")

        harness, paw_monitor, paw_monitor_scan = wait_for_setup(
            client,
            repo_url=args.repo_url,
            timeout_secs=args.setup_timeout_secs,
        )
        paw_agent = client.get("Sessions", paw_agent_id)
        harness_id = entity_id(harness)
        require(harness_id, "missing Harness id")

        external_monitor_id = f"dd-proof-{suffix()}"
        alert_body = {
            "org": {"id": 424242},
            "id": external_monitor_id,
            "monitor": {"id": external_monitor_id, "name": "deep-sci-fi platform npm ci"},
            "title": "deep-sci-fi platform clean install is broken",
            "text": "npm ci exits with package-lock drift in platform/",
            "priority": "2",
            "alert_type": "error",
            "alert_transition": "Triggered",
            "dd_query": "sum(last_5m):avg:ci.install.failures{service:deep-sci-fi-platform} > 0",
            "project_harness_id": harness_id,
            "repo_url": args.repo_url,
            "reply_channel_entity_id": channel_id,
            "reply_thread_id": thread_id,
            "reproduction": {
                "cwd": "platform",
                "command": "npm ci",
                "failure": "package.json and package-lock.json are out of sync",
            },
            "evidence": {
                "missing_packages": [
                    "@types/d3",
                    "d3",
                    "react-markdown",
                    "remark-gfm",
                ],
            },
        }
        if requested_sandbox_url:
            alert_body["developer_sandbox_url"] = requested_sandbox_url
        status_code, webhook_response = webhook_post(
            args.base_url,
            alert_body,
            secret=args.secret,
        )
        require(
            status_code == 200,
            f"unexpected webhook response {status_code}: {webhook_response}",
        )
        alert_cycle_id = str(webhook_response.get("alert_cycle_id") or "")
        sre_agent_id = str(webhook_response.get("sre_agent_id") or "")
        monitor_id = str(webhook_response.get("monitor_id") or "")
        require(alert_cycle_id, f"missing alert_cycle_id in {webhook_response}")
        require(sre_agent_id, f"missing sre_agent_id in {webhook_response}")
        require(monitor_id, f"missing monitor_id in {webhook_response}")

        sre_wait = client.wait_for_agent(sre_agent_id, int(args.alert_timeout_secs * 1000))
        sre_agent = client.get("Sessions", sre_agent_id)
        alert_cycle = wait_for_alert_cycle_terminal(
            client,
            alert_cycle_id,
            args.alert_timeout_secs,
        )
        work_cycle = find_latest_work_cycle(client, harness_id)
        issue = find_issue_for_alert(
            client,
            alert_cycle_id=alert_cycle_id,
            monitor_id=monitor_id,
        )
        sre_child_agents = client.list(
            "Sessions",
            filter_expr=f"parent_session_id eq '{sre_agent_id}'",
            orderby="sequence_nr desc",
            top=10,
        )

        proactive_reply = wait_for_thread_reply(
            collector,
            thread_id=thread_id,
            timeout_secs=min(args.alert_timeout_secs, 600.0),
            expect_substring="Temper Paw self-heal update",
            exclude_agent_id=paw_agent_id,
        )

        paw_sandbox_url = nested_str(paw_agent, ["sandbox_url", "SandboxUrl"])
        sre_sandbox_url = nested_str(sre_agent, ["sandbox_url", "SandboxUrl"])
        child_sandbox_urls = [
            nested_str(agent, ["sandbox_url", "SandboxUrl"])
            for agent in sre_child_agents
            if nested_str(agent, ["sandbox_url", "SandboxUrl"])
        ]
        actual_sandbox_urls = [url for url in [paw_sandbox_url, sre_sandbox_url, *child_sandbox_urls] if url]
        require(actual_sandbox_urls, "expected at least one agent sandbox URL in the full-loop proof")
        require(
            any("modal.host" in url for url in actual_sandbox_urls),
            f"expected the full-loop proof to run on Modal, got sandbox URLs: {actual_sandbox_urls}",
        )

        summary = {
            "base_url": args.base_url,
            "requested_sandbox_url": requested_sandbox_url,
            "actual_sandbox_urls": actual_sandbox_urls,
            "human_message": human_message,
            "channel_id": channel_id,
            "route_id": route_id,
            "thread_id": thread_id,
            "paw_agent_id": paw_agent_id,
            "paw_agent": paw_agent,
            "paw_reply": paw_reply,
            "project_harness": harness,
            "paw_monitor": paw_monitor,
            "paw_monitor_scan": paw_monitor_scan,
            "webhook_response": webhook_response,
            "sre_agent": sre_agent,
            "sre_wait": sre_wait,
            "alert_cycle": alert_cycle,
            "work_cycle": work_cycle,
            "issue": issue,
            "sre_child_agents": sre_child_agents,
            "proactive_reply": proactive_reply,
        }
        print(json.dumps(summary, indent=2))
        return 0
    finally:
        collector.close()


if __name__ == "__main__":
    raise SystemExit(main())
