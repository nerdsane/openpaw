#!/usr/bin/env python3
"""Launch or reuse a Modal sandbox that serves the OpenPaw sandbox HTTP API."""

from __future__ import annotations

import argparse
import importlib.util
import json
import os
from pathlib import Path
import subprocess
import sys
import time
import urllib.error
import urllib.request


REPO_ROOT = Path(__file__).resolve().parent.parent
LOCAL_SANDBOX = REPO_ROOT / "os-apps" / "paw-agent" / "sandbox" / "local_sandbox.py"
DEFAULT_VENV = Path(os.getenv("OPENPAW_MODAL_VENV", "/tmp/openpaw-modal-venv"))
NODE_RUNTIME_FLAVOR = "node20"


def load_dotenv() -> None:
    dotenv = REPO_ROOT / ".env"
    if not dotenv.exists():
        return
    for raw_line in dotenv.read_text().splitlines():
        line = raw_line.strip()
        if not line or line.startswith("#") or "=" not in line:
            continue
        key, value = line.split("=", 1)
        key = key.strip()
        value = value.strip().strip("'").strip('"')
        os.environ.setdefault(key, value)


def ensure_modal_sdk() -> None:
    if importlib.util.find_spec("modal") is not None:
        return
    venv_python = DEFAULT_VENV / "bin" / "python"
    venv_pip = DEFAULT_VENV / "bin" / "pip"
    if not venv_python.exists():
        subprocess.check_call([sys.executable, "-m", "venv", str(DEFAULT_VENV)])
    subprocess.check_call([str(venv_pip), "install", "modal"])
    os.execv(str(venv_python), [str(venv_python), __file__, *sys.argv[1:]])


def wait_for_health(url: str, timeout_secs: int = 120) -> None:
    deadline = time.time() + timeout_secs
    last_error: str | None = None
    while time.time() < deadline:
        try:
            with urllib.request.urlopen(f"{url}/health", timeout=10) as response:
                body = response.read().decode("utf-8")
            parsed = json.loads(body)
            if parsed.get("status") == "ok":
                return
            last_error = f"unexpected health payload: {parsed}"
        except (urllib.error.URLError, urllib.error.HTTPError, TimeoutError, json.JSONDecodeError) as exc:
            last_error = str(exc)
        time.sleep(2)
    raise RuntimeError(f"timed out waiting for Modal sandbox health at {url}: {last_error or 'unknown error'}")


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--port", type=int, default=8080)
    parser.add_argument("--workdir", default="/workspace")
    parser.add_argument("--name", default="openpaw-default")
    parser.add_argument("--app-name", default="openpaw-sandbox")
    parser.add_argument("--timeout-secs", type=int, default=3600)
    parser.add_argument("--idle-timeout-secs", type=int, default=900)
    parser.add_argument("--print-url", action="store_true")
    parser.add_argument("--json", action="store_true")
    args = parser.parse_args()
    sandbox_name = f"{args.name}-{NODE_RUNTIME_FLAVOR}"
    if not sandbox_name.endswith(f"-{args.port}"):
        sandbox_name = f"{sandbox_name}-{args.port}"

    load_dotenv()
    ensure_modal_sdk()

    import modal  # type: ignore

    modal_token_id = os.getenv("MODAL_TOKEN_ID", "").strip()
    modal_token_secret = os.getenv("MODAL_TOKEN_SECRET", "").strip()
    if not modal_token_id or not modal_token_secret:
        raise RuntimeError("MODAL_TOKEN_ID and MODAL_TOKEN_SECRET must be set")
    if not LOCAL_SANDBOX.exists():
        raise RuntimeError(f"missing sandbox server script at {LOCAL_SANDBOX}")

    app = modal.App.lookup(args.app_name, create_if_missing=True)
    image = (
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

    sandbox = None
    try:
        sandbox = modal.Sandbox.from_name(args.app_name, sandbox_name)
        tunnels = sandbox.tunnels(timeout=30)
        url = tunnels[args.port].url
        wait_for_health(url, timeout_secs=30)
    except Exception:
        sandbox = None

    if sandbox is None:
        try:
            sandbox = modal.Sandbox.create(
                "python3",
                "/root/local_sandbox.py",
                "--port",
                str(args.port),
                "--workdir",
                args.workdir,
                app=app,
                name=sandbox_name,
                image=image,
                encrypted_ports=[args.port],
                timeout=args.timeout_secs,
                idle_timeout=args.idle_timeout_secs,
                workdir=args.workdir,
            )
        except modal.exception.AlreadyExistsError:
            time.sleep(2)
            sandbox = modal.Sandbox.from_name(args.app_name, sandbox_name)
        tunnels = sandbox.tunnels(timeout=120)
        url = tunnels[args.port].url
        wait_for_health(url, timeout_secs=120)

    payload = {
        "sandbox_id": sandbox.object_id,
        "sandbox_url": url,
        "port": args.port,
        "app_name": args.app_name,
        "name": sandbox_name,
    }

    try:
        sandbox.detach()
    except Exception:
        pass

    if args.print_url:
        print(url)
    elif args.json:
        print(json.dumps(payload))
    else:
        print(json.dumps(payload, indent=2))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
