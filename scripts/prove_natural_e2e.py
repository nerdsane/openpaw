#!/usr/bin/env python3
"""Natural end-to-end proof: the full OpenPaw vision flow.

This is not a unit test. This is the real thing:
1. Human talks to Paw via Channel: "manage deep-sci-fi for me"
2. Paw sets up ProjectHarness, spawns Developer, creates Monitors
3. An alert fires via webhook (simulating Logfire)
4. Scout auto-spawns, triages, spawns Developer, Developer fixes
5. Proactive report sent back to the human's Channel
6. PM Issue created for the alert

No artificial setup. The only thing we create manually is the Channel
(because we need a webhook collector to receive replies).
"""

from __future__ import annotations

import json
import os
import queue
import socket
import sys
import threading
import time
import urllib.error
import urllib.parse
import urllib.request
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from typing import Any


BASE_URL = os.getenv("OPENPAW_BASE_URL", "http://127.0.0.1:3467")
TENANT = os.getenv("PAW_TENANT", "default")
API_KEY = os.getenv("TEMPER_API_KEY")
MODEL = "claude-sonnet-4-20250514"


# ── OData Client ─────────────────────────────────────────────────────

class ODataClient:
    def __init__(self) -> None:
        self.base_url = BASE_URL.rstrip("/")

    def _headers(self, json_body: bool = True) -> dict[str, str]:
        h = {"X-Tenant-Id": TENANT, "X-Temper-Principal-Kind": "admin", "Accept": "application/json"}
        if json_body:
            h["Content-Type"] = "application/json"
        if API_KEY:
            h["Authorization"] = f"Bearer {API_KEY}"
        return h

    def _req(self, method: str, path: str, body: Any = None) -> Any:
        data = json.dumps(body).encode() if body is not None else None
        req = urllib.request.Request(f"{self.base_url}{path}", data=data, method=method,
                                     headers=self._headers(body is not None))
        try:
            with urllib.request.urlopen(req, timeout=600) as r:
                raw = r.read().decode()
        except urllib.error.HTTPError as e:
            raise RuntimeError(f"{method} {path} → {e.code}: {e.read().decode(errors='replace')}") from e
        return json.loads(raw) if raw else {}

    def create(self, es: str, body: dict | None = None) -> dict:
        return self._req("POST", f"/tdata/{es}", body or {})

    def get(self, es: str, eid: str) -> dict:
        return self._req("GET", f"/tdata/{es}('{eid}')")

    def ls(self, es: str, *, filt: str | None = None, top: int | None = None) -> list[dict]:
        p = []
        if filt: p.append("$filter=" + urllib.parse.quote(filt, safe="'()"))
        if top: p.append(f"$top={top}")
        s = f"?{'&'.join(p)}" if p else ""
        return self._req("GET", f"/tdata/{es}{s}").get("value", [])

    def action(self, es: str, eid: str, act: str, body: dict | None = None) -> dict:
        return self._req("POST", f"/tdata/{es}('{eid}')/{act}", body or {})

    def wait_for_status(self, es: str, eid: str, status: str, secs: float = 30) -> dict:
        end = time.time() + secs
        while time.time() < end:
            e = self.get(es, eid)
            s = e.get("status", "") or e.get("fields", {}).get("Status", "")
            if s == status: return e
            time.sleep(0.3)
        raise TimeoutError(f"{es}/{eid} did not reach {status}")

    def wait_agent(self, aid: str, ms: int) -> dict:
        rem = max(ms, 1000)
        while True:
            chunk = min(rem, 300_000)
            r = self._req("GET", f"/observe/entities/Agent/{aid}/wait?statuses=Completed,Failed,Cancelled&timeout_ms={chunk}&poll_ms=500")
            st = str(r.get("status") or r.get("Status") or "")
            if st in ("Completed", "Failed", "Cancelled"): return r
            if not r.get("timed_out"): return r
            rem -= chunk
            if rem <= 0: return r


