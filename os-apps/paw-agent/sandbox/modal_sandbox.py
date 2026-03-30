#!/usr/bin/env python3
"""Modal Sandbox Bridge — HTTP API for Modal sandbox operations.

Provides the same interface as local_sandbox.py but provisions and proxies
operations to a Modal sandbox. The WASM sandbox_provisioner calls this bridge
to create sandboxes and interact with them.

Usage:
    python3 modal_sandbox.py --port 3478

Requires:
    MODAL_TOKEN_ID and MODAL_TOKEN_SECRET env vars set.
    pip install modal (in a venv if needed)

Endpoints:
    POST /v1/sandboxes          Create a new sandbox (returns sandbox_url, sandbox_id)
    GET  /health                Health check
    GET  /v1/fs/file?path=...   Read file from active sandbox
    PUT  /v1/fs/file?path=...   Write file to active sandbox
    POST /v1/processes/run      Run command in active sandbox
    DELETE /v1/sandboxes/{id}   Terminate sandbox
"""

import argparse
import json
import os
import sys
import threading
import traceback
from http.server import HTTPServer, BaseHTTPRequestHandler
from urllib.parse import urlparse, parse_qs

# Modal SDK import (requires pip install modal)
try:
    import modal
except ImportError:
    print("ERROR: modal package not installed. Run: pip install modal", file=sys.stderr)
    sys.exit(1)

# Active sandboxes keyed by sandbox_id
_sandboxes = {}
_lock = threading.Lock()


class ModalSandboxHandler(BaseHTTPRequestHandler):
    def log_message(self, fmt, *args):
        sys.stderr.write(f"[modal_sandbox] {fmt % args}\n")

    def _send_json(self, status, data):
        body = json.dumps(data).encode()
        self.send_response(status)
        self.send_header("Content-Type", "application/json")
        self.send_header("Content-Length", len(body))
        self.end_headers()
        self.wfile.write(body)

    def _send_text(self, status, text):
        body = text.encode() if isinstance(text, str) else text
        self.send_response(status)
        self.send_header("Content-Type", "text/plain")
        self.send_header("Content-Length", len(body))
        self.end_headers()
        self.wfile.write(body)

    def _read_body(self):
        length = int(self.headers.get("Content-Length", 0))
        return self.rfile.read(length) if length > 0 else b""

    def do_GET(self):
        parsed = urlparse(self.path)
        if parsed.path == "/health":
            self._send_json(200, {"status": "ok", "provider": "modal", "sandboxes": len(_sandboxes)})
            return

        if parsed.path == "/v1/fs/file":
            qs = parse_qs(parsed.query)
            path = qs.get("path", [""])[0]
            sandbox_id = qs.get("sandbox_id", [""])[0] or self._default_sandbox_id()
            if not sandbox_id or sandbox_id not in _sandboxes:
                self._send_json(404, {"error": "no active sandbox"})
                return
            sb = _sandboxes[sandbox_id]
            try:
                content = sb.read_file(path)
                self._send_text(200, content)
            except Exception as e:
                self._send_json(500, {"error": str(e)})
            return

        self._send_json(404, {"error": f"unknown path: {parsed.path}"})

    def do_PUT(self):
        parsed = urlparse(self.path)
        if parsed.path == "/v1/fs/file":
            qs = parse_qs(parsed.query)
            path = qs.get("path", [""])[0]
            sandbox_id = qs.get("sandbox_id", [""])[0] or self._default_sandbox_id()
            body = self._read_body()
            if not sandbox_id or sandbox_id not in _sandboxes:
                self._send_json(404, {"error": "no active sandbox"})
                return
            sb = _sandboxes[sandbox_id]
            try:
                sb.write_file(path, body.decode())
                self._send_json(200, {"ok": True})
            except Exception as e:
                self._send_json(500, {"error": str(e)})
            return

        self._send_json(404, {"error": f"unknown path: {parsed.path}"})

    def do_POST(self):
        parsed = urlparse(self.path)

        if parsed.path == "/v1/sandboxes":
            body = json.loads(self._read_body() or b"{}")
            try:
                result = _create_sandbox(body)
                self._send_json(200, result)
            except Exception as e:
                traceback.print_exc()
                self._send_json(500, {"error": str(e)})
            return

        if parsed.path == "/v1/processes/run":
            body = json.loads(self._read_body() or b"{}")
            qs = parse_qs(parsed.query)
            sandbox_id = qs.get("sandbox_id", [""])[0] or body.get("sandbox_id", "") or self._default_sandbox_id()
            if not sandbox_id or sandbox_id not in _sandboxes:
                self._send_json(404, {"error": "no active sandbox"})
                return
            sb = _sandboxes[sandbox_id]
            command = body.get("command", body.get("cmd", "echo no command"))
            workdir = body.get("workdir", "/workspace")
            timeout = body.get("timeout", 60)
            try:
                result = _exec_in_sandbox(sb, command, workdir, timeout)
                self._send_json(200, result)
            except Exception as e:
                self._send_json(500, {"error": str(e)})
            return

        self._send_json(404, {"error": f"unknown path: {parsed.path}"})

    def do_DELETE(self):
        parsed = urlparse(self.path)
        parts = parsed.path.strip("/").split("/")
        if len(parts) >= 3 and parts[0] == "v1" and parts[1] == "sandboxes":
            sandbox_id = parts[2]
            with _lock:
                sb = _sandboxes.pop(sandbox_id, None)
            if sb:
                try:
                    sb.terminate()
                except Exception:
                    pass
                self._send_json(200, {"terminated": True})
            else:
                self._send_json(404, {"error": "sandbox not found"})
            return
        self._send_json(404, {"error": f"unknown path: {parsed.path}"})

    def _default_sandbox_id(self):
        with _lock:
            if _sandboxes:
                return next(iter(_sandboxes))
        return ""


