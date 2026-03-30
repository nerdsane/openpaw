#!/usr/bin/env python3
"""Run a curl-equivalent self-heal proof against a live Open Paw daemon.

This driver creates a deep-sci-fi ProjectHarness + Monitor, opens a synthetic
AlertCycle for a real repo issue, provisions a SRE agent, and waits for the
SRE -> Developer remediation loop to finish.
"""

from __future__ import annotations

import argparse
import datetime as dt
import json
import os
import sys
import time
import urllib.error
import urllib.parse
import urllib.request
from typing import Any


DEFAULT_BASE_URL = "http://127.0.0.1:3467"
DEFAULT_TENANT = "default"
DEFAULT_MODEL = "claude-sonnet-4-20250514"
DEFAULT_REPO_URL = "https://github.com/arni-labs/deep-sci-fi.git"


def derive_local_sandbox_url(base_url: str) -> str:
    parsed = urllib.parse.urlparse(base_url)
    scheme = parsed.scheme or "http"
    host = parsed.hostname or "127.0.0.1"
    if parsed.port is not None:
        port = parsed.port
    elif scheme == "https":
        port = 443
    else:
        port = 80
    return f"{scheme}://{host}:{port + 10}"


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
            # Keep the server-side wait chunk comfortably below urllib's
            # request timeout so the proof driver does not time out first.
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


def now_utc() -> str:
    return dt.datetime.now(dt.timezone.utc).replace(microsecond=0).isoformat()


def suffix() -> str:
    return dt.datetime.now(dt.timezone.utc).strftime("%Y%m%d%H%M%S")


def agent_entity_id(entity: dict[str, Any]) -> str:
    return str(entity.get("entity_id") or entity.get("Id") or "")


def build_alert_payload(repo_url: str, project_harness_id: str) -> str:
    payload = {
        "source": "synthetic-ci-monitor",
        "severity": "high",
        "summary": "deep-sci-fi platform clean install is broken",
        "project_harness_id": project_harness_id,
        "repo_url": repo_url,
        "reproduction": {
            "cwd": "platform",
            "command": "npm ci",
            "failure": "npm ci exits with EUSAGE because package.json and package-lock.json are out of sync",
        },
        "evidence": {
            "missing_packages": [
                "@types/d3",
                "d3",
                "react-markdown",
                "remark-gfm",
            ],
            "expected_fix": "refresh platform/package-lock.json so clean install succeeds again",
        },
        "notes": [
            "Treat this as a real issue, not monitor noise.",
            "Open a PR against arni-labs/deep-sci-fi once the lockfile drift is fixed.",
        ],
    }
    return json.dumps(payload, separators=(",", ":"))


