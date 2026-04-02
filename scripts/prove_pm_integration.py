#!/usr/bin/env python3
"""Prove SRE creates a PM Issue for a real alert."""

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


def wait_for_issue(client: ODataClient, alert_cycle_id: str, timeout_secs: float) -> dict[str, object]:
    deadline = time.time() + timeout_secs
    while time.time() < deadline:
        issues = client.list("Issues", orderby="sequence_nr desc", top=25)
        match = next(
            (
                issue
                for issue in issues
                if alert_cycle_id
                in str(
                    issue.get("fields", {}).get("Description")
                    or issue.get("fields", {}).get("description")
                    or issue.get("Description")
                    or ""
                )
            ),
            None,
        )
        if match is not None:
            return match
        time.sleep(2)
    raise TimeoutError(f"timed out waiting for Issue linked to AlertCycle {alert_cycle_id}")


def wait_for_work_cycle(
    client: ODataClient,
    project_harness_id: str,
    timeout_secs: float,
) -> dict[str, object]:
    deadline = time.time() + timeout_secs
    while time.time() < deadline:
        work_cycles = client.list(
            "WorkCycles",
            filter_expr=f"project_harness_id eq '{project_harness_id}'",
            orderby="sequence_nr desc",
            top=10,
        )
        if work_cycles:
            return work_cycles[0]
        time.sleep(2)
    raise TimeoutError(f"timed out waiting for WorkCycle tied to Harness {project_harness_id}")


def wait_for_child_agent(
    client: ODataClient,
    parent_agent_id: str,
    timeout_secs: float,
) -> dict[str, object] | None:
    deadline = time.time() + timeout_secs
    while time.time() < deadline:
        children = client.list(
            "Sessions",
            filter_expr=f"parent_session_id eq '{parent_agent_id}'",
            orderby="sequence_nr desc",
            top=5,
        )
        if children:
            return children[0]
        time.sleep(2)
    return None


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
    harness = client.create("Harnesses", {"Id": f"pm-harness-{run_suffix}"})
    harness_id = entity_id(harness)
    require(harness_id, "failed to create Harness")
    client.action(
        "Harnesses",
        harness_id,
        "OpenPaw.Harness.Configure",
        {
            "repo_url": args.repo_url,
            "tech_stack": "Next.js frontend, Python backend",
            "conventions": "PM integration proof harness.",
        },
    )
    client.action(
        "Harnesses",
        harness_id,
        "OpenPaw.Harness.Activate",
        {"last_activated_at": now_utc()},
    )

    external_monitor_id = f"pm-monitor-{run_suffix}"
    body = {
        "source": "datadog",
        "event_type": "alert_fired",
        "payload": {
            "monitor_id": external_monitor_id,
            "project_harness_id": harness_id,
            "repo_url": args.repo_url,
            "severity": "high",
            "summary": "SRE should create a PM issue for this alert",
            "reproduction": {
                "command": "npm ci",
                "failure": "package-lock drift detected during PM integration proof",
            },
        },
    }
    status, response = webhook_post(args.base_url, body, secret=args.secret)
    require(status == 200, f"unexpected webhook response {status}: {response}")
    alert_cycle_id = str(response.get("alert_cycle_id") or "")
    sre_agent_id = str(response.get("sre_agent_id") or "")
    require(alert_cycle_id, f"missing alert_cycle_id: {response}")
    require(sre_agent_id, f"missing sre_agent_id: {response}")

    issue = wait_for_issue(client, alert_cycle_id, args.timeout_ms / 1000)
    work_cycle = wait_for_work_cycle(client, harness_id, args.timeout_ms / 1000)
    developer_agent = wait_for_child_agent(client, sre_agent_id, min(args.timeout_ms / 1000, 120))
    alert_cycle = client.get("AlertCycles", alert_cycle_id)
    sre_agent = client.get("Sessions", sre_agent_id)
    description = str(
        issue.get("fields", {}).get("Description")
        or issue.get("fields", {}).get("description")
        or issue.get("Description")
        or ""
    )
    require(alert_cycle_id in description, f"Issue description missing AlertCycle id: {issue}")
    issue_status = str(issue.get("Status") or issue.get("status") or issue.get("fields", {}).get("Status") or "")
    require(issue_status in {"Backlog", "Triage", "Todo", "InProgress", "Testing", "Reviewing", "Complete"}, f"unexpected Issue status: {issue}")

    summary = {
        "project_harness_id": harness_id,
        "alert_cycle_id": alert_cycle_id,
        "alert_cycle": alert_cycle,
        "sre_agent_id": sre_agent_id,
        "sre_agent": sre_agent,
        "work_cycle": work_cycle,
        "developer_agent": developer_agent,
        "issue": issue,
    }
    print(json.dumps(summary, indent=2))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
