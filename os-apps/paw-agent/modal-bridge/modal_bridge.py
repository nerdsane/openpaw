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
from datetime import datetime

app = modal.App("openpaw-sandbox-bridge")

sandbox_image = modal.Image.debian_slim(python_version="3.12").apt_install(
    "curl", "git", "jq", "build-essential"
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


def _ensure_dir(sb, path: str):
    target = path or "/"
    result = sb.exec("mkdir", "-p", target)
    result.wait()
    return result


@app.function(image=bridge_image, secrets=[modal.Secret.from_name("openpaw-bridge-auth")], timeout=600)
@modal.concurrent(max_inputs=100)
@modal.fastapi_endpoint(method="POST", label="openpaw-sandbox-bridge-create")
def create_sandbox(body: dict, authorization: str = ""):
    if not _auth_ok(authorization):
        return _err("unauthorized", 401)

    sb = modal.Sandbox.create(
        image=sandbox_image,
        cpu=body.get("cpus", 2.0),
        memory=body.get("memory_mb", 4096),
        timeout=body.get("timeout_seconds", 3600),
        app=app,
    )
    _ensure_dir(sb, "/workspace")
    return {"sandbox_id": sb.object_id, "status": "running", "created_at": datetime.utcnow().isoformat()}


@app.function(image=bridge_image, secrets=[modal.Secret.from_name("openpaw-bridge-auth")], timeout=30)
@modal.concurrent(max_inputs=100)
@modal.fastapi_endpoint(method="GET", label="openpaw-sandbox-bridge-health")
def health_check(sandbox_id: str, authorization: str = ""):
    if not _auth_ok(authorization):
        return _err("unauthorized", 401)
    sb = _sb(sandbox_id)
    if not sb:
        return {"ready": False, "error": "sandbox not found"}
    try:
        result = sb.exec("echo", "ok")
        stdout = result.stdout.read()
        return {"ready": stdout.strip() == "ok", "status": "running"}
    except Exception as e:
        return {"ready": False, "error": str(e)}


@app.function(image=bridge_image, secrets=[modal.Secret.from_name("openpaw-bridge-auth")], timeout=60)
@modal.concurrent(max_inputs=100)
@modal.fastapi_endpoint(method="GET", label="openpaw-sandbox-bridge-file-read")
def file_read(sandbox_id: str, path: str, authorization: str = ""):
    if not _auth_ok(authorization):
        return _err("unauthorized", 401)
    sb = _sb(sandbox_id)
    if not sb:
        return _err("sandbox not found", 404)
    try:
        f = sb.open(path, "r")
        content = f.read()
        f.close()
        return {"content": content, "path": path}
    except Exception as e:
        return _err(str(e), 500)


@app.function(image=bridge_image, secrets=[modal.Secret.from_name("openpaw-bridge-auth")], timeout=60)
@modal.concurrent(max_inputs=100)
@modal.fastapi_endpoint(method="POST", label="openpaw-sandbox-bridge-file-write")
def file_write(body: dict, authorization: str = ""):
    """POST /file-write — Body: {sandbox_id, path, content}"""
    if not _auth_ok(authorization):
        return _err("unauthorized", 401)
    sb = _sb(body.get("sandbox_id", ""))
    if not sb:
        return _err("sandbox not found", 404)
    try:
        path = body["path"]
        parent = os.path.dirname(path)
        if parent:
            _ensure_dir(sb, parent)
        f = sb.open(path, "w")
        f.write(body["content"])
        f.close()
        return {"ok": True, "path": path}
    except Exception as e:
        return _err(str(e), 500)


@app.function(image=bridge_image, secrets=[modal.Secret.from_name("openpaw-bridge-auth")], timeout=60)
@modal.concurrent(max_inputs=100)
@modal.fastapi_endpoint(method="DELETE", label="openpaw-sandbox-bridge-file-delete")
def file_delete(sandbox_id: str, path: str, authorization: str = ""):
    if not _auth_ok(authorization):
        return _err("unauthorized", 401)
    sb = _sb(sandbox_id)
    if not sb:
        return _err("sandbox not found", 404)
    try:
        result = sb.exec("rm", "-f", path)
        result.stdout.read()
        return {"ok": True, "path": path}
    except Exception as e:
        return _err(str(e), 500)


@app.function(image=bridge_image, secrets=[modal.Secret.from_name("openpaw-bridge-auth")], timeout=600)
@modal.concurrent(max_inputs=100)
@modal.fastapi_endpoint(method="POST", label="openpaw-sandbox-bridge-exec")
def exec_command(body: dict, authorization: str = ""):
    """POST /exec — Body: {sandbox_id, command, workdir}"""
    if not _auth_ok(authorization):
        return _err("unauthorized", 401)
    sb = _sb(body.get("sandbox_id", ""))
    if not sb:
        return _err("sandbox not found", 404)
    try:
        workdir = body.get("workdir", "/") or "/"
        command = body.get("command", "")
        _ensure_dir(sb, workdir)
        result = sb.exec("bash", "-lc", f"cd {shlex.quote(workdir)} && {command}")
        stdout = result.stdout.read()
        stderr = result.stderr.read()
        result.wait()
        return {
            "stdout": stdout,
            "stderr": stderr,
            "exit_code": result.returncode,
        }
    except Exception as e:
        return {"stdout": "", "stderr": str(e), "exit_code": -1}


@app.function(image=bridge_image, secrets=[modal.Secret.from_name("openpaw-bridge-auth")], timeout=30)
@modal.concurrent(max_inputs=100)
@modal.fastapi_endpoint(method="DELETE", label="openpaw-sandbox-bridge-terminate")
def terminate_sandbox(sandbox_id: str, authorization: str = ""):
    if not _auth_ok(authorization):
        return _err("unauthorized", 401)
    sb = _sb(sandbox_id)
    if not sb:
        return {"ok": True, "message": "already terminated"}
    try:
        sb.terminate()
        return {"ok": True, "sandbox_id": sandbox_id}
    except Exception as e:
        return _err(str(e), 500)