def build_sre_message(
    *,
    repo_url: str,
    project_harness_id: str,
    monitor_id: str,
    alert_cycle_id: str,
    developer_sandbox_url: str,
) -> str:
    return f"""
You are handling a real self-healing remediation for deep-sci-fi.

Workflow entity IDs:
- ProjectHarness: {project_harness_id}
- Monitor: {monitor_id}
- AlertCycle: {alert_cycle_id}
- Repository: {repo_url}

Treat this as a real issue requiring remediation, not monitor tuning.

The issue is concrete and reproducible:
- In the deep-sci-fi repo, `cd platform && npm ci` currently fails.
- The error is that `package.json` and `package-lock.json` are out of sync.
- Missing packages reported locally include `@types/d3`, `d3`, `react-markdown`, and `remark-gfm`.

Follow this exact workflow:
1. Read the ProjectHarness, Monitor, and AlertCycle.
2. Create one WorkCycle tied to the ProjectHarness with a task summary about fixing deep-sci-fi platform lockfile drift.
3. Spawn exactly one Developer child agent with:
   - soul_id = Developer
   - tools = read,write,edit,bash,temper_get,temper_list,temper_action,read_entity
   - sandbox_url = {developer_sandbox_url}
   - workdir = /tmp/deep-sci-fi-self-heal
   - max_turns = 80
   - background = false
   - do not spawn a replacement developer unless the first developer reaches a terminal failed state
4. Give the Developer this concrete task:
   - clone {repo_url}
   - reproduce `cd platform && npm ci`
   - update the lockfile so `npm ci` succeeds again
   - if a full `npm install` is killed or hangs, do not loop on it; instead run `rm -rf node_modules` and then use a bounded lockfile refresh such as `timeout 120 npm install --package-lock-only --ignore-scripts --no-fund --no-audit`
   - after refreshing the lockfile, run `npm ci --no-fund --no-audit`
   - run at least one additional targeted validation command in `platform/` that actually exists, preferring a lighter command like `npm run typecheck` before heavier options
   - commit the fix
   - push a branch
   - open a PR against the upstream repository using the GitHub token available in the shell; if `gh` is unavailable, use `curl` against the GitHub REST API
   - report the exact PR URL
5. After the child agent finishes, read the WorkCycle and child result.
6. If the fix succeeded, call:
   - WorkCycle.BeginTesting
   - WorkCycle.PassTests with a short test summary covering `npm ci` and the additional validation command
   - WorkCycle.Approve with your sre agent ID as approver_id and the PR URL
   - AlertCycle.HealComplete with a diagnosis, PR URL, and the developer agent ID
7. If the fix did not succeed, call:
   - WorkCycle.Fail with the error_message
   - AlertCycle.Escalate with the diagnosis

Your final response must contain these exact keys on separate lines:
ALERT_CYCLE_STATUS=<status>
WORK_CYCLE_STATUS=<status>
PR_URL=<url or empty>
DEVELOPER_AGENT_ID=<id or empty>
""".strip()


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--base-url", default=os.getenv("OPENPAW_BASE_URL", DEFAULT_BASE_URL))
    parser.add_argument("--tenant", default=os.getenv("PAW_TENANT", DEFAULT_TENANT))
    parser.add_argument("--repo-url", default=DEFAULT_REPO_URL)
    parser.add_argument("--model", default=DEFAULT_MODEL)
    parser.add_argument("--timeout-ms", type=int, default=15 * 60 * 1000)
    parser.add_argument("--sandbox-url", default=os.getenv("OPENPAW_SANDBOX_URL"))
    args = parser.parse_args()
    sandbox_url = args.sandbox_url or derive_local_sandbox_url(args.base_url)

    client = ODataClient(
        base_url=args.base_url,
        tenant=args.tenant,
        api_key=os.getenv("TEMPER_API_KEY") or None,
    )

    run_suffix = suffix()

    project = client.create(
        "ProjectHarnesses",
        {"Id": f"deep-sci-fi-harness-{run_suffix}"},
    )
    project_id = agent_entity_id(project)
    print(f"PROJECT_HARNESS_ID={project_id}", flush=True)
    client.action(
        "ProjectHarnesses",
        project_id,
        "OpenPaw.Harness.Configure",
        {
            "repo_url": args.repo_url,
            "tech_stack": "Next.js frontend, Python backend",
            "conventions": "Prefer focused fixes, validate in platform/, open PR with concrete reproduction notes.",
        },
    )
    client.action(
        "ProjectHarnesses",
        project_id,
        "OpenPaw.Harness.Activate",
        {"last_activated_at": now_utc()},
    )

    monitor = client.create(
        "Monitors",
        {"Id": f"deep-sci-fi-monitor-{run_suffix}"},
    )
    monitor_id = agent_entity_id(monitor)
    print(f"MONITOR_ID={monitor_id}", flush=True)
    client.action(
        "Monitors",
        monitor_id,
        "OpenPaw.Heal.Configure",
        {
            "logfire_query": "synthetic:deep-sci-fi:npm-ci-lockfile-drift",
            "threshold": "1",
            "dd_monitor_id": f"synthetic-{run_suffix}",
        },
    )
    client.action("Monitors", monitor_id, "OpenPaw.Heal.Activate")

    alert_cycle = client.create(
        "AlertCycles",
        {"Id": f"deep-sci-fi-alert-{run_suffix}"},
    )
    alert_cycle_id = agent_entity_id(alert_cycle)
    print(f"ALERT_CYCLE_ID={alert_cycle_id}", flush=True)
    alert_payload = build_alert_payload(args.repo_url, project_id)
    print(f"SANDBOX_URL={sandbox_url}", flush=True)

    sre = client.create("Agents", {})
    sre_id = agent_entity_id(sre)
    print(f"SRE_AGENT_ID={sre_id}", flush=True)
    sre_message = build_sre_message(
        repo_url=args.repo_url,
        project_harness_id=project_id,
        monitor_id=monitor_id,
        alert_cycle_id=alert_cycle_id,
        developer_sandbox_url=sandbox_url,
    )
    client.action(
        "Agents",
        sre_id,
        "OpenPaw.Configure",
        {
            "model": args.model,
            "provider": "anthropic",
            "max_turns": "60",
            "tools_enabled": "temper_get,temper_list,temper_action,temper_create,spawn_agent,read_entity",
            "sandbox_url": sandbox_url,
            "workdir": "/tmp/openpaw-sre-self-heal",
            "soul_id": "SRE",
            "user_message": sre_message,
        },
    )

    client.action(
        "AlertCycles",
        alert_cycle_id,
        "OpenPaw.Heal.Open",
        {
            "monitor_id": monitor_id,
            "alert_payload": alert_payload,
            "sre_agent_id": sre_id,
        },
    )

    client.action("Monitors", monitor_id, "OpenPaw.Heal.AlertFired", {"last_alert_payload": alert_payload})
    client.action("Agents", sre_id, "OpenPaw.Provision")
    print("SRE_STATUS=Provisioned", flush=True)

    sre_wait = client.wait_for_agent(sre_id, args.timeout_ms)
    alert_cycle_state = client.get("AlertCycles", alert_cycle_id)
    work_cycles = client.list(
        "WorkCycles",
        filter_expr=f"project_harness_id eq '{project_id}'",
        orderby="sequence_nr desc",
        top=5,
    )
    child_agents = client.list(
        "Agents",
        filter_expr=f"parent_agent_id eq '{sre_id}'",
        orderby="sequence_nr desc",
        top=5,
    )

    summary = {
        "project_harness_id": project_id,
        "monitor_id": monitor_id,
        "alert_cycle_id": alert_cycle_id,
        "sre_agent_id": sre_id,
        "sre_wait": sre_wait,
        "alert_cycle": alert_cycle_state,
        "work_cycles": work_cycles,
        "child_agents": child_agents,
    }
    print(json.dumps(summary, indent=2))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
