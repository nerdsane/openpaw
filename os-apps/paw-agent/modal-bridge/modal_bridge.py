"""
Modal REST Bridge — wraps the Modal Python SDK as REST endpoints for WASM sandbox abstraction.

Deploy: modal deploy modal_bridge.py
Auth:   Bearer token via BRIDGE_AUTH_TOKEN secret (passed as query param `authorization`)

The bridge creates Modal Sandboxes and proxies file/exec operations so that
Rust/WASM modules can interact with Modal sandboxes via standard HTTP.
"""

import modal
import os
import shlex
import json
import time
from datetime import datetime

app = modal.App("temperpaw-sandbox-bridge")

PLAYWRIGHT_APT_DEPS = [
    "libglib2.0-0",
    "libnss3",
    "libnspr4",
    "libdbus-1-3",
    "libatk1.0-0",
    "libatk-bridge2.0-0",
    "libcups2",
    "libdrm2",
    "libxcb1",
    "libxkbcommon0",
    "libatspi2.0-0",
    "libx11-6",
    "libxcomposite1",
    "libxdamage1",
    "libxext6",
    "libxfixes3",
    "libxrandr2",
    "libgbm1",
    "libpango-1.0-0",
    "libcairo2",
    "libasound2",
]

sandbox_image = (
    modal.Image.debian_slim(python_version="3.12")
    .apt_install("curl", "git", "jq", "build-essential", *PLAYWRIGHT_APT_DEPS)
    .pip_install("playwright", "pillow")
    .run_commands("python -m playwright install chromium")
    # /workspace baked into the image: a post-create mkdir exec cost ~1.2s of
    # the bridged acquire path (measured 2026-07-23).
    .run_commands("mkdir -p /workspace")
)

bridge_image = modal.Image.debian_slim(python_version="3.12").pip_install("fastapi[standard]")


def _auth_ok(authorization: str) -> bool:
    expected = os.environ.get("BRIDGE_AUTH_TOKEN", "")
    if not expected:
        return True
    return authorization == f"Bearer {expected}"


def _sb(sandbox_id: str):
    try:
        return modal.Sandbox.from_id(sandbox_id)
    except Exception:
        return None


def _err(msg: str, status: int = 400):
    from fastapi.responses import JSONResponse
    return JSONResponse(content={"error": msg}, status_code=status)


def _duration_ms(started_at: float) -> int:
    return int((time.monotonic() - started_at) * 1000)


def _error_message(error: Exception) -> str:
    message = str(error)
    if len(message) > 500:
        return f"{message[:497]}..."
    return message


def _log_bridge_event(
    operation: str,
    endpoint: str,
    outcome: str,
    *,
    started_at: float,
    sandbox_id: str = "",
    status_code: int = 200,
    exit_code: int | None = None,
    workdir: str = "",
    path: str = "",
    error: Exception | None = None,
):
    fields = {
        "service": "temperpaw",
        "source": "modal_bridge",
        "observability_event": "temperpaw.sandbox",
        "sandbox_provider": "modal",
        "sandbox_id": sandbox_id or "",
        "sandbox": {
            "operation": operation,
            "outcome": outcome,
            "backend": "modal",
            "exit_code": -1 if exit_code is None else exit_code,
            "status_code": status_code,
            "workdir": workdir or "",
        },
        "modal_bridge": {
            "operation": operation,
            "endpoint": endpoint,
            "duration_ms": _duration_ms(started_at),
        },
    }
    if path:
        fields["sandbox"]["path"] = path
    if error is not None:
        fields["error"] = {
            "kind": error.__class__.__name__,
            "message": _error_message(error),
        }
    print(json.dumps(fields, sort_keys=True), flush=True)


def _ensure_dir(sb, path: str):
    target = path or "/"
    result = sb.exec("mkdir", "-p", target)
    result.wait()
    return result


def _sandbox_policy(body: dict) -> dict:
    return {
        "networking_type": body.get("networking_type", ""),
        "allowed_hosts": body.get("allowed_hosts", []),
        "allow_mcp_servers": body.get("allow_mcp_servers", False),
        "allow_package_managers": body.get("allow_package_managers", False),
        "packages": body.get("packages", []),
    }