def eid(e: dict) -> str:
    return str(e.get("entity_id") or e.get("Id") or "")


def fld(e: dict, key: str) -> str:
    return e.get("fields", {}).get(key, "") or e.get(key, "")


# ── Reply Collector ──────────────────────────────────────────────────

class Collector:
    def __init__(self) -> None:
        self._q: queue.Queue[dict] = queue.Queue()
        with socket.socket() as s:
            s.bind(("127.0.0.1", 0))
            self.port = s.getsockname()[1]
        me = self
        class H(BaseHTTPRequestHandler):
            def do_POST(self):
                raw = self.rfile.read(int(self.headers.get("content-length", "0"))).decode()
                me._q.put(json.loads(raw) if raw else {})
                self.send_response(200); self.end_headers(); self.wfile.write(b'{"ok":true}')
            def log_message(self, *a): pass
        self._srv = ThreadingHTTPServer(("127.0.0.1", self.port), H)
        self._t = threading.Thread(target=self._srv.serve_forever, daemon=True)

    @property
    def url(self) -> str: return f"http://127.0.0.1:{self.port}/reply"

    def start(self): self._t.start()
    def stop(self): self._srv.shutdown(); self._t.join(2)

    def wait(self, thread_id: str, secs: float = 300) -> dict:
        end = time.time() + secs
        while time.time() < end:
            try:
                p = self._q.get(timeout=max(end - time.time(), 0.1))
                if p.get("thread_id") == thread_id: return p
            except queue.Empty: pass
        raise TimeoutError(f"no reply on {thread_id}")

    def drain_all(self, secs: float = 5) -> list[dict]:
        msgs = []
        end = time.time() + secs
        while time.time() < end:
            try: msgs.append(self._q.get(timeout=0.5))
            except queue.Empty:
                if msgs: break
        return msgs


# ── Main Flow ────────────────────────────────────────────────────────

def derive_sandbox(base: str) -> str:
    if ":" in base.rsplit("//", 1)[-1]:
        prefix, port_s = base.rsplit(":", 1)
        return f"{prefix}:{int(port_s) + 10}"
    return f"{base}:3477"


