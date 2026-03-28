#!/usr/bin/env python3
"""Proof script for Phase 5: PM Integration with Alert Flow.

Verifies that the PM Issue entity can be created and linked to alerts,
and that the Scout soul has instructions to create Issues during triage.

This script tests the entity-level flow directly via OData, simulating
what Scout would do when it creates an Issue for a real alert.
"""

from __future__ import annotations

import argparse
import datetime as dt
import json
import os
import sys
import urllib.error
import urllib.parse
import urllib.request
from typing import Any


DEFAULT_BASE_URL = "http://127.0.0.1:3467"
DEFAULT_TENANT = "default"


class ODataClient:
    def __init__(self, base_url: str, tenant: str, api_key: str | None = None) -> None:
        self.base_url = base_url.rstrip("/")
        self.tenant = tenant
        self.api_key = api_key

    def _headers(self, json_body: bool = True) -> dict[str, str]:
        headers = {
            "X-Tenant-Id": self.tenant,
            "X-Temper-Principal-Kind": "admin",
            "Accept": "application/json",
        }
        if json_body:
            headers["Content-Type"] = "application/json"
        if self.api_key:
            headers["Authorization"] = f"Bearer {self.api_key}"
        return headers

    def _request(self, method: str, path: str, body: Any | None = None) -> Any:
        data = None
        if body is not None:
            data = json.dumps(body).encode("utf-8")
        req = urllib.request.Request(
            f"{self.base_url}{path}",
            data=data,
            method=method,
            headers=self._headers(json_body=body is not None),
        )
        try:
            with urllib.request.urlopen(req, timeout=60) as resp:
                raw = resp.read().decode("utf-8")
        except urllib.error.HTTPError as exc:
            payload = exc.read().decode("utf-8", errors="replace")
            raise RuntimeError(f"{method} {path} failed ({exc.code}): {payload}") from exc
        if not raw:
            return {}
        try:
            return json.loads(raw)
        except json.JSONDecodeError:
            return raw

    def create(self, entity_set: str, body: dict[str, Any] | None = None) -> dict[str, Any]:
        return self._request("POST", f"/tdata/{entity_set}", body or {})

    def get(self, entity_set: str, entity_id: str) -> dict[str, Any]:
        return self._request("GET", f"/tdata/{entity_set}('{entity_id}')")

    def list(self, entity_set: str, *, filter_expr: str | None = None) -> list[dict[str, Any]]:
        params: list[str] = []
        if filter_expr:
            params.append("$filter=" + urllib.parse.quote(filter_expr, safe="'()"))
        suffix = f"?{'&'.join(params)}" if params else ""
        payload = self._request("GET", f"/tdata/{entity_set}{suffix}")
        return payload.get("value", [])

    def action(
        self, entity_set: str, entity_id: str, action_name: str,
        body: dict[str, Any] | None = None,
    ) -> dict[str, Any]:
        return self._request(
            "POST", f"/tdata/{entity_set}('{entity_id}')/{action_name}", body or {},
        )


def entity_id(entity: dict[str, Any]) -> str:
    return str(entity.get("entity_id") or entity.get("Id") or "")


def suffix() -> str:
    return dt.datetime.now(dt.timezone.utc).strftime("%Y%m%d%H%M%S")


def now_utc() -> str:
    return dt.datetime.now(dt.timezone.utc).replace(microsecond=0).isoformat()