def _write_policy_file(sb, body: dict):
    policy = _sandbox_policy(body)
    if not any(
        [
            policy["networking_type"],
            policy["allowed_hosts"],
            policy["allow_mcp_servers"],
            policy["allow_package_managers"],
            policy["packages"],
        ]
    ):
        return

    _ensure_dir(sb, "/workspace")
    f = sb.open("/workspace/.temperpaw-sandbox-config.json", "w")
    f.write(json.dumps(policy, indent=2))
    f.close()


@app.function(image=bridge_image, secrets=[modal.Secret.from_name("temperpaw-bridge-auth")], timeout=600, min_containers=1)
@modal.concurrent(max_inputs=100)
@modal.fastapi_endpoint(method="POST", label="temperpaw-sandbox-bridge-create")
def create_sandbox(body: dict, authorization: str = ""):
    started_at = time.monotonic()
    if not _auth_ok(authorization):
        _log_bridge_event(
            "create",
            "create",
            "unauthorized",
            started_at=started_at,
            status_code=401,
        )
        return _err("unauthorized", 401)

    try:
        sb = modal.Sandbox.create(
            image=sandbox_image,
            cpu=body.get("cpus", 2.0),
            memory=body.get("memory_mb", 4096),
            timeout=body.get("timeout_seconds", 3600),
            app=app,
        )
        # One combined post-create operation instead of two round-trips
        # (mkdir exec + policy file write each cost ~1s; measured 2026-07-23,
        # they dominated bridged acquire latency: 2.0s vs 0.16s native create).
        policy = _sandbox_policy(body)
        needs_policy = any(
            [
                policy["networking_type"],
                policy["allowed_hosts"],
                policy["allow_mcp_servers"],
                policy["allow_package_managers"],
                policy["packages"],
            ]
        )
        if needs_policy:
            script = (
                "mkdir -p /workspace && cat > /workspace/.temperpaw-sandbox-config.json <<'TPWEOF'\n"
                + json.dumps(policy, indent=2)
                + "\nTPWEOF"
            )
            sb.exec("bash", "-c", script).wait()
        # else: /workspace is baked into the sandbox image — no exec needed.
        _log_bridge_event(
            "create",
            "create",
            "success",
            started_at=started_at,
            sandbox_id=sb.object_id,
            status_code=200,
            workdir="/workspace",
        )
        return {
            "sandbox_id": sb.object_id,
            "status": "running",
            "created_at": datetime.utcnow().isoformat(),
        }
    except Exception as e:
        _log_bridge_event(
            "create",
            "create",
            "error",
            started_at=started_at,
            status_code=500,
            error=e,
        )
        return _err(str(e), 500)


@app.function(image=bridge_image, secrets=[modal.Secret.from_name("temperpaw-bridge-auth")], timeout=30)
@modal.concurrent(max_inputs=100)
@modal.fastapi_endpoint(method="GET", label="temperpaw-sandbox-bridge-health")
def health_check(sandbox_id: str, authorization: str = ""):
    started_at = time.monotonic()
    if not _auth_ok(authorization):
        _log_bridge_event(
            "health",
            "health",
            "unauthorized",
            started_at=started_at,
            sandbox_id=sandbox_id,
            status_code=401,
        )
        return _err("unauthorized", 401)
    sb = _sb(sandbox_id)
    if not sb:
        _log_bridge_event(
            "health",
            "health",
            "not_found",
            started_at=started_at,
            sandbox_id=sandbox_id,
            status_code=200,
        )
        return {"ready": False, "error": "sandbox not found"}
    try:
        result = sb.exec("echo", "ok")
        stdout = result.stdout.read()
        ready = stdout.strip() == "ok"
        _log_bridge_event(
            "health",
            "health",
            "ready" if ready else "not_ready",
            started_at=started_at,
            sandbox_id=sandbox_id,
            status_code=200,
        )
        return {"ready": ready, "status": "running"}
    except Exception as e:
        _log_bridge_event(
            "health",
            "health",
            "error",
            started_at=started_at,
            sandbox_id=sandbox_id,
            status_code=200,
            error=e,
        )
        return {"ready": False, "error": str(e)}


