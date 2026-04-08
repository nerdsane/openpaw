#!/usr/bin/env python3
"""Deploy the OpenPaw Datadog dashboard.

Reads dd-dashboards/openpaw-overview.json and creates or updates the dashboard
via the Datadog REST API.  Idempotent: finds existing dashboard by title.

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

    dashboard_path = Path(__file__).resolve().parent.parent / "dd-dashboards" / "openpaw-overview.json"
    dashboard = json.loads(dashboard_path.read_text())
    title = dashboard["title"]

    base_url = f"https://api.{site}/api/v1"
    headers = {
        "DD-API-KEY": api_key,
        "DD-APPLICATION-KEY": app_key,
        "Content-Type": "application/json",
    }

    # Search for existing dashboard by title
    resp = requests.get(f"{base_url}/dashboard", headers=headers)
    resp.raise_for_status()
    existing = [
        d for d in resp.json().get("dashboards", []) if d.get("title") == title
    ]

    if existing:
        dash_id = existing[0]["id"]
        resp = requests.put(
            f"{base_url}/dashboard/{dash_id}",
            headers=headers,
            json=dashboard,
        )
        resp.raise_for_status()
        print(f"Updated dashboard: https://app.{site}/dashboard/{dash_id}")
    else:
        resp = requests.post(
            f"{base_url}/dashboard",
            headers=headers,
            json=dashboard,
        )
        resp.raise_for_status()
        dash_id = resp.json().get("id", "unknown")
        print(f"Created dashboard: https://app.{site}/dashboard/{dash_id}")


if __name__ == "__main__":
    main()