def run_proof(args: argparse.Namespace) -> dict[str, Any]:
    results: dict[str, Any] = {"steps": [], "passed": True}
    client = ODataClient(args.base_url, args.tenant, args.api_key)
    run_id = suffix()

    # ── Step 1: Create alert-related prerequisites ───────────────────
    print("\n=== Step 1: Create ProjectHarness + Monitor + AlertCycle ===")
    harness_id = f"pm-proof-harness-{run_id}"
    monitor_id = f"pm-proof-monitor-{run_id}"

    client.create("ProjectHarnesses", {"Id": harness_id})
    client.action("ProjectHarnesses", harness_id, "OpenPaw.Harness.Configure", {
        "repo_url": "https://github.com/arni-labs/deep-sci-fi.git",
        "tech_stack": "Next.js + Python",
        "conventions": "PM integration proof",
    })
    client.action("ProjectHarnesses", harness_id, "OpenPaw.Harness.Activate", {
        "last_activated_at": now_utc(),
    })

    client.create("Monitors", {"Id": monitor_id})
    client.action("Monitors", monitor_id, "OpenPaw.Heal.Configure", {
        "logfire_query": "synthetic:pm-proof",
        "threshold": "1",
        "dd_monitor_id": f"synthetic-pm-{run_id}",
    })
    client.action("Monitors", monitor_id, "OpenPaw.Heal.Activate", {})

    alert_cycle = client.create("AlertCycles")
    alert_cycle_id = entity_id(alert_cycle)
    alert_payload = json.dumps({
        "monitor_id": monitor_id,
        "severity": "high",
        "summary": "npm ci failure in deep-sci-fi platform",
    })
    client.action("AlertCycles", alert_cycle_id, "OpenPaw.Heal.Open", {
        "monitor_id": monitor_id,
        "alert_payload": alert_payload,
        "scout_agent_id": "",
    })

    print(f"  ProjectHarness: {harness_id}")
    print(f"  Monitor: {monitor_id}")
    print(f"  AlertCycle: {alert_cycle_id}")
    results["steps"].append({"name": "create_alert_prerequisites", "status": "PASS"})

    # ── Step 2: Create PM Issue (simulating Scout behavior) ──────────
    print("\n=== Step 2: Create PM Issue for alert ===")
    issue = client.create("Issues")
    issue_id = entity_id(issue)
    print(f"  Issue created: {issue_id}")

    client.action("Issues", issue_id, "OpenPaw.PM.SetDescription", {
        "description": (
            f"Alert from monitor {monitor_id}: npm ci failure in deep-sci-fi platform. "
            f"AlertCycle: {alert_cycle_id}. "
            f"Severity: high. "
            f"Reproduction: cd platform && npm ci fails due to lockfile drift."
        ),
    })
    client.action("Issues", issue_id, "OpenPaw.PM.SetPriority", {"priority": 3})

    issue_data = client.get("Issues", issue_id)
    issue_status = issue_data.get("status", "") or issue_data.get("fields", {}).get("Status", "")
    issue_priority = issue_data.get("counters", {}).get("priority", 0)
    issue_desc = issue_data.get("fields", {}).get("description", "")
    has_desc = issue_data.get("booleans", {}).get("has_description", False)

    print(f"  Issue Status: {issue_status}")
    print(f"  Issue Priority: {issue_priority}")
    print(f"  Has Description: {has_desc}")
    print(f"  Description contains monitor_id: {monitor_id in issue_desc}")

    step2_pass = (
        issue_id
        and issue_priority >= 1
        and monitor_id in issue_desc
        and alert_cycle_id in issue_desc
    )
    results["steps"].append({
        "name": "create_pm_issue",
        "status": "PASS" if step2_pass else "FAIL",
        "issue_id": issue_id,
        "issue_status": issue_status,
        "issue_priority": issue_priority,
        "has_description": has_desc,
        "description_has_monitor_id": monitor_id in issue_desc,
        "description_has_alert_cycle_id": alert_cycle_id in issue_desc,
    })
    if not step2_pass:
        results["passed"] = False

    # ── Step 3: Move Issue to Triage ─────────────────────────────────
    print("\n=== Step 3: Move Issue to Triage ===")
    client.action("Issues", issue_id, "OpenPaw.PM.MoveToTriage", {})
    issue_data = client.get("Issues", issue_id)
    issue_status = issue_data.get("status", "") or issue_data.get("fields", {}).get("Status", "")
    print(f"  Issue Status after triage: {issue_status}")

    step3_pass = issue_status == "Triage"
    results["steps"].append({
        "name": "move_to_triage",
        "status": "PASS" if step3_pass else "FAIL",
        "issue_status": issue_status,
    })
    if not step3_pass:
        results["passed"] = False

    # ── Step 4: Add comment to Issue ─────────────────────────────────
    print("\n=== Step 4: Add comment linking to AlertCycle ===")
    client.action("Issues", issue_id, "OpenPaw.PM.AddComment", {
        "comment": f"Alert triage started. AlertCycle {alert_cycle_id} is in Triaging state.",
    })
    issue_data = client.get("Issues", issue_id)
    comment_count = issue_data.get("counters", {}).get("comment_count", 0)
    print(f"  Comment count: {comment_count}")

    step4_pass = comment_count >= 1
    results["steps"].append({
        "name": "add_comment",
        "status": "PASS" if step4_pass else "FAIL",
        "comment_count": comment_count,
    })
    if not step4_pass:
        results["passed"] = False

    # ── Step 5: Verify Scout soul has PM instructions ────────────────
    print("\n=== Step 5: Verify Scout soul has PM instructions ===")
    with open("souls/scout.md") as f:
        scout_content = f.read()
    has_pm_section = "PM Integration" in scout_content
    has_issue_create = "create" in scout_content.lower() and "Issue" in scout_content
    has_set_priority = "SetPriority" in scout_content
    has_issue_id_output = "ISSUE_ID" in scout_content

    print(f"  Has PM Integration section: {has_pm_section}")
    print(f"  Has Issue creation: {has_issue_create}")
    print(f"  Has SetPriority: {has_set_priority}")
    print(f"  Has ISSUE_ID output: {has_issue_id_output}")

    step5_pass = has_pm_section and has_issue_create and has_set_priority and has_issue_id_output
    results["steps"].append({
        "name": "scout_soul_has_pm_instructions",
        "status": "PASS" if step5_pass else "FAIL",
        "has_pm_section": has_pm_section,
        "has_issue_create": has_issue_create,
        "has_set_priority": has_set_priority,
        "has_issue_id_output": has_issue_id_output,
    })
    if not step5_pass:
        results["passed"] = False

    results["issue_id"] = issue_id
    results["alert_cycle_id"] = alert_cycle_id

    # ── Summary ──────────────────────────────────────────────────────
    print("\n=== SUMMARY ===")
    for step in results["steps"]:
        icon = "✓" if step["status"] == "PASS" else "✗"
        print(f"  {icon} {step['name']}: {step['status']}")
    print(f"\n  Overall: {'PASS' if results['passed'] else 'FAIL'}")
    return results


def main() -> None:
    parser = argparse.ArgumentParser(description="Prove PM integration (Phase 5)")
    parser.add_argument("--base-url", default=DEFAULT_BASE_URL)
    parser.add_argument("--tenant", default=DEFAULT_TENANT)
    parser.add_argument("--api-key", default=os.environ.get("TEMPER_API_KEY"))
    args = parser.parse_args()

    results = run_proof(args)
    print("\n" + json.dumps(results, indent=2))
    if not results["passed"]:
        sys.exit(1)


if __name__ == "__main__":
    main()