def _create_sandbox(config):
    """Create a Modal sandbox and return connection details."""
    timeout = config.get("timeout_secs", config.get("timeout", 600))

    app = modal.App.lookup("openpaw-sandbox", create_if_missing=True)
    image = (
        modal.Image.debian_slim(python_version="3.12")
        .apt_install("git", "curl", "nodejs", "npm")
        .pip_install("requests")
    )

    sb = modal.Sandbox.create(
        app=app,
        image=image,
        timeout=timeout,
        encrypted_ports=[8080],
        workdir="/workspace",
    )

    sandbox_id = sb.object_id
    tunnels = sb.tunnels()
    tunnel_url = ""
    if tunnels:
        first_tunnel = next(iter(tunnels.values()))
        tunnel_url = first_tunnel.url

    with _lock:
        _sandboxes[sandbox_id] = sb

    return {
        "sandbox_id": sandbox_id,
        "sandbox_url": f"http://127.0.0.1:{_port}",
        "tunnel_url": tunnel_url,
        "provider": "modal",
    }


def _exec_in_sandbox(sb, command, workdir, timeout):
    """Execute a command in a Modal sandbox."""
    # Modal exec takes args as separate strings
    if isinstance(command, str):
        args = ["bash", "-c", f"cd {workdir} && {command}"]
    else:
        args = command

    result = sb.exec(*args)
    result.wait()

    stdout = result.stdout.read()
    stderr = result.stderr.read()

    return {
        "exit_code": result.returncode,
        "stdout": stdout,
        "stderr": stderr,
    }


_port = 3478


def main():
    global _port
    parser = argparse.ArgumentParser(description="Modal Sandbox Bridge")
    parser.add_argument("--port", type=int, default=3478)
    args = parser.parse_args()
    _port = args.port

    if not os.environ.get("MODAL_TOKEN_ID") or not os.environ.get("MODAL_TOKEN_SECRET"):
        print("ERROR: MODAL_TOKEN_ID and MODAL_TOKEN_SECRET must be set", file=sys.stderr)
        sys.exit(1)

    server = HTTPServer(("127.0.0.1", args.port), ModalSandboxHandler)
    print(f"Modal sandbox bridge listening on http://127.0.0.1:{args.port}")
    try:
        server.serve_forever()
    except KeyboardInterrupt:
        pass
    finally:
        # Cleanup active sandboxes
        with _lock:
            for sb in _sandboxes.values():
                try:
                    sb.terminate()
                except Exception:
                    pass


if __name__ == "__main__":
    main()
