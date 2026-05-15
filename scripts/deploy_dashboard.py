#!/usr/bin/env python3
"""Deploy the TemperPaw Datadog dashboards.

By default deploys every *.json file in dd-dashboards/ via the Datadog
REST API. Idempotent: finds existing dashboard by title. Pass a specific
path to deploy just that file. With --reconcile, stale legacy dashboards
owned by the TemperPaw migration are deleted after desired dashboards are
created/updated.

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

LEGACY_DASHBOARD_TERMS = (
    "OpenPaw",
    "OpenPAW",
    "openpaw",
    "service:openpaw",
    "slack-openpaw-alerts",
)


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


def fetch_dashboards(base_url: str, headers: dict) -> list[dict]:
    resp = requests.get(f"{base_url}/dashboard", headers=headers)
    resp.raise_for_status()
    return resp.json().get("dashboards", [])


def dashboard_search_blob(dashboard: dict) -> str:
    return json.dumps(dashboard, sort_keys=True)


def legacy_openpaw_dashboard(dashboard: dict) -> bool:
    blob = dashboard_search_blob(dashboard)
    return any(term in blob for term in LEGACY_DASHBOARD_TERMS)


def is_temperpaw_owned_dashboard(dashboard: dict, desired_titles: set[str]) -> bool:
    tags = set(dashboard.get("tags") or [])
    return (
        dashboard.get("title") in desired_titles
        or "team:temperpaw" in tags
        or legacy_openpaw_dashboard(dashboard)
    )


def deploy_one(
    path: Path,
    base_url: str,
    headers: dict,
    site: str,
    dashboards: list[dict],
) -> None:
    dashboard = json.loads(path.read_text())
    title = dashboard["title"]
    existing = [d for d in dashboards if d.get("title") == title]

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


def reconcile_dashboards(
    dashboards: list[dict],
    desired_titles: set[str],
    base_url: str,
    headers: dict,
) -> None:
    for dashboard in dashboards:
        dash_id = dashboard.get("id")
        title = dashboard.get("title", "")
        if not dash_id or title in desired_titles:
            continue
        if not is_temperpaw_owned_dashboard(dashboard, desired_titles):
            continue
        resp = requests.delete(f"{base_url}/dashboard/{dash_id}", headers=headers)
        resp.raise_for_status()
        print(f"Deleted legacy dashboard {dash_id}: {title}")


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

    args = sys.argv[1:]
    reconcile = "--reconcile" in args
    args = [arg for arg in args if arg != "--reconcile"]
    if len(args) > 1:
        sys.exit("usage: deploy_dashboard.py [--reconcile] [dashboard.json]")

    dashboards_dir = Path(__file__).resolve().parent.parent / "dd-dashboards"
    if args:
        paths = [Path(args[0])]
    else:
        paths = sorted(dashboards_dir.glob("*.json"))

    if not paths:
        sys.exit(f"no dashboards found under {dashboards_dir}")

    dashboards = fetch_dashboards(base_url, headers)
    desired_titles = {json.loads(path.read_text())["title"] for path in paths}
    protected_titles = desired_titles | {
        json.loads(path.read_text())["title"] for path in sorted(dashboards_dir.glob("*.json"))
    }

    for p in paths:
        deploy_one(p, base_url, headers, site, dashboards)

    if reconcile:
        reconcile_dashboards(dashboards, protected_titles, base_url, headers)


if __name__ == "__main__":
    main()