@app.function(image=bridge_image, secrets=[modal.Secret.from_name("temperpaw-bridge-auth")], timeout=60)
@modal.concurrent(max_inputs=100)
@modal.fastapi_endpoint(method="GET", label="temperpaw-sandbox-bridge-file-read")
def file_read(sandbox_id: str, path: str, authorization: str = ""):
    started_at = time.monotonic()
    if not _auth_ok(authorization):
        _log_bridge_event(
            "read",
            "file-read",
            "unauthorized",
            started_at=started_at,
            sandbox_id=sandbox_id,
            status_code=401,
            path=path,
        )
        return _err("unauthorized", 401)
    sb = _sb(sandbox_id)
    if not sb:
        _log_bridge_event(
            "read",
            "file-read",
            "not_found",
            started_at=started_at,
            sandbox_id=sandbox_id,
            status_code=404,
            path=path,
        )
        return _err("sandbox not found", 404)
    try:
        f = sb.open(path, "r")
        content = f.read()
        f.close()
        _log_bridge_event(
            "read",
            "file-read",
            "success",
            started_at=started_at,
            sandbox_id=sandbox_id,
            status_code=200,
            path=path,
        )
        return {"content": content, "path": path}
    except Exception as e:
        _log_bridge_event(
            "read",
            "file-read",
            "error",
            started_at=started_at,
            sandbox_id=sandbox_id,
            status_code=500,
            path=path,
            error=e,
        )
        return _err(str(e), 500)


@app.function(image=bridge_image, secrets=[modal.Secret.from_name("temperpaw-bridge-auth")], timeout=60)
@modal.concurrent(max_inputs=100)
@modal.fastapi_endpoint(method="POST", label="temperpaw-sandbox-bridge-file-write")
def file_write(body: dict, authorization: str = ""):
    """POST /file-write — Body: {sandbox_id, path, content}"""
    started_at = time.monotonic()
    sandbox_id = body.get("sandbox_id", "")
    path = body.get("path", "")
    if not _auth_ok(authorization):
        _log_bridge_event(
            "write",
            "file-write",
            "unauthorized",
            started_at=started_at,
            sandbox_id=sandbox_id,
            status_code=401,
            path=path,
        )
        return _err("unauthorized", 401)
    sb = _sb(sandbox_id)
    if not sb:
        _log_bridge_event(
            "write",
            "file-write",
            "not_found",
            started_at=started_at,
            sandbox_id=sandbox_id,
            status_code=404,
            path=path,
        )
        return _err("sandbox not found", 404)
    try:
        path = body["path"]
        parent = os.path.dirname(path)
        if parent:
            _ensure_dir(sb, parent)
        f = sb.open(path, "w")
        f.write(body["content"])
        f.close()
        _log_bridge_event(
            "write",
            "file-write",
            "success",
            started_at=started_at,
            sandbox_id=sandbox_id,
            status_code=200,
            path=path,
        )
        return {"ok": True, "path": path}
    except Exception as e:
        _log_bridge_event(
            "write",
            "file-write",
            "error",
            started_at=started_at,
            sandbox_id=sandbox_id,
            status_code=500,
            path=path,
            error=e,
        )
        return _err(str(e), 500)


@app.function(image=bridge_image, secrets=[modal.Secret.from_name("temperpaw-bridge-auth")], timeout=60)
@modal.concurrent(max_inputs=100)
@modal.fastapi_endpoint(method="DELETE", label="temperpaw-sandbox-bridge-file-delete")
def file_delete(sandbox_id: str, path: str, authorization: str = ""):
    started_at = time.monotonic()
    if not _auth_ok(authorization):
        _log_bridge_event(
            "delete",
            "file-delete",
            "unauthorized",
            started_at=started_at,
            sandbox_id=sandbox_id,
            status_code=401,
            path=path,
        )
        return _err("unauthorized", 401)
    sb = _sb(sandbox_id)
    if not sb:
        _log_bridge_event(
            "delete",
            "file-delete",
            "not_found",
            started_at=started_at,
            sandbox_id=sandbox_id,
            status_code=404,
            path=path,
        )
        return _err("sandbox not found", 404)
    try:
        result = sb.exec("rm", "-f", path)
        result.stdout.read()
        _log_bridge_event(
            "delete",
            "file-delete",
            "success",
            started_at=started_at,
            sandbox_id=sandbox_id,
            status_code=200,
            path=path,
        )
        return {"ok": True, "path": path}
    except Exception as e:
        _log_bridge_event(
            "delete",
            "file-delete",
            "error",
            started_at=started_at,
            sandbox_id=sandbox_id,
            status_code=500,
            path=path,
            error=e,
        )
        return _err(str(e), 500)


