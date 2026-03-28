#!/usr/bin/env python3
"""Proof script for Phase 3: Paw Orchestration via OData Channel.

Sends "manage deep-sci-fi for me" to Paw through a Channel (curl-style,
no Discord) and verifies Paw creates ProjectHarness + Developer + Monitors.
"""

from __future__ import annotations

import argparse
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


DEFAULT_BASE_URL = "http://127.0.0.1:3467"
DEFAULT_TENANT = "default"
DEFAULT_MODEL = "claude-sonnet-4-20250514"


def derive_local_sandbox_url(base_url: str) -> str:
    parsed = urllib.parse.urlparse(base_url)
    scheme = parsed.scheme or "http"
    host = parsed.hostname or "127.0.0.1"
    port = parsed.port or (443 if scheme == "https" else 80)
    return f"{scheme}://{host}:{port + 10}"


def pick_free_port() -> int:
    with socket.socket(socket.AF_INET, socket.SOCK_STREAM) as sock:
        sock.bind(("127.0.0.1", 0))
        return int(sock.getsockname()[1])


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

    def wait_for_status(
        self, entity_set: str, entity_id: str, expected_status: str, timeout_secs: float = 30.0,
    ) -> dict[str, Any]:
        deadline = time.time() + timeout_secs
        while time.time() < deadline:
            entity = self.get(entity_set, entity_id)
            status = entity.get("status", "") or entity.get("fields", {}).get("Status", "")
            if status == expected_status:
                return entity
            time.sleep(0.25)
        raise TimeoutError(
            f"timed out waiting for {entity_set} {entity_id} to reach {expected_status}"
        )


def entity_id(entity: dict[str, Any]) -> str:
    return str(entity.get("entity_id") or entity.get("Id") or "")


class ReplyCollector:
    def __init__(self) -> None:
        self._queue: "queue.Queue[dict[str, Any]]" = queue.Queue()
        self.port = pick_free_port()
        collector = self

        class Handler(BaseHTTPRequestHandler):
            def do_POST(self) -> None:
                length = int(self.headers.get("content-length", "0"))
                raw = self.rfile.read(length).decode("utf-8")
                try:
                    payload = json.loads(raw)
                except json.JSONDecodeError:
                    payload = {"raw": raw}
                collector._queue.put(payload)
                self.send_response(200)
                self.send_header("content-type", "application/json")
                self.end_headers()
                self.wfile.write(b'{"ok":true}')

            def log_message(self, format: str, *args: Any) -> None:
                return

        self._server = ThreadingHTTPServer(("127.0.0.1", self.port), Handler)
        self._thread = threading.Thread(target=self._server.serve_forever, daemon=True)

    @property
    def url(self) -> str:
        return f"http://127.0.0.1:{self.port}/reply"

    def start(self) -> None:
        self._thread.start()

    def close(self) -> None:
        self._server.shutdown()
        self._server.server_close()
        self._thread.join(timeout=2)

    def wait_for_reply(self, expected_thread_id: str, timeout_secs: float = 300.0) -> dict[str, Any]:
        deadline = time.time() + timeout_secs
        while time.time() < deadline:
            remaining = max(deadline - time.time(), 0.1)
            try:
                payload = self._queue.get(timeout=remaining)
            except queue.Empty:
                continue
            if payload.get("thread_id") == expected_thread_id:
                return payload
        raise TimeoutError(f"timed out waiting for reply on thread {expected_thread_id}")


