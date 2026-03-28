#!/usr/bin/env python3
"""Prove webhook ingestion automatically spawns Scout and drives triage."""

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
    now_utc,
    require,
    suffix,
    webhook_post,
)


def wait_for_alert_cycle_terminal(
    client: ODataClient,
    alert_cycle_id: str,
    timeout_secs: float,
) -> dict[str, object]:
    deadline = time.time() + timeout_secs
    while time.time() < deadline:
        alert_cycle = client.get("AlertCycles", alert_cycle_id)
        status = str(alert_cycle.get("Status") or alert_cycle.get("status") or "")
        if status in {"Fixed", "Tuned", "Failed"}:
            return alert_cycle
        time.sleep(2)
    raise TimeoutError(f"timed out waiting for AlertCycle {alert_cycle_id} to reach terminal state")


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
    harness = client.create("ProjectHarnesses", {"Id": f"webhook-scout-harness-{run_suffix}"})
    harness_id = entity_id(harness)
    require(harness_id, "failed to create ProjectHarness")
    client.action(
        "ProjectHarnesses",
        harness_id,
        "OpenPaw.Harness.Configure",
        {
            "repo_url": args.repo_url,
            "tech_stack": "Next.js frontend, Python backend",
            "conventions": "Webhook -> Scout proof harness.",
        },
    )
    client.action(
        "ProjectHarnesses",
        harness_id,
        "OpenPaw.Harness.Activate",
        {"last_activated_at": now_utc()},
    )

    external_monitor_id = f"webhook-scout-monitor-{run_suffix}"
    alert_body = {
        "source": "logfire",
        "event_type": "alert_fired",
        "payload": {
            "monitor_id": external_monitor_id,
            "project_harness_id": harness_id,
            "repo_url": args.repo_url,
            "severity": "high",
            "summary": "Webhook should auto-spawn Scout",
            "reproduction": {
                "command": "npm ci",
                "failure": "package-lock drift detected during webhook-to-scout proof",
            },
        },
    }
    status, response = webhook_post(args.base_url, alert_body, secret=args.secret)
    require(status == 200, f"unexpected webhook response {status}: {response}")
    alert_cycle_id = str(response.get("alert_cycle_id") or "")
    scout_agent_id = str(response.get("scout_agent_id") or "")
    monitor_id = str(response.get("monitor_id") or "")
    require(alert_cycle_id, f"missing alert_cycle_id: {response}")
    require(scout_agent_id, f"missing scout_agent_id: {response}")

    scout_wait = client.wait_for_agent(scout_agent_id, args.timeout_ms)
    alert_cycle = wait_for_alert_cycle_terminal(client, alert_cycle_id, args.timeout_ms / 1000)
    work_cycles = client.list(
        "WorkCycles",
        filter_expr=f"project_harness_id eq '{harness_id}'",
        orderby="sequence_nr desc",
        top=5,
    )
    child_agents = client.list(
        "Agents",
        filter_expr=f"parent_agent_id eq '{scout_agent_id}'",
        orderby="sequence_nr desc",
        top=5,
    )

    require(
        str(alert_cycle.get("Status") or alert_cycle.get("status") or "") in {"Fixed", "Tuned", "Failed"},
        f"AlertCycle did not reach terminal state: {alert_cycle}",
    )

    summary = {
        "project_harness_id": harness_id,
        "monitor_id": monitor_id,
        "alert_cycle_id": alert_cycle_id,
        "scout_agent_id": scout_agent_id,
        "scout_wait": scout_wait,
        "alert_cycle": alert_cycle,
        "work_cycles": work_cycles,
        "child_agents": child_agents,
    }
    print(json.dumps(summary, indent=2))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