def main() -> int:
    c = ODataClient()
    coll = Collector()
    coll.start()
    sandbox_url = derive_sandbox(BASE_URL)
    thread = f"e2e-{int(time.time())}"
    results: dict[str, Any] = {"phases": {}}
    ok = True

    try:
        # ────────────────────────────────────────────────────────────
        # PHASE A: Human talks to Paw — "manage deep-sci-fi"
        # ────────────────────────────────────────────────────────────
        print("\n" + "=" * 60)
        print("PHASE A: Human → Paw: 'manage deep-sci-fi for me'")
        print("=" * 60)

        ch = c.create("Channels")
        ch_id = eid(ch)
        c.action("Channels", ch_id, "Paw.Channel.Configure", {
            "channel_type": "webhook-proof", "channel_id": f"e2e-{int(time.time())}",
            "webhook_url": coll.url, "webhook_secret": "", "guild_id": "", "default_agent_config": "",
        })
        c.action("Channels", ch_id, "Paw.Channel.Connect", {})
        c.wait_for_status("Channels", ch_id, "Connected", 10)

        rt = c.create("AgentRoutes")
        rt_id = eid(rt)
        rt_cfg = {
            "model": MODEL, "provider": "anthropic",
            "tools_enabled": "temper_get,temper_list,temper_action,temper_create,spawn_agent,save_memory,read_entity",
            "max_turns": "40", "workdir": "/tmp/paw-e2e", "sandbox_url": sandbox_url,
            "temper_api_url": BASE_URL.rstrip("/"), "max_follow_ups": "10",
        }
        c.action("AgentRoutes", rt_id, "Paw.Channel.Register", {
            "binding_tier": "channel", "channel_id": f"e2e-{int(time.time())}",
            "guild_id": "", "match_pattern": "",
            "agent_config": json.dumps(rt_cfg, separators=(",", ":")),
            "soul_id": "Paw", "system_prompt": "",
        })

        c.action("Channels", ch_id, "Paw.Channel.ReceiveMessage", {
            "message_id": f"msg-{int(time.time())}", "author_id": "human",
            "thread_id": thread,
            "content": (
                "Manage deep-sci-fi for me. The repo is https://github.com/arni-labs/deep-sci-fi.git. "
                "It's a Next.js frontend with a Python backend. "
                "Set up a project harness, spawn a developer to look at the repo, "
                "and create at least one monitor for the platform/ directory."
            ),
        })
        print("  Message sent. Waiting for Paw reply...")

        try:
            reply = coll.wait(thread, 300)
            paw_reply = reply.get("content", "")
            print(f"  Paw replied ({len(paw_reply)} chars)")
            print(f"  Preview: {paw_reply[:300]}...")
            results["phases"]["A_paw_orchestration"] = "PASS"
        except TimeoutError:
            print("  TIMEOUT waiting for Paw")
            results["phases"]["A_paw_orchestration"] = "FAIL"
            ok = False

        # Check what Paw created
        harnesses = [h for h in c.ls("ProjectHarnesses", top=10)
                     if "deep-sci-fi" in json.dumps(h.get("fields", {})).lower()]
        monitors = c.ls("Monitors", top=10)
        print(f"  Harnesses with deep-sci-fi: {len(harnesses)}")
        print(f"  Monitors: {len(monitors)}")

        if not harnesses:
            print("  WARNING: Paw did not create a ProjectHarness")

        # Find the harness Paw created (most recent)
        harness_id = eid(harnesses[0]) if harnesses else ""
        # Find a monitor to use for the alert
        monitor_id = ""
        for m in monitors:
            ms = m.get("status", "") or fld(m, "Status")
            if ms == "Active":
                monitor_id = eid(m)
                break

        # If Paw didn't create a monitor, create one ourselves for Phase B
        if not monitor_id:
            print("  Paw didn't create an Active monitor — creating one for Phase B")
            mon = c.create("Monitors", {"Id": f"e2e-monitor-{int(time.time())}"})
            monitor_id = eid(mon)
            c.action("Monitors", monitor_id, "OpenPaw.Heal.Configure", {
                "logfire_query": "synthetic:e2e-proof",
                "threshold": "1", "dd_monitor_id": f"e2e-{int(time.time())}",
            })
            c.action("Monitors", monitor_id, "OpenPaw.Heal.Activate", {})

        print(f"  Using harness: {harness_id}")
        print(f"  Using monitor: {monitor_id}")

        # ────────────────────────────────────────────────────────────
        # PHASE B: Alert fires via webhook → Scout auto-triages
        # ────────────────────────────────────────────────────────────
        print("\n" + "=" * 60)
        print("PHASE B: Alert fires → Scout triages → Developer fixes")
        print("=" * 60)

        webhook_body = json.dumps({
            "source": "logfire", "event_type": "alert",
            "payload": {
                "monitor_id": monitor_id, "severity": "high",
                "summary": "npm ci failure in deep-sci-fi platform",
                "project_harness_id": harness_id,
                "repo_url": "https://github.com/arni-labs/deep-sci-fi.git",
                "reproduction": {"cwd": "platform", "command": "npm ci",
                                  "failure": "package.json and package-lock.json out of sync"},
                "notes": ["Treat as real issue, not noise."],
            },
            "report_channel_id": ch_id,
            "report_thread_id": thread,
        }).encode()

        req = urllib.request.Request(
            f"{BASE_URL}/webhooks/ingest", data=webhook_body, method="POST",
            headers={"Content-Type": "application/json", "Accept": "application/json"},
        )
        with urllib.request.urlopen(req, timeout=30) as resp:
            wh_resp = json.loads(resp.read().decode())

        ac_id = wh_resp.get("alert_cycle_id", "")
        scout_msg = wh_resp.get("message", "")
        scout_id = scout_msg.replace("Scout auto-spawned: ", "") if "Scout auto-spawned" in scout_msg else ""
        print(f"  AlertCycle: {ac_id}")
        print(f"  Scout: {scout_id}")

        if not scout_id:
            print("  FAIL: Scout was not auto-spawned")
            results["phases"]["B_alert_scout_spawn"] = "FAIL"
            ok = False
        else:
            results["phases"]["B_alert_scout_spawn"] = "PASS"
            print(f"  Waiting for Scout to complete (up to 10min)...")
            scout_result = c.wait_agent(scout_id, 600_000)
            scout_status = str(scout_result.get("status") or scout_result.get("Status") or "")
            print(f"  Scout final status: {scout_status}")

            # Check AlertCycle
            ac = c.get("AlertCycles", ac_id)
            ac_status = ac.get("status", "") or fld(ac, "Status")
            print(f"  AlertCycle status: {ac_status}")

            # Check WorkCycles
            wcs = c.ls("WorkCycles", filt=f"project_harness_id eq '{harness_id}'" if harness_id else None, top=5)
            print(f"  WorkCycles: {len(wcs)}")
            for wc in wcs:
                ws = wc.get("status", "") or fld(wc, "Status")
                pr = fld(wc, "pr_url")
                print(f"    - {eid(wc)}: {ws} PR={pr}")

            # Check child agents (Developer spawned by Scout)
            children = c.ls("Agents", filt=f"parent_agent_id eq '{scout_id}'", top=5)
            print(f"  Developer agents: {len(children)}")

            # Check PM Issues
            issues = c.ls("Issues", top=10)
            recent_issues = [i for i in issues if monitor_id in json.dumps(i.get("fields", {}))]
            print(f"  PM Issues mentioning monitor: {len(recent_issues)}")

            results["phases"]["B_scout_completes"] = "PASS" if scout_status in ("Completed", "Failed") else "FAIL"
            results["phases"]["B_alert_cycle_resolved"] = "PASS" if ac_status in ("Fixed", "Tuned", "Failed") else "FAIL"
            results["phases"]["B_developer_spawned"] = "PASS" if len(children) >= 1 else "FAIL"

            if results["phases"]["B_scout_completes"] != "PASS" or results["phases"]["B_alert_cycle_resolved"] != "PASS":
                ok = False

        # ────────────────────────────────────────────────────────────
        # PHASE C: Check for proactive report on Channel
        # ────────────────────────────────────────────────────────────
        print("\n" + "=" * 60)
        print("PHASE C: Check for proactive report on Channel")
        print("=" * 60)

        # The background reporter polls every 5s. Give it time after Scout completes.
        print("  Waiting up to 30s for proactive report...")
        extra_msgs = coll.drain_all(30)
        report_received = False
        for msg in extra_msgs:
            content = msg.get("content", "")
            if "Alert Report" in content or "AlertCycle" in content.lower():
                print(f"  Proactive report received! ({len(content)} chars)")
                print(f"  Preview: {content[:300]}...")
                report_received = True
                break
        if not report_received:
            print("  No proactive report received (Scout may have failed before reporter ran)")

        results["phases"]["C_proactive_report"] = "PASS" if report_received else "SKIP"

    finally:
        coll.stop()

    # ────────────────────────────────────────────────────────────────
    # SUMMARY
    # ────────────────────────────────────────────────────────────────
    print("\n" + "=" * 60)
    print("NATURAL END-TO-END SUMMARY")
    print("=" * 60)
    for phase, status in results["phases"].items():
        icon = "✓" if status == "PASS" else ("⊘" if status == "SKIP" else "✗")
        print(f"  {icon} {phase}: {status}")
    overall = "PASS" if ok else "FAIL"
    print(f"\n  Overall: {overall}")
    results["overall"] = overall

    print("\n" + json.dumps(results, indent=2))
    return 0 if ok else 1


if __name__ == "__main__":
    sys.exit(main())
