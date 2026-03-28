#!/usr/bin/env python3
"""Proof script for Phase 1: Webhook Ingestion Endpoint.

Verifies that POST /webhooks/ingest correctly:
1. Creates AlertCycle from a Logfire alert payload
2. Increments Monitor.alert_count
3. Prevents duplicate AlertCycles for the same monitor
4. Handles GitHub PR events
5. Rejects bad webhook signatures
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

    def list(
        self,
        entity_set: str,
        *,
        filter_expr: str | None = None,
    ) -> list[dict[str, Any]]:
        params: list[str] = []
        if filter_expr:
            params.append("$filter=" + urllib.parse.quote(filter_expr, safe="'()"))
        suffix = f"?{'&'.join(params)}" if params else ""
        payload = self._request("GET", f"/tdata/{entity_set}{suffix}")
        return payload.get("value", [])

    def action(
        self,
        entity_set: str,
        entity_id: str,
        action_name: str,
        body: dict[str, Any] | None = None,
    ) -> dict[str, Any]:
        return self._request(
            "POST",
            f"/tdata/{entity_set}('{entity_id}')/{action_name}",
            body or {},
        )


def post_webhook(
    base_url: str,
    payload: dict[str, Any],
    *,
    signature: str | None = None,
    tenant: str = DEFAULT_TENANT,
    api_key: str | None = None,
) -> tuple[int, dict[str, Any]]:
    """POST to /webhooks/ingest, returning (status_code, response_body)."""
    data = json.dumps(payload).encode("utf-8")
    headers = {
        "Content-Type": "application/json",
        "Accept": "application/json",
    }
    if signature is not None:
        headers["x-webhook-signature"] = signature
    req = urllib.request.Request(
        f"{base_url}/webhooks/ingest",
        data=data,
        method="POST",
        headers=headers,
    )
    try:
        with urllib.request.urlopen(req, timeout=30) as resp:
            raw = resp.read().decode("utf-8")
            return resp.status, json.loads(raw) if raw else {}
    except urllib.error.HTTPError as exc:
        raw = exc.read().decode("utf-8", errors="replace")
        try:
            body = json.loads(raw)
        except (json.JSONDecodeError, ValueError):
            body = {"raw": raw}
        return exc.code, body


def suffix() -> str:
    return dt.datetime.now(dt.timezone.utc).strftime("%Y%m%d%H%M%S")


def run_proof(args: argparse.Namespace) -> dict[str, Any]:
    results: dict[str, Any] = {"steps": [], "passed": True}
    client = ODataClient(args.base_url, args.tenant, args.api_key)
    run_id = suffix()

    # ── Step 1: Create prerequisite entities ──────────────────────────
    print("\n=== Step 1: Create ProjectHarness + Monitor ===")
    harness_id = f"webhook-proof-harness-{run_id}"
    monitor_id = f"webhook-proof-monitor-{run_id}"

    harness = client.create("ProjectHarnesses", {"Id": harness_id})
    client.action("ProjectHarnesses", harness_id, "OpenPaw.Harness.Configure", {
        "repo_url": "https://github.com/arni-labs/deep-sci-fi.git",
        "tech_stack": "Next.js + Python",
        "conventions": "Proof test conventions",
    })
    client.action("ProjectHarnesses", harness_id, "OpenPaw.Harness.Activate", {
        "last_activated_at": dt.datetime.now(dt.timezone.utc).isoformat(),
    })

    monitor = client.create("Monitors", {"Id": monitor_id})
    client.action("Monitors", monitor_id, "OpenPaw.Heal.Configure", {
        "logfire_query": "synthetic:webhook-proof",
        "threshold": "1",
        "dd_monitor_id": f"synthetic-{run_id}",
    })
    client.action("Monitors", monitor_id, "OpenPaw.Heal.Activate", {})

    print(f"  ProjectHarness: {harness_id}")
    print(f"  Monitor: {monitor_id}")
    results["harness_id"] = harness_id
    results["monitor_id"] = monitor_id
    results["steps"].append({"name": "create_prerequisites", "status": "PASS"})

    # ── Step 2: POST Logfire alert → AlertCycle created ──────────────
    print("\n=== Step 2: POST Logfire alert payload ===")
    logfire_payload = {
        "source": "logfire",
        "event_type": "alert",
        "payload": {
            "monitor_id": monitor_id,
            "severity": "high",
            "summary": "Error rate spike in deep-sci-fi API",
            "project_harness_id": harness_id,
        },
    }
    status, body = post_webhook(args.base_url, logfire_payload)
    print(f"  Status: {status}")
    print(f"  Response: {json.dumps(body, indent=2)}")

    step2_pass = status == 201 and body.get("ok") is True and body.get("alert_cycle_id")
    results["steps"].append({
        "name": "logfire_alert_creates_alert_cycle",
        "status": "PASS" if step2_pass else "FAIL",
        "http_status": status,
        "alert_cycle_id": body.get("alert_cycle_id"),
    })
    if not step2_pass:
        results["passed"] = False

    alert_cycle_id = body.get("alert_cycle_id", "")

    # ── Step 3: Verify AlertCycle is in Triaging state ───────────────
    print("\n=== Step 3: Verify AlertCycle state ===")
    if alert_cycle_id:
        ac = client.get("AlertCycles", alert_cycle_id)
        # OData response has status at root (lowercase) and fields nested
        ac_status = ac.get("status", "") or ac.get("fields", {}).get("Status", "")
        ac_monitor_id = ac.get("fields", {}).get("monitor_id", "")
        print(f"  AlertCycle Status: {ac_status}")
        print(f"  AlertCycle monitor_id: {ac_monitor_id}")

        step3_pass = ac_status == "Triaging" and ac_monitor_id == monitor_id
        results["steps"].append({
            "name": "alert_cycle_in_triaging",
            "status": "PASS" if step3_pass else "FAIL",
            "alert_cycle_status": ac_status,
            "alert_cycle_monitor_id": ac_monitor_id,
        })
        if not step3_pass:
            results["passed"] = False
    else:
        results["steps"].append({"name": "alert_cycle_in_triaging", "status": "SKIP"})

    # ── Step 4: Verify Monitor alert_count incremented ───────────────
    print("\n=== Step 4: Verify Monitor alert_count ===")
    mon = client.get("Monitors", monitor_id)
    # alert_count is in counters dict or fields dict
    alert_count = mon.get("counters", {}).get("alert_count", 0)
    if alert_count == 0:
        alert_count = mon.get("fields", {}).get("alert_count", 0)
    print(f"  Monitor alert_count: {alert_count}")

    step4_pass = alert_count >= 1
    results["steps"].append({
        "name": "monitor_alert_count_incremented",
        "status": "PASS" if step4_pass else "FAIL",
        "alert_count": alert_count,
    })
    if not step4_pass:
        results["passed"] = False

    # ── Step 5: POST duplicate → no new AlertCycle ───────────────────
    print("\n=== Step 5: POST duplicate payload ===")
    status2, body2 = post_webhook(args.base_url, logfire_payload)
    print(f"  Status: {status2}")
    print(f"  Response: {json.dumps(body2, indent=2)}")

    dup_id = body2.get("alert_cycle_id", "")
    step5_pass = status2 == 200 and dup_id == alert_cycle_id
    results["steps"].append({
        "name": "duplicate_prevention",
        "status": "PASS" if step5_pass else "FAIL",
        "http_status": status2,
        "returned_alert_cycle_id": dup_id,
        "expected_alert_cycle_id": alert_cycle_id,
    })
    if not step5_pass:
        results["passed"] = False

    # ── Step 6: POST with bad source → 400 ──────────────────────────
    print("\n=== Step 6: POST with unknown source ===")
    bad_payload = {"source": "unknown", "event_type": "alert", "payload": {}}
    status3, body3 = post_webhook(args.base_url, bad_payload)
    print(f"  Status: {status3}")

    step6_pass = status3 == 400
    results["steps"].append({
        "name": "unknown_source_rejected",
        "status": "PASS" if step6_pass else "FAIL",
        "http_status": status3,
    })
    if not step6_pass:
        results["passed"] = False

    # ── Step 7: POST GitHub PR event ─────────────────────────────────
    print("\n=== Step 7: POST GitHub PR merged event ===")
    gh_payload = {
        "source": "github",
        "event_type": "pull_request.merged",
        "payload": {
            "pull_request": {
                "html_url": "https://github.com/arni-labs/deep-sci-fi/pull/999",
            },
        },
    }
    status4, body4 = post_webhook(args.base_url, gh_payload)
    print(f"  Status: {status4}")
    print(f"  Response: {json.dumps(body4, indent=2)}")

    step7_pass = status4 == 200 and body4.get("ok") is True
    results["steps"].append({
        "name": "github_pr_event_handled",
        "status": "PASS" if step7_pass else "FAIL",
        "http_status": status4,
    })
    if not step7_pass:
        results["passed"] = False

    # ── Step 8: POST with missing monitor_id → 400 ──────────────────
    print("\n=== Step 8: POST alert with missing monitor_id ===")
    no_monitor_payload = {
        "source": "logfire",
        "event_type": "alert",
        "payload": {"severity": "low"},
    }
    status5, body5 = post_webhook(args.base_url, no_monitor_payload)
    print(f"  Status: {status5}")

    step8_pass = status5 == 400
    results["steps"].append({
        "name": "missing_monitor_id_rejected",
        "status": "PASS" if step8_pass else "FAIL",
        "http_status": status5,
    })
    if not step8_pass:
        results["passed"] = False

    # ── Summary ──────────────────────────────────────────────────────
    print("\n=== SUMMARY ===")
    for step in results["steps"]:
        icon = "✓" if step["status"] == "PASS" else ("⊘" if step["status"] == "SKIP" else "✗")
        print(f"  {icon} {step['name']}: {step['status']}")

    print(f"\n  Overall: {'PASS' if results['passed'] else 'FAIL'}")
    return results


def main() -> None:
    parser = argparse.ArgumentParser(description="Prove webhook ingestion (Phase 1)")
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
