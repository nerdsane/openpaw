#!/usr/bin/env python3
"""Deploy OpenPaw self-monitoring Datadog monitors.

Reads dd-monitors/openpaw-monitors.json and creates or updates each monitor
via the Datadog REST API.  Idempotent: finds existing monitors by name.

Requires DD_API_KEY and DD_APP_KEY in env (or .env file).
"""

import json
import os
import sys
from pathlib import Path

try:
    import requests
except ImportError:
    sys.exit("requests is required: pip install requests")


def load_env():
    """Best-effort .env loading."""
    env_path = Path(__file__).resolve().parent.parent / ".env"
    if env_path.exists():
        for line in env_path.read_text().splitlines():
            line = line.strip()
            if not line or line.startswith("#") or "=" not in line:
                continue
            key, _, value = line.partition("=")
            os.environ.setdefault(key.strip(), value.strip())


def main():
    load_env()

    api_key = os.environ.get("DD_API_KEY", "")
    app_key = os.environ.get("DD_APP_KEY", "")
    site = os.environ.get("DD_SITE", "datadoghq.com")

    if not api_key or not app_key:
        sys.exit("DD_API_KEY and DD_APP_KEY must be set")

    monitors_path = Path(__file__).resolve().parent.parent / "dd-monitors" / "openpaw-monitors.json"
    monitors = json.loads(monitors_path.read_text())

    base_url = f"https://api.{site}/api/v1"
    headers = {
        "DD-API-KEY": api_key,
        "DD-APPLICATION-KEY": app_key,
        "Content-Type": "application/json",
    }

    # Fetch existing monitors tagged with managed_by:openpaw
    resp = requests.get(
        f"{base_url}/monitor",
        headers=headers,
        params={"monitor_tags": "team:openpaw"},
    )
    resp.raise_for_status()
    existing_by_name = {m["name"]: m["id"] for m in resp.json()}

    for monitor in monitors:
        name = monitor["name"]
        if name in existing_by_name:
            monitor_id = existing_by_name[name]
            resp = requests.put(
                f"{base_url}/monitor/{monitor_id}",
                headers=headers,
                json=monitor,
            )
            resp.raise_for_status()
            print(f"Updated: {name} (id={monitor_id})")
        else:
            resp = requests.post(
                f"{base_url}/monitor",
                headers=headers,
                json=monitor,
            )
            resp.raise_for_status()
            monitor_id = resp.json().get("id", "unknown")
            print(f"Created: {name} (id={monitor_id})")


if __name__ == "__main__":
    main()
