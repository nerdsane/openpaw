#!/usr/bin/env python3
"""Prove webhook ingestion automatically spawns an SRE agent.

This proof focuses on the Temper-native architecture checkpoint required by the
audit: WebhookEvent -> validate -> route -> process -> AlertCycle.Open ->
alert_opener creates/configures/provisions an SRE agent.

It intentionally does not require the downstream remediation loop to reach a
terminal AlertCycle outcome, because that depends on external sandbox and model
providers rather than the orchestration changes under test.
"""

from __future__ import annotations

import argparse
import json
import os
import time

from openpaw_proof_support import (
    DEFAULT_BASE_URL,
    DEFAULT_REPO_URL,
    DEFAULT_TENANT,
    ODataClient,
    entity_id,
    nested_str,
    now_utc,
    register_webhook_route,
    require,
    suffix,
    webhook_post,
)


def wait_for_webhook_event_terminal(
    client: ODataClient,
    event_id: str,
    timeout_secs: float,
) -> dict[str, object]:
    deadline = time.time() + timeout_secs
    while time.time() < deadline:
        event = client.get("WebhookEvents", event_id)
        status = str(event.get("Status") or event.get("status") or "")
        if status in {"Processed", "Rejected"}:
            return event
        time.sleep(1)
    raise TimeoutError(f"timed out waiting for WebhookEvent {event_id} to reach terminal state")


def wait_for_sre_agent(
    client: ODataClient,
    alert_cycle_id: str,
    timeout_secs: float,
) -> tuple[dict[str, object], str]:
    deadline = time.time() + timeout_secs
    while time.time() < deadline:
        alert_cycle = client.get("AlertCycles", alert_cycle_id)
        sre_agent_id = nested_str(alert_cycle, ["sre_agent_id", "SreAgentId"])
        if sre_agent_id:
            return alert_cycle, sre_agent_id
        time.sleep(1)
    raise TimeoutError(f"timed out waiting for AlertCycle {alert_cycle_id} to record sre_agent_id")


def wait_for_agent_progress(
    client: ODataClient,
    agent_id: str,
    timeout_secs: float,
) -> dict[str, object]:
    deadline = time.time() + timeout_secs
    latest: dict[str, object] | None = None
    while time.time() < deadline:
        latest = client.get("Agents", agent_id)
        status = nested_str(latest, ["status", "Status"])
        if status and status != "Created":
            return latest
        time.sleep(1)
    if latest is None:
        raise TimeoutError(f"timed out waiting for Agent {agent_id} to appear")
    return latest


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--base-url", default=os.getenv("OPENPAW_BASE_URL", DEFAULT_BASE_URL))
    parser.add_argument("--tenant", default=os.getenv("PAW_TENANT", DEFAULT_TENANT))
    parser.add_argument("--repo-url", default=DEFAULT_REPO_URL)
    parser.add_argument("--secret", default=os.getenv("WEBHOOK_SECRET"))
    parser.add_argument("--timeout-ms", type=int, default=15 * 60 * 1000)
    args = parser.parse_args()

    client = ODataClient(
        base_url=args.base_url,
        tenant=args.tenant,
        api_key=os.getenv("TEMPER_API_KEY") or None,
    )
    run_suffix = suffix()
    route_key = f"datadog-sre-{run_suffix}"
    harness = client.create("Harnesses", {"Id": f"webhook-sre-harness-{run_suffix}"})
    harness_id = entity_id(harness)
    require(harness_id, "failed to create Harness")
    client.action(
        "Harnesses",
        harness_id,
        "OpenPaw.Harness.Configure",
        {
            "repo_url": args.repo_url,
            "tech_stack": "Next.js frontend, Python backend",
            "conventions": "Webhook -> SRE proof harness.",
        },
    )
    client.action(
        "Harnesses",
        harness_id,
        "OpenPaw.Harness.Activate",
        {"last_activated_at": now_utc()},
    )
    route = register_webhook_route(
        client,
        route_key=route_key,
        source_type="datadog",
        target_entity_type="AlertCycle",
        target_action="OpenPaw.Heal.Open",
        webhook_secret=args.secret or "",
        monitor_resolution_enabled=True,
        dedup_enabled=False,
    )
    route_id = entity_id(route)

    external_monitor_id = f"webhook-sre-monitor-{run_suffix}"
    alert_body = {
        "monitor_id": external_monitor_id,
        "project_harness_id": harness_id,
        "repo_url": args.repo_url,
        "severity": "high",
        "summary": "Webhook should auto-spawn SRE",
        "failure": "package-lock drift detected during webhook-to-sre proof",
    }
    status, response = webhook_post(
        args.base_url,
        alert_body,
        route_key=route_key,
        source_type="datadog",
        secret=args.secret,
    )
    require(status == 200, f"unexpected webhook response {status}: {response}")
    event_id = str(response.get("event_id") or "")
    require(event_id, f"missing event_id: {response}")

    webhook_event = wait_for_webhook_event_terminal(client, event_id, args.timeout_ms / 1000)
    require(
        str(webhook_event.get("Status") or webhook_event.get("status") or "") == "Processed",
        f"WebhookEvent did not process successfully: {webhook_event}",
    )
    alert_cycle_id = nested_str(webhook_event, ["target_entity_id", "TargetEntityId"])
    require(alert_cycle_id, f"WebhookEvent missing target_entity_id: {webhook_event}")

    alert_cycle_after_open, sre_agent_id = wait_for_sre_agent(client, alert_cycle_id, args.timeout_ms / 1000)
    monitor_id = nested_str(alert_cycle_after_open, ["monitor_id", "MonitorId"])
    require(monitor_id, f"AlertCycle missing monitor_id after webhook processing: {alert_cycle_after_open}")

    alert_cycle = client.get("AlertCycles", alert_cycle_id)
    require(
        nested_str(alert_cycle, ["status", "Status"]) == "Triaging",
        f"AlertCycle did not remain in Triaging after SRE spawn: {alert_cycle}",
    )
    sre_agent = wait_for_agent_progress(client, sre_agent_id, min(args.timeout_ms / 1000, 120))
    work_cycles = client.list(
        "WorkCycles",
        filter_expr=f"project_harness_id eq '{harness_id}'",
        orderby="sequence_nr desc",
        top=5,
    )
    child_agents = client.list(
        "Agents",
        filter_expr=f"parent_agent_id eq '{sre_agent_id}'",
        orderby="sequence_nr desc",
        top=5,
    )

    summary = {
        "project_harness_id": harness_id,
        "route_id": route_id,
        "route_key": route_key,
        "webhook_event_id": event_id,
        "webhook_event": webhook_event,
        "monitor_id": monitor_id,
        "alert_cycle_id": alert_cycle_id,
        "sre_agent_id": sre_agent_id,
        "sre_agent": sre_agent,
        "alert_cycle": alert_cycle,
        "work_cycles": work_cycles,
        "child_agents": child_agents,
    }
    print(json.dumps(summary, indent=2))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
