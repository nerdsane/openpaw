#!/usr/bin/env python3
"""Prove curl-style threaded conversation continuity through Channel routing.

This script drives the OData API the same way a transport would:
1. Create a Channel with a local webhook collector.
2. Register an AgentRoute for that Channel.
3. Send two ReceiveMessage actions on the same thread.
4. Verify the second response recalls context from the first message.
5. Verify the ChannelSession was updated to a continuation agent that reuses
   the same session tree and points back to the first agent.
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
DEFAULT_MODEL = "claude-sonnet-4-6"


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

    def wait_for_status(
        self,
        entity_set: str,
        entity_id: str,
        expected_status: str,
        timeout_secs: float = 30.0,
    ) -> dict[str, Any]:
        deadline = time.time() + timeout_secs
        while time.time() < deadline:
            entity = self.get(entity_set, entity_id)
            status = nested_str(entity, ["Status", "status"])
            if status == expected_status:
                return entity
            time.sleep(0.25)
        raise TimeoutError(
            f"timed out waiting for {entity_set} {entity_id} to reach status {expected_status}"
        )


def entity_id(entity: dict[str, Any]) -> str:
    return str(entity.get("entity_id") or entity.get("Id") or "")


def nested_str(value: dict[str, Any], keys: list[str]) -> str:
    for key in keys:
        found = value.get(key)
        if isinstance(found, str):
            return found
    fields = value.get("fields")
    if isinstance(fields, dict):
        for key in keys:
            found = fields.get(key)
            if isinstance(found, str):
                return found
    return ""


class ReplyCollector:
    def __init__(self) -> None:
        self._queue: "queue.Queue[dict[str, Any]]" = queue.Queue()
        self.port = pick_free_port()
        collector = self

        class Handler(BaseHTTPRequestHandler):
            def do_POST(self) -> None:  # noqa: N802
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

            def log_message(self, format: str, *args: Any) -> None:  # noqa: A003
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

    def wait_for_reply(self, expected_thread_id: str, timeout_secs: float = 30.0) -> dict[str, Any]:
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


def require(condition: bool, message: str) -> None:
    if not condition:
        raise RuntimeError(message)


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--base-url", default=os.getenv("TEMPERPAW_BASE_URL", DEFAULT_BASE_URL))
    parser.add_argument("--tenant", default=os.getenv("PAW_TENANT", DEFAULT_TENANT))
    parser.add_argument("--model", default=DEFAULT_MODEL)
    parser.add_argument("--sandbox-url", default=os.getenv("TEMPERPAW_SANDBOX_URL"))
    args = parser.parse_args()

    sandbox_url = args.sandbox_url or derive_local_sandbox_url(args.base_url)
    client = ODataClient(
        base_url=args.base_url,
        tenant=args.tenant,
        api_key=os.getenv("TEMPER_API_KEY"),
    )

    collector = ReplyCollector()
    collector.start()

    channel_name = f"curl-proof-{int(time.time())}"
    thread_id = f"thread-{int(time.time())}"
    author_id = "proof-user"

    try:
        channel = client.create("Channels")
        channel_id = entity_id(channel)
        require(channel_id, "failed to create Channel entity")

        client.action(
            "Channels",
            channel_id,
            "Paw.Channel.Configure",
            {
                "channel_type": "webhook-proof",
                "channel_id": channel_name,
                "guild_id": "",
                "default_agent_config": "",
                "webhook_secret": "",
                "webhook_url": collector.url,
            },
        )
        client.action("Channels", channel_id, "Paw.Channel.Connect", {})
        client.wait_for_status("Channels", channel_id, "Connected", timeout_secs=10.0)

        route = client.create("AgentRoutes")
        route_id = entity_id(route)
        require(route_id, "failed to create AgentRoute entity")

        route_config = {
            "system_prompt": (
                "You are a conversation continuity proof agent. "
                "When the user asks you to remember a token, reply exactly "
                "`REMEMBERED <token>`. When the user asks what token they asked "
                "you to remember, reply exactly `RECALL <token>`. Use the "
                "conversation history and do not invent values."
            ),
            "model": args.model,
            "provider": "anthropic",
            "tools_enabled": "temper_read",
            "max_turns": "8",
            "workdir": "/tmp/channel-continuation-proof",
            "sandbox_url": sandbox_url,
            "temper_api_url": args.base_url.rstrip("/"),
            "max_follow_ups": "5",
        }
        client.action(
            "AgentRoutes",
            route_id,
            "Paw.Channel.Register",
            {
                "binding_tier": "channel",
                "channel_id": channel_name,
                "guild_id": "",
                "match_pattern": "",
                "agent_config": json.dumps(route_config, separators=(",", ":")),
                "soul_id": "",
            },
        )

        client.action(
            "Channels",
            channel_id,
            "Paw.Channel.ReceiveMessage",
            {
                "message_id": "msg-1",
                "author_id": author_id,
                "thread_id": thread_id,
                "content": "Remember this token: moon-biscuit-42",
            },
        )
        first_reply = collector.wait_for_reply(thread_id)
        first_agent_id = str(first_reply.get("agent_entity_id") or "")
        require(first_reply.get("content") == "REMEMBERED moon-biscuit-42", f"unexpected first reply: {first_reply}")
        require(first_agent_id, f"missing first agent id in reply: {first_reply}")

        sessions = client.list(
            "ChannelSessions",
            filter_expr=(
                f"Status eq 'Active' and channel_id eq '{channel_name}' and "
                f"thread_id eq '{thread_id}' and author_id eq '{author_id}'"
            ),
            top=5,
        )
        require(len(sessions) == 1, f"expected one active ChannelSession, found {len(sessions)}")
        session_id = entity_id(sessions[0])
        require(session_id, "active ChannelSession missing entity_id")

        first_agent = client.get("Sessions", first_agent_id)
        first_session_file_id = nested_str(first_agent, ["session_file_id", "SessionFileId"])
        require(first_session_file_id, "first agent missing session_file_id")

        client.action(
            "Channels",
            channel_id,
            "Paw.Channel.ReceiveMessage",
            {
                "message_id": "msg-2",
                "author_id": author_id,
                "thread_id": thread_id,
                "content": "What token did I ask you to remember?",
            },
        )
        second_reply = collector.wait_for_reply(thread_id)
        second_agent_id = str(second_reply.get("agent_entity_id") or "")
        require(second_reply.get("content") == "RECALL moon-biscuit-42", f"unexpected second reply: {second_reply}")
        require(second_agent_id, f"missing second agent id in reply: {second_reply}")
        require(second_agent_id != first_agent_id, "expected continuation to create a new agent after terminal first turn")

        second_agent = client.get("Sessions", second_agent_id)
        second_parent_agent_id = nested_str(second_agent, ["parent_session_id", "ParentSessionId"])
        second_session_file_id = nested_str(second_agent, ["session_file_id", "SessionFileId"])
        require(second_parent_agent_id == first_agent_id, "second agent parent_agent_id did not point at first agent")
        require(second_session_file_id == first_session_file_id, "continuation did not reuse the same session tree")

        updated_session = client.get("ChannelSessions", session_id)
        session_agent_id = nested_str(updated_session, ["agent_entity_id", "AgentEntityId"])
        require(session_agent_id == second_agent_id, "ChannelSession did not rebind to continuation agent")

        result = {
            "channel_id": channel_id,
            "channel_name": channel_name,
            "route_id": route_id,
            "session_id": session_id,
            "sandbox_url": sandbox_url,
            "first_reply": first_reply,
            "second_reply": second_reply,
            "first_agent_id": first_agent_id,
            "second_agent_id": second_agent_id,
            "second_parent_agent_id": second_parent_agent_id,
            "session_file_id": second_session_file_id,
            "session_continued": True,
        }
        print(json.dumps(result, indent=2, sort_keys=True))
        print("SESSION_CONTINUED=true")
        print(f"FIRST_AGENT_ID={first_agent_id}")
        print(f"SECOND_AGENT_ID={second_agent_id}")
        print(f"CHANNEL_SESSION_ID={session_id}")
        return 0
    finally:
        collector.close()


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except Exception as exc:  # pragma: no cover - proof script
        print(f"ERROR: {exc}", file=sys.stderr)
        raise
