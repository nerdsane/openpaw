#!/usr/bin/env python3
"""Prove webhook ingestion end to end against a live Open Paw daemon."""

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
    harness = client.create("Harnesses", {"Id": f"webhook-harness-{run_suffix}"})
    harness_id = entity_id(harness)
    require(harness_id, "failed to create Harness")
    client.action(
        "Harnesses",
        harness_id,
        "OpenPaw.Harness.Configure",
        {
            "repo_url": repo_url,
            "tech_stack": "Next.js frontend, Python backend",
            "conventions": "Webhook proof harness for autonomous healing flows.",
        },
    )
    client.action(
        "Harnesses",
        harness_id,
        "OpenPaw.Harness.Activate",
        {"last_activated_at": now_utc()},
    )
    return harness_id


def create_monitor(client: ODataClient, run_suffix: str, external_monitor_id: str) -> str:
    monitor = client.create("Monitors", {"Id": f"webhook-monitor-{run_suffix}"})
    monitor_id = entity_id(monitor)
    require(monitor_id, "failed to create Monitor")
    client.action(
        "Monitors",
        monitor_id,
        "OpenPaw.Heal.Configure",
        {
            "dd_query": f"synthetic:webhook:{run_suffix}",
            "threshold": "1",
            "dd_monitor_id": external_monitor_id,
        },
    )
    client.action("Monitors", monitor_id, "OpenPaw.Heal.Activate")
    return monitor_id


def create_reviewing_work_cycle(client: ODataClient, harness_id: str, run_suffix: str) -> str:
    work_cycle = client.create("WorkCycles", {"Id": f"webhook-work-{run_suffix}"})
    work_cycle_id = entity_id(work_cycle)
    require(work_cycle_id, "failed to create WorkCycle")
    client.action(
        "WorkCycles",
        work_cycle_id,
        "OpenPaw.Harness.Configure",
        {
            "project_harness_id": harness_id,
            "task_summary": "Synthetic merge-approval proof cycle",
            "planner_id": "webhook-proof",
        },
    )
    client.action(
        "WorkCycles",
        work_cycle_id,
        "OpenPaw.Harness.WritePlan",
        {
            "plan_summary": "Open a reviewing work cycle and complete it through merged PR webhook approval.",
            "planner_id": "webhook-proof",
        },
    )
    client.action("WorkCycles", work_cycle_id, "OpenPaw.Harness.StartWork")
    client.action("WorkCycles", work_cycle_id, "OpenPaw.Harness.BeginTesting")
    client.action(
        "WorkCycles",
        work_cycle_id,
        "OpenPaw.Harness.PassTests",
        {"test_summary": "Synthetic review gate already passed for webhook verification."},
    )
    return work_cycle_id


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
    external_monitor_id = f"proof-webhook-monitor-{run_suffix}"
    harness_id = create_harness(client, args.repo_url, run_suffix)
    monitor_id = create_monitor(client, run_suffix, external_monitor_id)

    alert_body = {
        "source": "datadog",
        "event_type": "alert_fired",
        "payload": {
            "monitor_id": external_monitor_id,
            "project_harness_id": harness_id,
            "repo_url": args.repo_url,
            "severity": "high",
            "summary": "Synthetic webhook ingestion proof alert",
            "reproduction": {
                "command": "npm ci",
                "failure": "package-lock drift detected by webhook proof",
            },
        },
    }
    status, alert_response = webhook_post(args.base_url, alert_body, secret=args.secret)
    require(status == 200, f"expected 200 from alert webhook, got {status}: {alert_response}")
    require(alert_response.get("accepted") is True, f"unexpected alert response: {alert_response}")
    alert_cycle_id = str(alert_response.get("alert_cycle_id") or "")
    require(alert_cycle_id, f"missing alert_cycle_id: {alert_response}")

    alert_cycle = client.get("AlertCycles", alert_cycle_id)
    require(
        (alert_cycle.get("Status") or alert_cycle.get("status")) == "Triaging",
        f"AlertCycle not opened into Triaging: {alert_cycle}",
    )
    monitor = client.get("Monitors", monitor_id)
    alert_count = int(
        (monitor.get("fields", {}).get("alert_count"))
        or (monitor.get("fields", {}).get("AlertCount"))
        or monitor.get("alert_count")
        or 0
    )
    require(alert_count == 1, f"expected alert_count=1 after webhook, got {monitor}")

    work_cycle_id = create_reviewing_work_cycle(client, harness_id, run_suffix)
    merged_pr_url = f"https://github.com/arni-labs/deep-sci-fi/pull/{run_suffix[-6:]}"
    merge_body = {
        "source": "github",
        "event_type": "pull_request.merged",
        "payload": {
            "work_cycle_id": work_cycle_id,
            "pr_url": merged_pr_url,
            "merged_by": {"login": "webhook-proof"},
            "html_url": merged_pr_url,
        },
    }
    status, merge_response = webhook_post(args.base_url, merge_body, secret=args.secret)
    require(status == 200, f"expected 200 from merge webhook, got {status}: {merge_response}")
    completed_work_cycle = client.get("WorkCycles", work_cycle_id)
    require(
        (completed_work_cycle.get("Status") or completed_work_cycle.get("status")) == "Complete",
        f"WorkCycle did not complete after merge webhook: {completed_work_cycle}",
    )
    require(
        (completed_work_cycle.get("fields", {}).get("pr_url") or completed_work_cycle.get("fields", {}).get("PrUrl"))
        == merged_pr_url,
        f"WorkCycle pr_url missing after merge webhook: {completed_work_cycle}",
    )

    if args.secret:
        status, bad_sig_response = webhook_post(
            args.base_url,
            alert_body,
            secret=args.secret,
            tamper_signature=True,
        )
        require(
            status == 401,
            f"expected 401 for bad HMAC signature, got {status}: {bad_sig_response}",
        )
    else:
        bad_sig_response = {"skipped": "WEBHOOK_SECRET not configured"}

    before_duplicate = client.list(
        "AlertCycles",
        filter_expr=f"monitor_id eq '{monitor_id}'",
        orderby="sequence_nr desc",
        top=20,
    )
    status, duplicate_response = webhook_post(args.base_url, alert_body, secret=args.secret)
    require(status == 200, f"duplicate webhook should still return 200, got {status}: {duplicate_response}")
    after_duplicate = client.list(
        "AlertCycles",
        filter_expr=f"monitor_id eq '{monitor_id}'",
        orderby="sequence_nr desc",
        top=20,
    )
    require(
        len(before_duplicate) == len(after_duplicate),
        "duplicate webhook created an extra AlertCycle",
    )
    require(
        duplicate_response.get("duplicate") is True,
        f"duplicate webhook response did not mark duplicate=true: {duplicate_response}",
    )

    summary = {
        "project_harness_id": harness_id,
        "monitor_id": monitor_id,
        "alert_cycle_id": alert_cycle_id,
        "merge_work_cycle_id": work_cycle_id,
        "alert_response": alert_response,
        "merge_response": merge_response,
        "bad_signature_response": bad_sig_response,
        "duplicate_response": duplicate_response,
    }
    print(json.dumps(summary, indent=2))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
