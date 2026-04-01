#!/usr/bin/env python3
"""Prove native Datadog webhook normalization and recovery handling."""

from __future__ import annotations

import argparse
import json
import os

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


def create_harness(client: ODataClient, repo_url: str, run_suffix: str) -> str:
    harness = client.create("Harnesses", {"Id": f"dd-webhook-harness-{run_suffix}"})
    harness_id = entity_id(harness)
    require(harness_id, "failed to create Harness")
    client.action(
        "Harnesses",
        harness_id,
        "OpenPaw.Harness.Configure",
        {
            "repo_url": repo_url,
            "tech_stack": "Next.js frontend, Python backend",
            "conventions": "Datadog webhook proof harness.",
        },
    )
    client.action(
        "Harnesses",
        harness_id,
        "OpenPaw.Harness.Activate",
        {"last_activated_at": now_utc()},
    )
    return harness_id


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--base-url", default=os.getenv("OPENPAW_BASE_URL", DEFAULT_BASE_URL))
    parser.add_argument("--tenant", default=os.getenv("PAW_TENANT", DEFAULT_TENANT))
    parser.add_argument("--repo-url", default=DEFAULT_REPO_URL)
    parser.add_argument("--secret", default=os.getenv("WEBHOOK_SECRET"))
    args = parser.parse_args()

    client = ODataClient(
        base_url=args.base_url,
        tenant=args.tenant,
        api_key=os.getenv("TEMPER_API_KEY") or None,
    )
    run_suffix = suffix()
    harness_id = create_harness(client, args.repo_url, run_suffix)
    dd_monitor_id = f"dd-webhook-monitor-{run_suffix}"

    triggered_payload = {
        "org": {"id": 12345, "name": "openpaw-proof"},
        "id": dd_monitor_id,
        "title": "Datadog webhook proof alert",
        "text": "package-lock drift detected by Datadog proof",
        "alert_type": "error",
        "alert_transition": "Triggered",
        "priority": "P1",
        "project_harness_id": harness_id,
        "repo_url": args.repo_url,
        "query": f"synthetic:datadog:webhook:{run_suffix}",
        "reproduction": {
            "command": "npm ci",
            "failure": "package-lock drift detected during Datadog webhook proof",
        },
    }
    status, alert_response = webhook_post(args.base_url, triggered_payload, secret=args.secret)
    require(status == 200, f"unexpected Datadog alert response {status}: {alert_response}")
    monitor_id = str(alert_response.get("monitor_id") or "")
    alert_cycle_id = str(alert_response.get("alert_cycle_id") or "")
    require(monitor_id, f"missing monitor_id: {alert_response}")
    require(alert_cycle_id, f"missing alert_cycle_id: {alert_response}")

    client.action(
        "AlertCycles",
        alert_cycle_id,
        "OpenPaw.Heal.HealComplete",
        {
            "diagnosis": "Proof-only remediation before Datadog recovery event.",
            "pr_url": f"https://github.com/arni-labs/deep-sci-fi/pull/{run_suffix[-6:]}",
            "developer_agent_id": "prove-datadog-webhook",
        },
    )

    recovered_payload = {
        "org": {"id": 12345, "name": "openpaw-proof"},
        "id": dd_monitor_id,
        "title": "Datadog webhook proof alert",
        "text": "monitor returned to normal",
        "alert_type": "success",
        "alert_transition": "Recovered",
    }
    status, recovery_response = webhook_post(args.base_url, recovered_payload, secret=args.secret)
    require(status == 200, f"unexpected Datadog recovery response {status}: {recovery_response}")
    require(
        recovery_response.get("outcome") == "alert_resolved",
        f"unexpected recovery outcome: {recovery_response}",
    )

    monitor = client.get("Monitors", monitor_id)
    alert_cycle = client.get("AlertCycles", alert_cycle_id)
    require(
        str(alert_cycle.get("Status") or alert_cycle.get("status") or "") == "Resolved",
        f"AlertCycle did not reach Resolved: {alert_cycle}",
    )

    summary = {
        "project_harness_id": harness_id,
        "monitor_id": monitor_id,
        "alert_cycle_id": alert_cycle_id,
        "alert_response": alert_response,
        "recovery_response": recovery_response,
        "monitor": monitor,
        "alert_cycle": alert_cycle,
    }
    print(json.dumps(summary, indent=2))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
