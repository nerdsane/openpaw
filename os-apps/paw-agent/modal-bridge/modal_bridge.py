"""
Modal REST Bridge — wraps the Modal Python SDK as REST endpoints for WASM sandbox abstraction.

Deploy: modal deploy modal_bridge.py
Auth:   Bearer token via BRIDGE_AUTH_TOKEN secret

The bridge creates Modal Sandboxes and proxies file/exec operations so that
Rust/WASM modules can interact with Modal sandboxes via standard HTTP.
"""

import modal
import os
import json
from datetime import datetime

app = modal.App("openpaw-sandbox-bridge")

# Default sandbox image — Debian with common dev tools
sandbox_image = modal.Image.debian_slim(python_version="3.12").apt_install(
    "curl", "git", "jq", "build-essential"
)


def verify_auth(headers: dict) -> bool:
    """Verify Bearer token from request headers."""
    expected = os.environ.get("BRIDGE_AUTH_TOKEN", "")
    if not expected:
        return True  # No token configured = open (dev mode)
    auth = headers.get("authorization", headers.get("Authorization", ""))
    return auth == f"Bearer {expected}"


# In-memory sandbox registry (Modal sandboxes are ephemeral per app instance)
_sandboxes: dict[str, modal.Sandbox] = {}


@app.function(
    secrets=[modal.Secret.from_name("openpaw-bridge-auth")],
    allow_concurrent_inputs=100,
    timeout=600,
)
@modal.web_endpoint(method="POST", label="openpaw-sandbox-bridge-create")
def create_sandbox(request: dict, headers: dict = {}):
    """POST /sandboxes — Create a new Modal sandbox."""
    if not verify_auth(headers):
        return {"error": "unauthorized"}, 401

    cpus = request.get("cpus", 2.0)
    memory_mb = request.get("memory_mb", 4096)
    timeout_seconds = request.get("timeout_seconds", 3600)

    sb = modal.Sandbox.create(
        image=sandbox_image,
        cpu=cpus,
        memory=memory_mb,
        timeout=timeout_seconds,
        app=app,
    )

    sandbox_id = sb.object_id
    _sandboxes[sandbox_id] = sb

    return {
        "sandbox_id": sandbox_id,
        "status": "running",
        "created_at": datetime.utcnow().isoformat(),
    }


@app.function(
    secrets=[modal.Secret.from_name("openpaw-bridge-auth")],
    allow_concurrent_inputs=100,
    timeout=30,
)
@modal.web_endpoint(method="GET", label="openpaw-sandbox-bridge-health")
def health_check(sandbox_id: str, headers: dict = {}):
    """GET /sandboxes/{id}/health — Check if sandbox is ready."""
    if not verify_auth(headers):
        return {"error": "unauthorized"}, 401

    sb = _sandboxes.get(sandbox_id)
    if not sb:
        # Try to reconnect to existing sandbox
        try:
            sb = modal.Sandbox.from_id(sandbox_id, app=app)
            _sandboxes[sandbox_id] = sb
        except Exception:
            return {"ready": False, "error": "sandbox not found"}

    try:
        result = sb.exec("echo", "ok")
        stdout = result.stdout.read()
        return {"ready": stdout.strip() == "ok", "status": "running"}
    except Exception as e:
        return {"ready": False, "error": str(e)}


@app.function(
    secrets=[modal.Secret.from_name("openpaw-bridge-auth")],
    allow_concurrent_inputs=100,
    timeout=60,
)
@modal.web_endpoint(method="GET", label="openpaw-sandbox-bridge-file-read")
def file_read(sandbox_id: str, path: str, headers: dict = {}):
    """GET /sandboxes/{id}/files?path=... — Read a file from sandbox."""
    if not verify_auth(headers):
        return {"error": "unauthorized"}, 401

    sb = _sandboxes.get(sandbox_id)
    if not sb:
        try:
            sb = modal.Sandbox.from_id(sandbox_id, app=app)
            _sandboxes[sandbox_id] = sb
        except Exception:
            return {"error": "sandbox not found"}, 404

    try:
        content = sb.read_file(path)
        return {"content": content, "path": path}
    except Exception as e:
        return {"error": str(e)}, 500


