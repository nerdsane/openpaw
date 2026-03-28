#!/usr/bin/env python3
"""Proof script for Phase 2: Webhook-Triggered Scout Auto-Spawn.

Verifies that POST /webhooks/ingest automatically spawns a Scout agent
to triage the alert. Optionally waits for the Scout to complete.
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
DEFAULT_MODEL = "claude-sonnet-4-20250514"
DEFAULT_REPO_URL = "https://github.com/arni-labs/deep-sci-fi.git"


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
            with urllib.request.urlopen(req, timeout=600) as resp:
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
        orderby: str | None = None,
        top: int | None = None,
    ) -> list[dict[str, Any]]:
        params: list[str] = []
        if filter_expr:
            params.append("$filter=" + urllib.parse.quote(filter_expr, safe="'()"))
        if orderby:
            params.append("$orderby=" + urllib.parse.quote(orderby, safe=","))
        if top:
            params.append(f"$top={top}")
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

    def wait_for_agent(self, agent_id: str, timeout_ms: int) -> dict[str, Any]:
        remaining_ms = max(timeout_ms, 1000)
        while True:
            chunk_ms = min(remaining_ms, 300_000)
            path = (
                "/observe/entities/Agent/"
                f"{agent_id}/wait?statuses=Completed,Failed,Cancelled"
                f"&timeout_ms={chunk_ms}&poll_ms=500"
            )
            payload = self._request("GET", path)
            status = str(payload.get("status") or payload.get("Status") or "")
            if status in {"Completed", "Failed", "Cancelled"}:
                return payload
            if not payload.get("timed_out"):
                return payload
            remaining_ms -= chunk_ms
            if remaining_ms <= 0:
                return payload


def post_webhook(
    base_url: str,
    payload: dict[str, Any],
) -> tuple[int, dict[str, Any]]:
    data = json.dumps(payload).encode("utf-8")
    headers = {"Content-Type": "application/json", "Accept": "application/json"}
    req = urllib.request.Request(
        f"{base_url}/webhooks/ingest", data=data, method="POST", headers=headers,
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


def now_utc() -> str:
    return dt.datetime.now(dt.timezone.utc).replace(microsecond=0).isoformat()


def run_proof(args: argparse.Namespace) -> dict[str, Any]:
    results: dict[str, Any] = {"steps": [], "passed": True}
    client = ODataClient(args.base_url, args.tenant, args.api_key)
    run_id = suffix()

    # ── Step 1: Create prerequisites ─────────────────────────────────
    print("\n=== Step 1: Create ProjectHarness + Monitor ===")
    harness_id = f"scout-spawn-harness-{run_id}"
    monitor_id = f"scout-spawn-monitor-{run_id}"

    client.create("ProjectHarnesses", {"Id": harness_id})
    client.action("ProjectHarnesses", harness_id, "OpenPaw.Harness.Configure", {
        "repo_url": args.repo_url,
        "tech_stack": "Next.js + Python",
        "conventions": "Self-heal proof conventions",
    })
    client.action("ProjectHarnesses", harness_id, "OpenPaw.Harness.Activate", {
        "last_activated_at": now_utc(),
    })

    client.create("Monitors", {"Id": monitor_id})
    client.action("Monitors", monitor_id, "OpenPaw.Heal.Configure", {
        "logfire_query": "synthetic:scout-spawn-proof",
        "threshold": "1",
        "dd_monitor_id": f"synthetic-{run_id}",
    })
    client.action("Monitors", monitor_id, "OpenPaw.Heal.Activate", {})

    print(f"  ProjectHarness: {harness_id}")
    print(f"  Monitor: {monitor_id}")
    results["harness_id"] = harness_id
    results["monitor_id"] = monitor_id
    results["steps"].append({"name": "create_prerequisites", "status": "PASS"})

    # ── Step 2: POST webhook → should auto-spawn Scout ───────────────
    print("\n=== Step 2: POST webhook payload ===")
    webhook_payload = {
        "source": "logfire",
        "event_type": "alert",
        "payload": {
            "monitor_id": monitor_id,
            "severity": "high",
            "summary": "npm ci failure in deep-sci-fi",
            "project_harness_id": harness_id,
            "repo_url": args.repo_url,
            "reproduction": {
                "cwd": "platform",
                "command": "npm ci",
                "failure": "package.json and package-lock.json out of sync",
            },
            "notes": ["Treat as real issue, not noise."],
        },
    }
    status, body = post_webhook(args.base_url, webhook_payload)
    print(f"  Status: {status}")
    print(f"  Response: {json.dumps(body, indent=2)}")

    alert_cycle_id = body.get("alert_cycle_id", "")
    message = body.get("message", "")
    scout_spawned = "Scout auto-spawned" in message
    scout_agent_id = message.replace("Scout auto-spawned: ", "") if scout_spawned else ""

    step2_pass = status == 201 and alert_cycle_id and scout_spawned
    results["steps"].append({
        "name": "webhook_auto_spawns_scout",
        "status": "PASS" if step2_pass else "FAIL",
        "http_status": status,
        "alert_cycle_id": alert_cycle_id,
        "scout_agent_id": scout_agent_id,
        "scout_spawned": scout_spawned,
    })
    if not step2_pass:
        results["passed"] = False
    results["alert_cycle_id"] = alert_cycle_id
    results["scout_agent_id"] = scout_agent_id

    # ── Step 3: Verify Scout agent exists and is configured ──────────
    print("\n=== Step 3: Verify Scout agent configuration ===")
    if scout_agent_id:
        scout = client.get("Agents", scout_agent_id)
        scout_status = scout.get("status", "") or scout.get("fields", {}).get("Status", "")
        scout_soul = scout.get("fields", {}).get("soul_id", "")
        print(f"  Scout Status: {scout_status}")
        print(f"  Scout soul_id: {scout_soul}")

        # Scout should be in a non-Created state (provisioned or running)
        step3_pass = scout_status not in ("", "Created") and scout_soul == "Scout"
        results["steps"].append({
            "name": "scout_configured_correctly",
            "status": "PASS" if step3_pass else "FAIL",
            "scout_status": scout_status,
            "scout_soul_id": scout_soul,
        })
        if not step3_pass:
            results["passed"] = False
    else:
        results["steps"].append({"name": "scout_configured_correctly", "status": "SKIP"})

    # ── Step 4: Optionally wait for Scout to complete ────────────────
    if args.wait and scout_agent_id:
        print(f"\n=== Step 4: Waiting for Scout to complete (timeout: {args.timeout_ms}ms) ===")
        scout_result = client.wait_for_agent(scout_agent_id, args.timeout_ms)
        scout_final_status = str(
            scout_result.get("status") or scout_result.get("Status") or ""
        )
        print(f"  Scout final status: {scout_final_status}")

        # Check AlertCycle final state
        ac = client.get("AlertCycles", alert_cycle_id)
        ac_status = ac.get("status", "") or ac.get("fields", {}).get("Status", "")
        print(f"  AlertCycle final status: {ac_status}")

        step4_pass = scout_final_status in ("Completed", "Failed") and ac_status in (
            "Fixed",
            "Tuned",
            "Failed",
        )
        results["steps"].append({
            "name": "scout_completes_triage",
            "status": "PASS" if step4_pass else "FAIL",
            "scout_final_status": scout_final_status,
            "alert_cycle_final_status": ac_status,
        })
        if not step4_pass:
            results["passed"] = False

        # Check for WorkCycles and child developers
        work_cycles = client.list(
            "WorkCycles", filter_expr=f"project_harness_id eq '{harness_id}'"
        )
        child_agents = client.list(
            "Agents", filter_expr=f"parent_agent_id eq '{scout_agent_id}'"
        )
        results["work_cycles"] = len(work_cycles)
        results["child_agents"] = len(child_agents)
        if work_cycles:
            wc = work_cycles[0]
            wc_status = wc.get("status", "") or wc.get("fields", {}).get("Status", "")
            pr_url = wc.get("fields", {}).get("pr_url", "")
            print(f"  WorkCycle status: {wc_status}")
            print(f"  PR URL: {pr_url}")
            results["work_cycle_status"] = wc_status
            results["pr_url"] = pr_url
    else:
        print("\n=== Step 4: SKIPPED (--no-wait or no scout) ===")
        results["steps"].append({"name": "scout_completes_triage", "status": "SKIP"})

    # ── Summary ──────────────────────────────────────────────────────
    print("\n=== SUMMARY ===")
    for step in results["steps"]:
        icon = "✓" if step["status"] == "PASS" else ("⊘" if step["status"] == "SKIP" else "✗")
        print(f"  {icon} {step['name']}: {step['status']}")
    print(f"\n  Overall: {'PASS' if results['passed'] else 'FAIL'}")
    return results


def main() -> None:
    parser = argparse.ArgumentParser(description="Prove webhook → Scout auto-spawn (Phase 2)")
    parser.add_argument("--base-url", default=DEFAULT_BASE_URL)
    parser.add_argument("--tenant", default=DEFAULT_TENANT)
    parser.add_argument("--api-key", default=os.environ.get("TEMPER_API_KEY"))
    parser.add_argument("--repo-url", default=DEFAULT_REPO_URL)
    parser.add_argument("--timeout-ms", type=int, default=15 * 60 * 1000)
    parser.add_argument("--wait", action="store_true", default=False,
                        help="Wait for Scout to complete (requires ANTHROPIC_API_KEY)")
    parser.add_argument("--no-wait", action="store_true", default=False,
                        help="Don't wait for Scout — just verify it was spawned")
    args = parser.parse_args()

    # Default: wait if ANTHROPIC_API_KEY is available, don't wait if not
    if args.no_wait:
        args.wait = False
    elif not args.wait:
        args.wait = bool(os.environ.get("ANTHROPIC_API_KEY"))

    results = run_proof(args)
    print("\n" + json.dumps(results, indent=2))
    if not results["passed"]:
        sys.exit(1)


if __name__ == "__main__":
    main()
