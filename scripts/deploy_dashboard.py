#!/usr/bin/env python3
"""Deploy the TemperPaw Datadog dashboards.

By default deploys every *.json file in dd-dashboards/ via the Datadog
REST API. Idempotent: finds existing dashboard by title. Pass a specific
path as argv[1] to deploy just that file.

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


def deploy_one(path: Path, base_url: str, headers: dict, site: str) -> None:
    dashboard = json.loads(path.read_text())
    title = dashboard["title"]

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
        print(f"Updated {path.name}: https://app.{site}/dashboard/{dash_id}")
    else:
        resp = requests.post(
            f"{base_url}/dashboard",
            headers=headers,
            json=dashboard,
        )
        resp.raise_for_status()
        dash_id = resp.json().get("id", "unknown")
        print(f"Created {path.name}: https://app.{site}/dashboard/{dash_id}")


def main():
    load_env()

    api_key = os.environ.get("DD_API_KEY", "")
    app_key = os.environ.get("DD_APP_KEY", "")
    site = os.environ.get("DD_SITE", "datadoghq.com")

    if not api_key or not app_key:
        sys.exit("DD_API_KEY and DD_APP_KEY must be set")

    base_url = f"https://api.{site}/api/v1"
    headers = {
        "DD-API-KEY": api_key,
        "DD-APPLICATION-KEY": app_key,
        "Content-Type": "application/json",
    }

    dashboards_dir = Path(__file__).resolve().parent.parent / "dd-dashboards"
    if len(sys.argv) > 1:
        paths = [Path(sys.argv[1])]
    else:
        paths = sorted(dashboards_dir.glob("*.json"))

    if not paths:
        sys.exit(f"no dashboards found under {dashboards_dir}")

    for p in paths:
        deploy_one(p, base_url, headers, site)


if __name__ == "__main__":
    main()
