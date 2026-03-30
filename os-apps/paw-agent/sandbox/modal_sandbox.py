#!/usr/bin/env python3
"""Modal sandbox bridge for OpenPaw.

The bridge exposes a small local HTTP API:
  GET  /health
  POST /v1/sandboxes
  DELETE /v1/sandboxes/<sandbox_id>

Each created sandbox runs the same `local_sandbox.py` server inside Modal.
The bridge returns the remote tunnel URL so the agent runtime can keep using
the normal local-sandbox HTTP protocol for file and process operations.
"""

from __future__ import annotations

import argparse
import json
import os
from pathlib import Path
import sys
import threading
import time
import traceback
import uuid
from http.server import BaseHTTPRequestHandler, HTTPServer
from urllib import request

try:
    import modal
except ImportError as exc:  # pragma: no cover - bootstrapped by helper script
    raise SystemExit(f"modal package is required: {exc}") from exc


REPO_ROOT = Path(__file__).resolve().parents[3]
LOCAL_SANDBOX = Path(__file__).resolve().with_name("local_sandbox.py")
DEFAULT_APP_NAME = "openpaw-sandbox"
DEFAULT_PORT = 8080
DEFAULT_WORKDIR = "/workspace"

_BRIDGE_PORT = 3478
_LOCK = threading.Lock()
_SANDBOXES: dict[str, dict[str, object]] = {}
_IMAGE = None


def _wait_for_remote_health(url: str, timeout_secs: int = 120) -> None:
    deadline = time.time() + timeout_secs
    last_error = "unknown error"
    while time.time() < deadline:
        try:
            with request.urlopen(f"{url}/health", timeout=10) as response:
                payload = json.loads(response.read().decode("utf-8"))
            if payload.get("status") == "ok":
                return
            last_error = f"unexpected health payload: {payload}"
        except Exception as exc:  # pragma: no cover - depends on remote startup timing
            last_error = str(exc)
        time.sleep(2)
    raise RuntimeError(f"timed out waiting for sandbox health at {url}: {last_error}")


def _modal_image():
    global _IMAGE
    if _IMAGE is None:
        _IMAGE = (
            modal.Image.debian_slim(python_version="3.11")
            .apt_install("git", "curl", "bash", "ca-certificates", "gnupg")
            .run_commands(
                "mkdir -p /etc/apt/keyrings",
                "curl -fsSL https://deb.nodesource.com/gpgkey/nodesource-repo.gpg.key | gpg --dearmor -o /etc/apt/keyrings/nodesource.gpg",
                "echo 'deb [signed-by=/etc/apt/keyrings/nodesource.gpg] https://deb.nodesource.com/node_20.x nodistro main' > /etc/apt/sources.list.d/nodesource.list",
                "apt-get update",
                "apt-get install -y nodejs",
                "node --version",
                "npm --version",
            )
            .add_local_file(str(LOCAL_SANDBOX), "/root/local_sandbox.py")
        )
    return _IMAGE


def _create_sandbox(config: dict[str, object]) -> dict[str, object]:
    timeout_secs = int(config.get("timeout_secs", 3600))
    idle_timeout_secs = int(config.get("idle_timeout_secs", 900))
    workdir = str(config.get("workdir", DEFAULT_WORKDIR))
    sandbox_port = int(config.get("port", DEFAULT_PORT))
    app_name = str(config.get("app_name", DEFAULT_APP_NAME))
    name_prefix = str(config.get("name_prefix", "openpaw"))
    sandbox_name = f"{name_prefix}-{uuid.uuid4().hex[:12]}"

    app = modal.App.lookup(app_name, create_if_missing=True)
    sandbox = modal.Sandbox.create(
        "python3",
        "/root/local_sandbox.py",
        "--port",
        str(sandbox_port),
        "--workdir",
        workdir,
        app=app,
        name=sandbox_name,
        image=_modal_image(),
        encrypted_ports=[sandbox_port],
        timeout=timeout_secs,
        idle_timeout=idle_timeout_secs,
        workdir=workdir,
    )
    tunnels = sandbox.tunnels(timeout=120)
    sandbox_url = tunnels[sandbox_port].url
    _wait_for_remote_health(sandbox_url, timeout_secs=120)

    payload = {
        "sandbox_id": sandbox.object_id,
        "sandbox_url": sandbox_url,
        "provider": "modal",
        "name": sandbox_name,
        "port": sandbox_port,
        "workdir": workdir,
    }
    with _LOCK:
        _SANDBOXES[sandbox.object_id] = {
            "sandbox": sandbox,
            "payload": payload,
        }
    return payload