@app.function(image=bridge_image, secrets=[modal.Secret.from_name("temperpaw-bridge-auth")], timeout=600)
@modal.concurrent(max_inputs=100)
@modal.fastapi_endpoint(method="POST", label="temperpaw-sandbox-bridge-exec")
def exec_command(body: dict, authorization: str = ""):
    """POST /exec — Body: {sandbox_id, command, workdir}"""
    started_at = time.monotonic()
    sandbox_id = body.get("sandbox_id", "")
    workdir = body.get("workdir", "/") or "/"
    if not _auth_ok(authorization):
        _log_bridge_event(
            "bash",
            "exec",
            "unauthorized",
            started_at=started_at,
            sandbox_id=sandbox_id,
            status_code=401,
            workdir=workdir,
        )
        return _err("unauthorized", 401)
    sb = _sb(sandbox_id)
    if not sb:
        _log_bridge_event(
            "bash",
            "exec",
            "not_found",
            started_at=started_at,
            sandbox_id=sandbox_id,
            status_code=404,
            workdir=workdir,
        )
        return _err("sandbox not found", 404)
    try:
        command = body.get("command", "")
        _ensure_dir(sb, workdir)
        result = sb.exec("bash", "-lc", f"cd {shlex.quote(workdir)} && {command}")
        stdout = result.stdout.read()
        stderr = result.stderr.read()
        result.wait()
        exit_code = result.returncode
        _log_bridge_event(
            "bash",
            "exec",
            "success" if exit_code == 0 else "nonzero_exit",
            started_at=started_at,
            sandbox_id=sandbox_id,
            status_code=200,
            exit_code=exit_code,
            workdir=workdir,
        )
        return {
            "stdout": stdout,
            "stderr": stderr,
            "exit_code": exit_code,
        }
    except Exception as e:
        _log_bridge_event(
            "bash",
            "exec",
            "error",
            started_at=started_at,
            sandbox_id=sandbox_id,
            status_code=200,
            exit_code=-1,
            workdir=workdir,
            error=e,
        )
        return {"stdout": "", "stderr": str(e), "exit_code": -1}


@app.function(image=bridge_image, secrets=[modal.Secret.from_name("temperpaw-bridge-auth")], timeout=30)
@modal.concurrent(max_inputs=100)
@modal.fastapi_endpoint(method="DELETE", label="temperpaw-sandbox-bridge-terminate")
def terminate_sandbox(sandbox_id: str, authorization: str = ""):
    started_at = time.monotonic()
    if not _auth_ok(authorization):
        _log_bridge_event(
            "terminate",
            "terminate",
            "unauthorized",
            started_at=started_at,
            sandbox_id=sandbox_id,
            status_code=401,
        )
        return _err("unauthorized", 401)
    sb = _sb(sandbox_id)
    if not sb:
        _log_bridge_event(
            "terminate",
            "terminate",
            "already_terminated",
            started_at=started_at,
            sandbox_id=sandbox_id,
            status_code=200,
        )
        return {"ok": True, "message": "already terminated"}
    try:
        sb.terminate()
        _log_bridge_event(
            "terminate",
            "terminate",
            "success",
            started_at=started_at,
            sandbox_id=sandbox_id,
            status_code=200,
        )
        return {"ok": True, "sandbox_id": sandbox_id}
    except Exception as e:
        _log_bridge_event(
            "terminate",
            "terminate",
            "error",
            started_at=started_at,
            sandbox_id=sandbox_id,
            status_code=500,
            error=e,
        )
        return _err(str(e), 500)
