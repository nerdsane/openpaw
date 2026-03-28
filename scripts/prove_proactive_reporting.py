#!/usr/bin/env python3
"""Prove webhook-driven self-heal reports flow back through Channel replies."""

from __future__ import annotations

import argparse
import json
import os

from openpaw_proof_support import (
    DEFAULT_BASE_URL,
    DEFAULT_MODEL,
    DEFAULT_REPO_URL,
    DEFAULT_TENANT,
    ODataClient,
    ReplyCollector,
    entity_id,
    now_utc,
    require,
    suffix,
    webhook_post,
)


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--base-url", default=os.getenv("OPENPAW_BASE_URL", DEFAULT_BASE_URL))
    parser.add_argument("--tenant", default=os.getenv("PAW_TENANT", DEFAULT_TENANT))
    parser.add_argument("--repo-url", default=DEFAULT_REPO_URL)
    parser.add_argument("--model", default=DEFAULT_MODEL)
    parser.add_argument("--secret", default=os.getenv("WEBHOOK_SECRET"))
    parser.add_argument("--timeout-secs", type=float, default=20 * 60.0)
    args = parser.parse_args()

    client = ODataClient(
        base_url=args.base_url,
        tenant=args.tenant,
        api_key=os.getenv("TEMPER_API_KEY") or None,
    )
    collector = ReplyCollector()
    collector.start()

    try:
        run_suffix = suffix()
        channel_name = f"heal-report-{run_suffix}"
        thread_id = f"report-thread-{run_suffix}"
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
        client.action(
            "AgentRoutes",
            route_id,
            "Paw.Channel.Register",
            {
                "binding_tier": "channel",
                "channel_id": channel_name,
                "guild_id": "",
                "match_pattern": "",
                "agent_config": json.dumps(
                    {
                        "model": args.model,
                        "provider": "anthropic",
                        "tools_enabled": "temper_create,temper_get,temper_list,temper_action,spawn_agent,read_entity,save_memory",
                        "max_turns": "12",
                        "workdir": "/tmp/proactive-report-proof",
                    },
                    separators=(",", ":"),
                ),
                "soul_id": "Paw",
            },
        )

        harness = client.create("ProjectHarnesses", {"Id": f"report-harness-{run_suffix}"})
        harness_id = entity_id(harness)
        require(harness_id, "failed to create ProjectHarness")
        client.action(
            "ProjectHarnesses",
            harness_id,
            "OpenPaw.Harness.Configure",
            {
                "repo_url": args.repo_url,
                "tech_stack": "Next.js frontend, Python backend",
                "conventions": "Proactive reporting proof harness.",
            },
        )
        client.action(
            "ProjectHarnesses",
            harness_id,
            "OpenPaw.Harness.Activate",
            {"last_activated_at": now_utc()},
        )

        external_monitor_id = f"heal-report-monitor-{run_suffix}"
        body = {
            "source": "logfire",
            "event_type": "alert_fired",
            "payload": {
                "monitor_id": external_monitor_id,
                "project_harness_id": harness_id,
                "repo_url": args.repo_url,
                "severity": "high",
                "summary": "Proactive reporting proof alert",
                "reproduction": {
                    "command": "npm ci",
                    "failure": "package-lock drift detected during proactive reporting proof",
                },
                "reply_channel_entity_id": channel_id,
                "reply_thread_id": thread_id,
            },
        }
        status, response = webhook_post(args.base_url, body, secret=args.secret)
        require(status == 200, f"unexpected webhook response {status}: {response}")
        alert_cycle_id = str(response.get("alert_cycle_id") or "")
        require(alert_cycle_id, f"missing alert_cycle_id: {response}")

        reply = collector.wait_for_reply(thread_id, timeout_secs=args.timeout_secs)
        content = str(reply.get("content") or "")
        require("Open Paw self-heal update" in content, f"reply missing summary header: {reply}")
        require("AlertCycle:" in content, f"reply missing alert summary: {reply}")

        summary = {
            "channel_id": channel_id,
            "route_id": route_id,
            "alert_cycle_id": alert_cycle_id,
            "reply": reply,
        }
        print(json.dumps(summary, indent=2))
        return 0
    finally:
        collector.close()


if __name__ == "__main__":
    raise SystemExit(main())