class ModalSandboxHandler(BaseHTTPRequestHandler):
    def log_message(self, fmt: str, *args) -> None:  # pragma: no cover - operational logging
        sys.stderr.write(f"[modal-bridge] {fmt % args}\n")

    def _read_json_body(self) -> dict[str, object]:
        length = int(self.headers.get("Content-Length", "0") or "0")
        raw = self.rfile.read(length) if length > 0 else b"{}"
        return json.loads(raw.decode("utf-8"))

    def _send(self, status: int, payload: dict[str, object]) -> None:
        body = json.dumps(payload).encode("utf-8")
        self.send_response(status)
        self.send_header("Content-Type", "application/json")
        self.send_header("Content-Length", str(len(body)))
        self.end_headers()
        self.wfile.write(body)

    def do_GET(self) -> None:  # pragma: no cover - exercised in manual verification
        if self.path == "/health":
            with _LOCK:
                sandbox_count = len(_SANDBOXES)
            self._send(
                200,
                {
                    "status": "ok",
                    "provider": "modal",
                    "bridge_url": f"http://127.0.0.1:{_BRIDGE_PORT}",
                    "sandboxes": sandbox_count,
                },
            )
            return
        self._send(404, {"error": f"unknown path: {self.path}"})

    def do_POST(self) -> None:  # pragma: no cover - exercised in manual verification
        if self.path != "/v1/sandboxes":
            self._send(404, {"error": f"unknown path: {self.path}"})
            return
        try:
            payload = _create_sandbox(self._read_json_body())
        except Exception as exc:
            traceback.print_exc()
            self._send(500, {"error": str(exc)})
            return
        self._send(200, payload)

    def do_DELETE(self) -> None:  # pragma: no cover - exercised in manual verification
        prefix = "/v1/sandboxes/"
        if not self.path.startswith(prefix):
            self._send(404, {"error": f"unknown path: {self.path}"})
            return
        sandbox_id = self.path.removeprefix(prefix)
        with _LOCK:
            record = _SANDBOXES.pop(sandbox_id, None)
        if record is None:
            self._send(404, {"error": f"unknown sandbox: {sandbox_id}"})
            return
        sandbox = record.get("sandbox")
        try:
            if sandbox is not None:
                sandbox.terminate()
        except Exception as exc:
            self._send(500, {"error": str(exc)})
            return
        self._send(200, {"terminated": True, "sandbox_id": sandbox_id})


def main() -> int:
    global _BRIDGE_PORT
    parser = argparse.ArgumentParser(description="OpenPaw Modal sandbox bridge")
    parser.add_argument("--port", type=int, default=3478)
    args = parser.parse_args()
    _BRIDGE_PORT = args.port

    if not os.environ.get("MODAL_TOKEN_ID") or not os.environ.get("MODAL_TOKEN_SECRET"):
        raise SystemExit("MODAL_TOKEN_ID and MODAL_TOKEN_SECRET must be set")
    if not LOCAL_SANDBOX.exists():
        raise SystemExit(f"missing local sandbox script at {LOCAL_SANDBOX}")

    server = HTTPServer(("127.0.0.1", args.port), ModalSandboxHandler)
    print(f"Modal sandbox bridge listening on http://127.0.0.1:{args.port}")
    try:
        server.serve_forever()
    except KeyboardInterrupt:
        pass
    finally:
        with _LOCK:
            sandboxes = list(_SANDBOXES.values())
            _SANDBOXES.clear()
        for record in sandboxes:
            sandbox = record.get("sandbox")
            if sandbox is None:
                continue
            try:
                sandbox.terminate()
            except Exception:
                pass
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