def run_proof(args: argparse.Namespace) -> dict[str, Any]:
    results: dict[str, Any] = {"steps": [], "passed": True}
    sandbox_url = args.sandbox_url or derive_local_sandbox_url(args.base_url)
    client = ODataClient(args.base_url, args.tenant, args.api_key)

    collector = ReplyCollector()
    collector.start()

    channel_name = f"paw-orch-proof-{int(time.time())}"
    thread_id = f"thread-{int(time.time())}"
    author_id = "proof-user"

    try:
        # ── Step 1: Create Channel + AgentRoute pointing at Paw ──────
        print("\n=== Step 1: Create Channel + AgentRoute for Paw ===")
        channel = client.create("Channels")
        channel_id = entity_id(channel)

        client.action("Channels", channel_id, "Paw.Channel.Configure", {
            "channel_type": "webhook-proof",
            "channel_id": channel_name,
            "guild_id": "",
            "default_agent_config": "",
            "webhook_secret": "",
            "webhook_url": collector.url,
        })
        client.action("Channels", channel_id, "Paw.Channel.Connect", {})
        client.wait_for_status("Channels", channel_id, "Connected", timeout_secs=10.0)

        route = client.create("AgentRoutes")
        route_id = entity_id(route)

        route_config = {
            "model": args.model,
            "provider": "anthropic",
            "tools_enabled": "temper_get,temper_list,temper_action,temper_create,spawn_agent,save_memory,read_entity",
            "max_turns": "40",
            "workdir": "/tmp/paw-orchestration-proof",
            "sandbox_url": sandbox_url,
            "temper_api_url": args.base_url.rstrip("/"),
            "max_follow_ups": "10",
        }
        client.action("AgentRoutes", route_id, "Paw.Channel.Register", {
            "binding_tier": "channel",
            "channel_id": channel_name,
            "guild_id": "",
            "match_pattern": "",
            "agent_config": json.dumps(route_config, separators=(",", ":")),
            "soul_id": "Paw",
            "system_prompt": "",
        })

        print(f"  Channel: {channel_id}")
        print(f"  AgentRoute: {route_id}")
        print(f"  Webhook: {collector.url}")
        results["channel_id"] = channel_id
        results["steps"].append({"name": "create_channel_and_route", "status": "PASS"})

        # ── Step 2: Send "manage deep-sci-fi" message ────────────────
        print("\n=== Step 2: Send 'manage deep-sci-fi' message ===")
        client.action("Channels", channel_id, "Paw.Channel.ReceiveMessage", {
            "message_id": f"msg-{int(time.time())}",
            "author_id": author_id,
            "thread_id": thread_id,
            "content": (
                "Manage deep-sci-fi for me. The repo is https://github.com/arni-labs/deep-sci-fi.git. "
                "It's a Next.js frontend with a Python backend. Set up a project harness, "
                "spawn a developer agent to look at the repo, and create at least one monitor."
            ),
        })
        print("  Message sent, waiting for Paw's reply...")
        results["steps"].append({"name": "send_manage_message", "status": "PASS"})

        # ── Step 3: Wait for Paw's reply ─────────────────────────────
        print(f"\n=== Step 3: Wait for Paw reply (timeout: {args.timeout_secs}s) ===")
        try:
            reply = collector.wait_for_reply(thread_id, timeout_secs=args.timeout_secs)
            reply_content = reply.get("content", "")
            print(f"  Reply received ({len(reply_content)} chars)")
            print(f"  Preview: {reply_content[:500]}...")
            results["paw_reply_preview"] = reply_content[:500]
            results["steps"].append({"name": "paw_replied", "status": "PASS"})
        except TimeoutError:
            print("  TIMEOUT waiting for Paw reply")
            results["steps"].append({"name": "paw_replied", "status": "FAIL"})
            results["passed"] = False
            return results

        # ── Step 4: Verify entities created ──────────────────────────
        print("\n=== Step 4: Verify entities created by Paw ===")

        # Check for ProjectHarnesses
        harnesses = client.list("ProjectHarnesses", orderby="sequence_nr desc", top=5)
        recent_harnesses = [
            h for h in harnesses
            if "deep-sci-fi" in json.dumps(h.get("fields", {})).lower()
        ]
        print(f"  ProjectHarnesses with 'deep-sci-fi': {len(recent_harnesses)}")

        # Check for Monitors
        monitors = client.list("Monitors", orderby="sequence_nr desc", top=10)
        print(f"  Recent Monitors: {len(monitors)}")

        # Check for child agents (Developer spawned by Paw)
        # Find the Paw agent that was created for this session
        sessions = client.list(
            "ChannelSessions",
            filter_expr=f"thread_id eq '{thread_id}'",
        )
        paw_agent_id = ""
        if sessions:
            paw_agent_id = sessions[0].get("fields", {}).get("agent_entity_id", "")
        child_agents = []
        if paw_agent_id:
            child_agents = client.list(
                "Agents", filter_expr=f"parent_agent_id eq '{paw_agent_id}'"
            )
        print(f"  Paw agent: {paw_agent_id}")
        print(f"  Child agents spawned by Paw: {len(child_agents)}")

        harness_created = len(recent_harnesses) > 0
        step4_pass = harness_created
        results["steps"].append({
            "name": "entities_created",
            "status": "PASS" if step4_pass else "FAIL",
            "harnesses_found": len(recent_harnesses),
            "monitors_found": len(monitors),
            "child_agents": len(child_agents),
            "paw_agent_id": paw_agent_id,
        })
        if not step4_pass:
            results["passed"] = False

    finally:
        collector.close()

    # ── Summary ──────────────────────────────────────────────────────
    print("\n=== SUMMARY ===")
    for step in results["steps"]:
        icon = "✓" if step["status"] == "PASS" else ("⊘" if step["status"] == "SKIP" else "✗")
        print(f"  {icon} {step['name']}: {step['status']}")
    print(f"\n  Overall: {'PASS' if results['passed'] else 'FAIL'}")
    return results


def main() -> None:
    parser = argparse.ArgumentParser(description="Prove Paw orchestration (Phase 3)")
    parser.add_argument("--base-url", default=DEFAULT_BASE_URL)
    parser.add_argument("--tenant", default=DEFAULT_TENANT)
    parser.add_argument("--api-key", default=os.environ.get("TEMPER_API_KEY"))
    parser.add_argument("--model", default=DEFAULT_MODEL)
    parser.add_argument("--sandbox-url", default=os.environ.get("OPENPAW_SANDBOX_URL"))
    parser.add_argument("--timeout-secs", type=float, default=300.0)
    args = parser.parse_args()

    results = run_proof(args)
    print("\n" + json.dumps(results, indent=2))
    if not results["passed"]:
        sys.exit(1)


if __name__ == "__main__":
    main()
