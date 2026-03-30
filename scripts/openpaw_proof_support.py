#!/usr/bin/env python3
"""Shared helpers for Open Paw proof drivers."""

from __future__ import annotations

import datetime as dt
import hashlib
import hmac
import json
import queue
import socket
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


def derive_webhook_trigger_url(base_url: str) -> str:
    parsed = urllib.parse.urlparse(base_url)
    scheme = parsed.scheme or "http"
    host = parsed.hostname or "127.0.0.1"
    if parsed.port is not None:
        port = parsed.port
    elif scheme == "https":
        port = 443
    else:
        port = 80
    return f"{scheme}://{host}:{port + 12}"


def pick_free_port() -> int:
    with socket.socket(socket.AF_INET, socket.SOCK_STREAM) as sock:
        sock.bind(("127.0.0.1", 0))
        return int(sock.getsockname()[1])


def now_utc() -> str:
    return dt.datetime.now(dt.timezone.utc).replace(microsecond=0).isoformat()


def suffix() -> str:
    return dt.datetime.now(dt.timezone.utc).strftime("%Y%m%d%H%M%S")


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


def require(condition: bool, message: str) -> None:
    if not condition:
        raise RuntimeError(message)


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

    def get(self, entity_set: str, entity_id_value: str) -> dict[str, Any]:
        return self._request("GET", f"/tdata/{entity_set}('{entity_id_value}')")

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
        entity_id_value: str,
        action_name: str,
        body: dict[str, Any] | None = None,
    ) -> dict[str, Any]:
        return self._request(
            "POST",
            f"/tdata/{entity_set}('{entity_id_value}')/{action_name}",
            body or {},
        )

    def wait_for_status(
        self,
        entity_set: str,
        entity_id_value: str,
        expected_status: str,
        timeout_secs: float = 30.0,
    ) -> dict[str, Any]:
        deadline = time.time() + timeout_secs
        while time.time() < deadline:
            entity = self.get(entity_set, entity_id_value)
            status = nested_str(entity, ["Status", "status"])
            if status == expected_status:
                return entity
            time.sleep(0.25)
        raise TimeoutError(
            f"timed out waiting for {entity_set} {entity_id_value} to reach status {expected_status}"
        )

    def wait_for_agent(self, agent_id: str, timeout_ms: int) -> dict[str, Any]:
        remaining_ms = max(timeout_ms, 1_000)
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


def register_webhook_route(
    client: ODataClient,
    *,
    route_key: str,
    source_type: str,
    target_entity_type: str,
    target_action: str,
    webhook_secret: str = "",
    event_filter: str = "",
    monitor_resolution_enabled: bool = False,
    dedup_enabled: bool = False,
    dedup_window_minutes: int = 60,
) -> dict[str, Any]:
    route = client.create(
        "WebhookRoutes",
        {"Id": f"webhook-route-{route_key}"},
    )
    route_id = entity_id(route)
    require(route_id, f"failed to create WebhookRoute for {route_key}")
    client.action(
        "WebhookRoutes",
        route_id,
        "OpenPaw.Ingest.Register",
        {
            "route_key": route_key,
            "source_type": source_type,
            "event_filter": event_filter,
            "target_entity_type": target_entity_type,
            "target_action": target_action,
            "webhook_secret": webhook_secret,
            "monitor_resolution_enabled": "true" if monitor_resolution_enabled else "false",
            "dedup_enabled": "true" if dedup_enabled else "false",
            "dedup_window_minutes": str(dedup_window_minutes),
        },
    )
    return route


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

    def wait_for_reply(
        self,
        expected_thread_id: str,
        timeout_secs: float = 60.0,
    ) -> dict[str, Any]:
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


def webhook_post(
    base_url: str,
    body: dict[str, Any],
    *,
    route_key: str,
    source_type: str = "generic",
    secret: str | None = None,
    tamper_signature: bool = False,
) -> tuple[int, Any]:
    raw = json.dumps(body, separators=(",", ":")).encode("utf-8")
    headers = {
        "Content-Type": "application/json",
        "Accept": "application/json",
    }
    if secret is not None:
        digest = hmac.new(secret.encode("utf-8"), raw, hashlib.sha256).hexdigest()
        if tamper_signature:
            digest = "0" * len(digest)
        header_name = {
            "datadog": "X-Datadog-Signature",
            "github": "X-Hub-Signature-256",
        }.get(source_type, "X-Webhook-Signature")
        headers[header_name] = f"sha256={digest}"

    req = urllib.request.Request(
        (
            f"{derive_webhook_trigger_url(base_url).rstrip('/')}"
            f"/triggers/webhook/{urllib.parse.quote(route_key, safe='')}"
        ),
        data=raw,
        method="POST",
        headers=headers,
    )
    try:
        with urllib.request.urlopen(req, timeout=120) as resp:
            payload = resp.read().decode("utf-8")
            status = resp.getcode()
    except urllib.error.HTTPError as exc:
        payload = exc.read().decode("utf-8", errors="replace")
        status = exc.code
    try:
        return status, json.loads(payload) if payload else {}
    except json.JSONDecodeError:
        return status, payload
