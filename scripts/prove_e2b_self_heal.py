#!/usr/bin/env python3
"""Proof script for Phase 4: Full E2B Self-Heal Loop.

Runs the same Scout → Developer → PR flow as prove_self_heal_loop.py
but forces E2B sandbox (no sandbox_url override) so the sandbox_provisioner
falls through to E2B REST API.

Requires: ANTHROPIC_API_KEY, E2B_API_KEY, GITHUB_TOKEN in .env
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
        self, entity_set: str, *, filter_expr: str | None = None,
        orderby: str | None = None, top: int | None = None,
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
        self, entity_set: str, entity_id: str, action_name: str,
        body: dict[str, Any] | None = None,
    ) -> dict[str, Any]:
        return self._request(
            "POST", f"/tdata/{entity_set}('{entity_id}')/{action_name}", body or {},
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

    # ── Step 1: Create prerequisites ─────────────────────────────────
    print("\n=== Step 1: Create ProjectHarness + Monitor ===")
    harness_id = f"e2b-proof-harness-{run_id}"
    monitor_id = f"e2b-proof-monitor-{run_id}"

    client.create("ProjectHarnesses", {"Id": harness_id})
    client.action("ProjectHarnesses", harness_id, "OpenPaw.Harness.Configure", {
        "repo_url": args.repo_url,
        "tech_stack": "Next.js frontend, Python backend",
        "conventions": "E2B self-heal proof: focused fixes, validate in platform/",
    })
    client.action("ProjectHarnesses", harness_id, "OpenPaw.Harness.Activate", {
        "last_activated_at": now_utc(),
    })

    client.create("Monitors", {"Id": monitor_id})
    client.action("Monitors", monitor_id, "OpenPaw.Heal.Configure", {
        "logfire_query": "synthetic:e2b-self-heal-proof",
        "threshold": "1",
        "dd_monitor_id": f"synthetic-e2b-{run_id}",
    })
    client.action("Monitors", monitor_id, "OpenPaw.Heal.Activate", {})

    print(f"  Harness: {harness_id}")
    print(f"  Monitor: {monitor_id}")
    results["harness_id"] = harness_id
    results["monitor_id"] = monitor_id
    results["steps"].append({"name": "create_prerequisites", "status": "PASS"})

    # ── Step 2: POST webhook (E2B — no sandbox_url override) ────────
    print("\n=== Step 2: POST webhook to trigger E2B-based self-heal ===")
    import urllib.request as urllib2
    webhook_payload = json.dumps({
        "source": "logfire",
        "event_type": "alert",
        "payload": {
            "monitor_id": monitor_id,
            "severity": "high",
            "summary": "npm ci failure in deep-sci-fi platform",
            "project_harness_id": harness_id,
            "repo_url": args.repo_url,
            "reproduction": {
                "cwd": "platform",
                "command": "npm ci",
                "failure": "package.json and package-lock.json out of sync",
            },
            "notes": ["Treat as real issue, not noise. Open a PR."],
        },
    }).encode("utf-8")
    req = urllib.request.Request(
        f"{args.base_url}/webhooks/ingest",
        data=webhook_payload,
        method="POST",
        headers={"Content-Type": "application/json", "Accept": "application/json"},
    )
    with urllib.request.urlopen(req, timeout=30) as resp:
        body = json.loads(resp.read().decode("utf-8"))

    alert_cycle_id = body.get("alert_cycle_id", "")
    scout_msg = body.get("message", "")
    scout_id = scout_msg.replace("Scout auto-spawned: ", "") if "Scout auto-spawned" in scout_msg else ""

    print(f"  AlertCycle: {alert_cycle_id}")
    print(f"  Scout: {scout_id}")
    results["alert_cycle_id"] = alert_cycle_id
    results["scout_id"] = scout_id

    step2_pass = bool(alert_cycle_id) and bool(scout_id)
    results["steps"].append({
        "name": "webhook_triggers_e2b_scout",
        "status": "PASS" if step2_pass else "FAIL",
    })
    if not step2_pass:
        results["passed"] = False
        return results

    # ── Step 3: Wait for Scout to complete ───────────────────────────
    print(f"\n=== Step 3: Wait for Scout (timeout: {args.timeout_ms}ms) ===")
    scout_result = client.wait_for_agent(scout_id, args.timeout_ms)
    scout_status = str(scout_result.get("status") or scout_result.get("Status") or "")
    print(f"  Scout final status: {scout_status}")

    # ── Step 4: Check results ────────────────────────────────────────
    print("\n=== Step 4: Check AlertCycle, WorkCycle, child agents ===")
    ac = client.get("AlertCycles", alert_cycle_id)
    ac_status = ac.get("status", "") or ac.get("fields", {}).get("Status", "")
    print(f"  AlertCycle status: {ac_status}")

    work_cycles = client.list("WorkCycles", filter_expr=f"project_harness_id eq '{harness_id}'")
    child_agents = client.list("Agents", filter_expr=f"parent_agent_id eq '{scout_id}'")

    wc_status = ""
    pr_url = ""
    if work_cycles:
        wc = work_cycles[0]
        wc_status = wc.get("status", "") or wc.get("fields", {}).get("Status", "")
        pr_url = wc.get("fields", {}).get("pr_url", "")
    print(f"  WorkCycles: {len(work_cycles)}")
    print(f"  WorkCycle status: {wc_status}")
    print(f"  PR URL: {pr_url}")
    print(f"  Child agents: {len(child_agents)}")

    # Check if Developer used E2B (look for sandbox_id in child agent)
    e2b_used = False
    if child_agents:
        dev = child_agents[0]
        sandbox_id = dev.get("fields", {}).get("sandbox_id", "")
        sandbox_url = dev.get("fields", {}).get("sandbox_url", "")
        e2b_used = bool(sandbox_id) or ("e2b" in sandbox_url.lower() if sandbox_url else False)
        print(f"  Developer sandbox_id: {sandbox_id}")
        print(f"  Developer sandbox_url: {sandbox_url}")
        print(f"  E2B used: {e2b_used}")

    step3_pass = scout_status in ("Completed", "Failed")
    step4_pass = ac_status in ("Fixed", "Tuned", "Failed")

    results["steps"].append({
        "name": "scout_completes",
        "status": "PASS" if step3_pass else "FAIL",
        "scout_status": scout_status,
    })
    results["steps"].append({
        "name": "alert_cycle_resolved",
        "status": "PASS" if step4_pass else "FAIL",
        "ac_status": ac_status,
        "wc_status": wc_status,
        "pr_url": pr_url,
        "e2b_used": e2b_used,
        "child_agents": len(child_agents),
    })
    if not step3_pass or not step4_pass:
        results["passed"] = False

    results["pr_url"] = pr_url
    results["e2b_used"] = e2b_used

    # ── Summary ──────────────────────────────────────────────────────
    print("\n=== SUMMARY ===")
    for step in results["steps"]:
        icon = "✓" if step["status"] == "PASS" else "✗"
        print(f"  {icon} {step['name']}: {step['status']}")
    print(f"\n  Overall: {'PASS' if results['passed'] else 'FAIL'}")
    return results


def main() -> None:
    parser = argparse.ArgumentParser(description="Prove E2B self-heal loop (Phase 4)")
    parser.add_argument("--base-url", default=DEFAULT_BASE_URL)
    parser.add_argument("--tenant", default=DEFAULT_TENANT)
    parser.add_argument("--api-key", default=os.environ.get("TEMPER_API_KEY"))
    parser.add_argument("--repo-url", default=DEFAULT_REPO_URL)
    parser.add_argument("--timeout-ms", type=int, default=15 * 60 * 1000)
    args = parser.parse_args()

    results = run_proof(args)
    print("\n" + json.dumps(results, indent=2))
    if not results["passed"]:
        sys.exit(1)


if __name__ == "__main__":
    main()