@app.function(
    secrets=[modal.Secret.from_name("openpaw-bridge-auth")],
    allow_concurrent_inputs=100,
    timeout=60,
)
@modal.web_endpoint(method="PUT", label="openpaw-sandbox-bridge-file-write")
def file_write(request: dict, headers: dict = {}):
    """PUT /sandboxes/{id}/files — Write a file to sandbox."""
    if not verify_auth(headers):
        return {"error": "unauthorized"}, 401

    sandbox_id = request.get("sandbox_id", "")
    path = request.get("path", "")
    content = request.get("content", "")

    sb = _sandboxes.get(sandbox_id)
    if not sb:
        try:
            sb = modal.Sandbox.from_id(sandbox_id, app=app)
            _sandboxes[sandbox_id] = sb
        except Exception:
            return {"error": "sandbox not found"}, 404

    try:
        sb.write_file(path, content)
        return {"ok": True, "path": path}
    except Exception as e:
        return {"error": str(e)}, 500


@app.function(
    secrets=[modal.Secret.from_name("openpaw-bridge-auth")],
    allow_concurrent_inputs=100,
    timeout=60,
)
@modal.web_endpoint(method="DELETE", label="openpaw-sandbox-bridge-file-delete")
def file_delete(sandbox_id: str, path: str, headers: dict = {}):
    """DELETE /sandboxes/{id}/files?path=... — Delete a file from sandbox."""
    if not verify_auth(headers):
        return {"error": "unauthorized"}, 401

    sb = _sandboxes.get(sandbox_id)
    if not sb:
        try:
            sb = modal.Sandbox.from_id(sandbox_id, app=app)
            _sandboxes[sandbox_id] = sb
        except Exception:
            return {"error": "sandbox not found"}, 404

    try:
        # Modal doesn't have a direct delete_file, use exec
        result = sb.exec("rm", "-f", path)
        result.stdout.read()  # wait for completion
        return {"ok": True, "path": path}
    except Exception as e:
        return {"error": str(e)}, 500


@app.function(
    secrets=[modal.Secret.from_name("openpaw-bridge-auth")],
    allow_concurrent_inputs=100,
    timeout=600,
)
@modal.web_endpoint(method="POST", label="openpaw-sandbox-bridge-exec")
def exec_command(request: dict, headers: dict = {}):
    """POST /sandboxes/{id}/exec — Execute a command in sandbox (synchronous)."""
    if not verify_auth(headers):
        return {"error": "unauthorized"}, 401

    sandbox_id = request.get("sandbox_id", "")
    command = request.get("command", "")
    workdir = request.get("workdir", "/")

    sb = _sandboxes.get(sandbox_id)
    if not sb:
        try:
            sb = modal.Sandbox.from_id(sandbox_id, app=app)
            _sandboxes[sandbox_id] = sb
        except Exception:
            return {"error": "sandbox not found"}, 404

    try:
        # Use bash -c to support complex commands and workdir
        full_command = f"cd {workdir} && {command}"
        result = sb.exec("bash", "-c", full_command)
        stdout = result.stdout.read()
        stderr = result.stderr.read()
        exit_code = result.returncode

        return {
            "stdout": stdout,
            "stderr": stderr,
            "exit_code": exit_code,
        }
    except Exception as e:
        return {
            "stdout": "",
            "stderr": str(e),
            "exit_code": -1,
        }


@app.function(
    secrets=[modal.Secret.from_name("openpaw-bridge-auth")],
    allow_concurrent_inputs=100,
    timeout=30,
)
@modal.web_endpoint(method="DELETE", label="openpaw-sandbox-bridge-terminate")
def terminate_sandbox(sandbox_id: str, headers: dict = {}):
    """DELETE /sandboxes/{id} — Terminate a sandbox."""
    if not verify_auth(headers):
        return {"error": "unauthorized"}, 401

    sb = _sandboxes.get(sandbox_id)
    if not sb:
        try:
            sb = modal.Sandbox.from_id(sandbox_id, app=app)
        except Exception:
            return {"ok": True, "message": "sandbox not found (may already be terminated)"}

    try:
        sb.terminate()
        _sandboxes.pop(sandbox_id, None)
        return {"ok": True, "sandbox_id": sandbox_id}
    except Exception as e:
        return {"error": str(e)}, 500
