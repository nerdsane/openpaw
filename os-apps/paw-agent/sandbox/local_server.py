#!/usr/bin/env python3
"""Local sandbox HTTP server for Temper Agent E2E testing.

Implements both the legacy Temper Agent API and the current Tensorlake-style
API used by paw-agent:
  GET    /v1/fs/file?path=...        → read file
  PUT    /v1/fs/file?path=...        → write file
  POST   /v1/processes/run           → execute bash command
  GET    /api/v1/files?path=...      → read file
  PUT    /api/v1/files?path=...      → write file
  DELETE /api/v1/files?path=...      → delete file
  GET    /api/v1/files/list?path=... → list directory
  POST   /api/v1/processes           → execute process
  GET    /health                     → health check

Usage:
  python3 local_server.py [--port 9999] [--workdir /tmp/sandbox]
"""

import argparse
import json
import os
import subprocess
import sys
from http.server import HTTPServer, BaseHTTPRequestHandler
from urllib.parse import urlparse, parse_qs


class SandboxHandler(BaseHTTPRequestHandler):
    workdir: str = "/tmp/temper-sandbox"
    workspace_root: str = "/workspace"

    def do_GET(self):
        parsed = urlparse(self.path)
        if parsed.path == "/health":
            self._json_response(200, {"status": "ok"})
            return
        if parsed.path in {"/v1/fs/file", "/api/v1/files"}:
            params = parse_qs(parsed.query)
            file_path = params.get("path", [None])[0]
            if not file_path:
                self._json_response(400, {"error": "missing path parameter"})
                return
            full_path = self._resolve_path(file_path)
            if not os.path.isfile(full_path):
                self._json_response(404, {"error": f"file not found: {file_path}"})
                return
            try:
                with open(full_path, "r") as f:
                    content = f.read()
                self.send_response(200)
                self.send_header("Content-Type", "text/plain")
                self.end_headers()
                self.wfile.write(content.encode())
            except Exception as e:
                self._json_response(500, {"error": str(e)})
            return
        if parsed.path == "/api/v1/files/list":
            params = parse_qs(parsed.query)
            list_path = params.get("path", ["/"])[0]
            full_path = self._resolve_list_path(list_path)
            if not os.path.isdir(full_path):
                self._json_response(404, {"error": f"directory not found: {list_path}"})
                return
            try:
                entries = []
                for name in sorted(os.listdir(full_path)):
                    child = os.path.join(full_path, name)
                    entries.append(
                        {
                            "name": name,
                            "type": "directory" if os.path.isdir(child) else "file",
                        }
                    )
                self._json_response(200, {"entries": entries})
            except Exception as e:
                self._json_response(500, {"error": str(e)})
            return
        self._json_response(404, {"error": f"unknown path: {parsed.path}"})

    def do_PUT(self):
        parsed = urlparse(self.path)
        if parsed.path in {"/v1/fs/file", "/api/v1/files"}:
            params = parse_qs(parsed.query)
            file_path = params.get("path", [None])[0]
            if not file_path:
                self._json_response(400, {"error": "missing path parameter"})
                return
            full_path = self._resolve_path(file_path)
            content_length = int(self.headers.get("Content-Length", 0))
            body = self.rfile.read(content_length).decode() if content_length > 0 else ""
            try:
                os.makedirs(os.path.dirname(full_path), exist_ok=True)
                with open(full_path, "w") as f:
                    f.write(body)
                self._json_response(200, {"status": "ok", "path": file_path})
            except Exception as e:
                self._json_response(500, {"error": str(e)})
            return
        self._json_response(404, {"error": f"unknown path: {parsed.path}"})

    def do_DELETE(self):
        parsed = urlparse(self.path)
        if parsed.path == "/api/v1/files":
            params = parse_qs(parsed.query)
            file_path = params.get("path", [None])[0]
            if not file_path:
                self._json_response(400, {"error": "missing path parameter"})
                return
            full_path = self._resolve_path(file_path)
            try:
                if os.path.isdir(full_path):
                    os.rmdir(full_path)
                elif os.path.exists(full_path):
                    os.remove(full_path)
                self._json_response(200, {"status": "ok", "path": file_path})
            except FileNotFoundError:
                self._json_response(200, {"status": "ok", "path": file_path})
            except OSError as e:
                self._json_response(500, {"error": str(e)})
            return
        self._json_response(404, {"error": f"unknown path: {parsed.path}"})

    def do_POST(self):
        parsed = urlparse(self.path)
        if parsed.path in {"/v1/processes/run", "/api/v1/processes"}:
            content_length = int(self.headers.get("Content-Length", 0))
            body = self.rfile.read(content_length).decode() if content_length > 0 else "{}"
            try:
                req = json.loads(body)
            except json.JSONDecodeError as e:
                self._json_response(400, {"error": f"invalid JSON: {e}"})
                return
            cwd = req.get("workdir", self.workdir)
            extra_env = req.get("env", {})
            try:
                env = os.environ.copy()
                if isinstance(extra_env, dict):
                    env.update({str(k): str(v) for k, v in extra_env.items()})
                cwd = self._resolve_execution_path(cwd)
                os.makedirs(cwd, exist_ok=True)
                if parsed.path == "/api/v1/processes":
                    command = req.get("command", "")
                    args = req.get("args", [])
                    if not command:
                        self._json_response(400, {"error": "missing command"})
                        return
                    process = [self._rewrite_command_path(str(command))]
                    process.extend(self._rewrite_command_path(str(arg)) for arg in args)
                    result = subprocess.run(
                        process,
                        capture_output=True,
                        text=True,
                        timeout=30,
                        cwd=cwd,
                        env=env,
                    )
                    self._json_response(
                        200,
                        {
                            "id": "local-process",
                            "status": "completed",
                            "stdout": result.stdout,
                            "stderr": result.stderr,
                            "exit_code": result.returncode,
                        },
                    )
                else:
                    command = req.get("command", "")
                    if not command:
                        self._json_response(400, {"error": "missing command"})
                        return
                    command = self._rewrite_command_path(command)
                    result = subprocess.run(
                        command,
                        shell=True,
                        capture_output=True,
                        text=True,
                        timeout=30,
                        cwd=cwd,
                        env=env,
                    )
                    self._json_response(
                        200,
                        {
                            "stdout": result.stdout,
                            "stderr": result.stderr,
                            "exit_code": result.returncode,
                        },
                    )
            except subprocess.TimeoutExpired:
                self._json_response(
                    200,
                    {
                        "stdout": "",
                        "stderr": "command timed out after 30s",
                        "exit_code": -1,
                    },
                )
            except Exception as e:
                self._json_response(500, {"error": str(e)})
            return
        self._json_response(404, {"error": f"unknown path: {parsed.path}"})

    def _resolve_path(self, path: str) -> str:
        if path == "/" or path == "":
            return self.workdir
        if path == self.workspace_root or path.startswith(f"{self.workspace_root}/"):
            relative = path[len(self.workspace_root) :].lstrip("/")
            return os.path.join(self.workdir, "workspace", relative)
        if path == "/tmp" or path.startswith("/tmp/"):
            return path
        if path.startswith("/"):
            return os.path.join(self.workdir, path.lstrip("/"))
        return os.path.join(self.workdir, path)

    def _resolve_list_path(self, path: str) -> str:
        if path in {"", "/"}:
            return self.workdir
        return self._resolve_path(path)

    def _resolve_execution_path(self, path: str) -> str:
        if not path:
            return self.workdir
        return self._resolve_path(path)

    def _rewrite_command_path(self, value: str) -> str:
        mapped_workspace = self._resolve_path(self.workspace_root)
        return value.replace(self.workspace_root, mapped_workspace)

    def _json_response(self, status: int, data: dict):
        body = json.dumps(data).encode()
        self.send_response(status)
        self.send_header("Content-Type", "application/json")
        self.send_header("Content-Length", str(len(body)))
        self.end_headers()
        self.wfile.write(body)

    def log_message(self, format, *args):
        sys.stderr.write(f"[sandbox] {args[0]} {args[1]} {args[2]}\n")


def main():
    parser = argparse.ArgumentParser(description="Local sandbox server for Temper Agent")
    parser.add_argument("--port", type=int, default=9999, help="Port to listen on")
    parser.add_argument("--workdir", default="/tmp/temper-sandbox", help="Working directory")
    args = parser.parse_args()

    os.makedirs(args.workdir, exist_ok=True)
    SandboxHandler.workdir = args.workdir

    server = HTTPServer(("0.0.0.0", args.port), SandboxHandler)
    print(f"Local sandbox server listening on http://localhost:{args.port}")
    print(f"Working directory: {args.workdir}")
    try:
        server.serve_forever()
    except KeyboardInterrupt:
        print("\nShutting down.")
        server.server_close()


if __name__ == "__main__":
    main()
