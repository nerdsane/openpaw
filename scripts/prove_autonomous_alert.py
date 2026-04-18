#!/usr/bin/env python3
"""Prove a native Datadog alert can autonomously open an SRE-led remediation loop."""

from __future__ import annotations

import argparse
import json
import os
import time

from temperpaw_proof_support import (
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


def wait_for_work_cycle(
    client: ODataClient,
    project_harness_id: str,
    timeout_secs: float,
) -> dict[str, object] | None:
    deadline = time.time() + timeout_secs
    while time.time() < deadline:
        work_cycles = client.list(
            "WorkCycles",
            filter_expr=f"project_harness_id eq '{project_harness_id}'",
            orderby="sequence_nr desc",
            top=5,
        )
        if work_cycles:
            return work_cycles[0]
        time.sleep(2)
    return None


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--base-url", default=os.getenv("TEMPERPAW_BASE_URL", DEFAULT_BASE_URL))
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

    harness = client.create("Harnesses", {"Id": f"autonomous-alert-harness-{run_suffix}"})
    harness_id = entity_id(harness)
    require(harness_id, "failed to create Harness")
    client.action(
        "Harnesses",
        harness_id,
        "TemperPaw.Harness.Configure",
        {
            "repo_url": args.repo_url,
            "tech_stack": "Next.js frontend, Python backend",
            "conventions": "Autonomous Datadog alert proof harness.",
        },
    )
    client.action(
        "Harnesses",
        harness_id,
        "TemperPaw.Harness.Activate",
        {"last_activated_at": now_utc()},
    )

    dd_monitor_id = f"autonomous-alert-monitor-{run_suffix}"
    payload = {
        "org": {"id": 12345, "name": "temperpaw-proof"},
        "id": dd_monitor_id,
        "title": "Autonomous Datadog alert proof",
        "text": "package-lock drift detected during autonomous alert proof",
        "alert_type": "error",
        "alert_transition": "Triggered",
        "priority": "P1",
        "project_harness_id": harness_id,
        "repo_url": args.repo_url,
        "query": f"synthetic:datadog:autonomous:{run_suffix}",
        "reproduction": {
            "command": "npm ci",
            "failure": "package-lock drift detected during autonomous Datadog alert proof",
        },
    }
    status, response = webhook_post(args.base_url, payload, secret=args.secret)
    require(status == 200, f"unexpected webhook response {status}: {response}")

    monitor_id = str(response.get("monitor_id") or "")
    alert_cycle_id = str(response.get("alert_cycle_id") or "")
    sre_agent_id = str(response.get("sre_agent_id") or "")
    require(monitor_id, f"missing monitor_id: {response}")
    require(alert_cycle_id, f"missing alert_cycle_id: {response}")
    require(sre_agent_id, f"missing sre_agent_id: {response}")

    sre_wait = client.wait_for_agent(sre_agent_id, args.timeout_ms)
    alert_cycle = client.get("AlertCycles", alert_cycle_id)
    work_cycle = wait_for_work_cycle(client, harness_id, min(args.timeout_ms / 1000, 120))
    child_agents = client.list(
        "Sessions",
        filter_expr=f"parent_session_id eq '{sre_agent_id}'",
        orderby="sequence_nr desc",
        top=5,
    )

    summary = {
        "project_harness_id": harness_id,
        "monitor_id": monitor_id,
        "alert_cycle_id": alert_cycle_id,
        "sre_agent_id": sre_agent_id,
        "sre_wait": sre_wait,
        "alert_cycle": alert_cycle,
        "work_cycle": work_cycle,
        "child_agents": child_agents,
    }
    print(json.dumps(summary, indent=2))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
