#!/usr/bin/env python3
"""Launch or reuse the local Modal sandbox bridge used by OpenPaw."""

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
MODAL_BRIDGE = REPO_ROOT / "os-apps" / "paw-agent" / "sandbox" / "modal_sandbox.py"
DEFAULT_VENV = Path(os.getenv("OPENPAW_MODAL_VENV", "/tmp/openpaw-modal-venv"))


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
            if parsed.get("status") == "ok" and parsed.get("provider") == "modal":
                return
            last_error = f"unexpected health payload: {parsed}"
        except (urllib.error.URLError, urllib.error.HTTPError, TimeoutError, json.JSONDecodeError) as exc:
            last_error = str(exc)
        time.sleep(2)
    raise RuntimeError(f"timed out waiting for Modal sandbox health at {url}: {last_error or 'unknown error'}")


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--port", type=int, default=3478)
    parser.add_argument("--print-url", action="store_true")
    parser.add_argument("--json", action="store_true")
    args = parser.parse_args()

    load_dotenv()
    ensure_modal_sdk()

    modal_token_id = os.getenv("MODAL_TOKEN_ID", "").strip()
    modal_token_secret = os.getenv("MODAL_TOKEN_SECRET", "").strip()
    if not modal_token_id or not modal_token_secret:
        raise RuntimeError("MODAL_TOKEN_ID and MODAL_TOKEN_SECRET must be set")
    if not MODAL_BRIDGE.exists():
        raise RuntimeError(f"missing Modal bridge script at {MODAL_BRIDGE}")

    url = f"http://127.0.0.1:{args.port}"
    try:
        wait_for_health(url, timeout_secs=3)
    except Exception:
        python = DEFAULT_VENV / "bin" / "python"
        if not python.exists():
            python = Path(sys.executable)
        log_path = Path(f"/tmp/openpaw-modal-bridge-{args.port}.log")
        with log_path.open("ab") as log_file:
            subprocess.Popen(
                [str(python), str(MODAL_BRIDGE), "--port", str(args.port)],
                stdout=log_file,
                stderr=subprocess.STDOUT,
                cwd=str(REPO_ROOT),
                env=os.environ.copy(),
                start_new_session=True,
            )
        wait_for_health(url, timeout_secs=120)

    payload = {
        "bridge_url": url,
        "provider": "modal",
        "port": args.port,
    }

    if args.print_url:
        print(url)
    elif args.json:
        print(json.dumps(payload))
    else:
        print(json.dumps(payload, indent=2))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
