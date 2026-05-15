#!/usr/bin/env python3
"""Deploy TemperPaw self-monitoring Datadog monitors.

Reads dd-monitors/temperpaw-monitors.json and creates or updates each monitor
via the Datadog REST API. Idempotent: finds existing monitors by name.

With --reconcile, also deletes TemperPaw-owned monitors that are NOT in the JSON
file. Ownership is detected by current team tags, desired monitor names, and
legacy OpenPaw identity in names, queries, messages, or notification routes.
This is the source-of-truth guarantee declared in ADR-0052: file-first, Datadog
state reconciles to match.

Requires DD_API_KEY and DD_APP_KEY in env (or .env file).
"""

import argparse
import json
import os
import sys
from pathlib import Path

try:
    import requests
except ImportError:
    sys.exit("requests is required: pip install requests")

TEAM_TAG = "team:temperpaw"
LEGACY_OPENPAW_MONITOR_TERMS = (
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


def monitor_search_text(monitor: dict) -> str:
    return "\n".join(
        [
            str(monitor.get("name", "")),
            str(monitor.get("message", "")),
            str(monitor.get("query", "")),
            json.dumps(monitor.get("tags", []), sort_keys=True),
            json.dumps(monitor.get("notifications", []), sort_keys=True),
        ]
    )


def legacy_openpaw_monitor(monitor: dict) -> bool:
    text = monitor_search_text(monitor)
    return any(term in text for term in LEGACY_OPENPAW_MONITOR_TERMS)


def is_temperpaw_owned_monitor(monitor: dict, desired_names: set[str]) -> bool:
    tags = set(monitor.get("tags") or [])
    return (
        monitor.get("name") in desired_names
        or TEAM_TAG in tags
        or legacy_openpaw_monitor(monitor)
    )


def raise_for_status(resp: requests.Response, action: str):
    if resp.ok:
        return
    body = resp.text.strip()
    detail = f": {body}" if body else ""
    raise requests.HTTPError(
        f"{action} failed with {resp.status_code} {resp.reason}{detail}",
        response=resp,
    )


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--reconcile",
        action="store_true",
        help="Delete TemperPaw-owned monitors that are not in the JSON file.",
    )
    parser.add_argument(
        "--dry-run",
        action="store_true",
        help="Show what would happen without making changes.",
    )
    args = parser.parse_args()

    load_env()

    api_key = os.environ.get("DD_API_KEY", "")
    app_key = os.environ.get("DD_APP_KEY", "")
    site = os.environ.get("DD_SITE", "datadoghq.com")

    if not api_key or not app_key:
        sys.exit("DD_API_KEY and DD_APP_KEY must be set")

    monitors_path = Path(__file__).resolve().parent.parent / "dd-monitors" / "temperpaw-monitors.json"
    monitors = json.loads(monitors_path.read_text())
    desired_names = {m["name"] for m in monitors}

    base_url = f"https://api.{site}/api/v1"
    headers = {
        "DD-API-KEY": api_key,
        "DD-APPLICATION-KEY": app_key,
        "Content-Type": "application/json",
    }

    resp = requests.get(f"{base_url}/monitor", headers=headers)
    raise_for_status(resp, "List monitors")
    existing_monitors = [
        m for m in resp.json() if is_temperpaw_owned_monitor(m, desired_names)
    ]
    existing_by_name = {m["name"]: m for m in existing_monitors}

    for monitor in monitors:
        name = monitor["name"]
        if name in existing_by_name:
            existing = existing_by_name[name]
            monitor_id = existing["id"]
            if existing.get("type") != monitor.get("type"):
                if args.dry_run:
                    print(
                        f"[dry-run] Would recreate: {name} "
                        f"(id={monitor_id}, {existing.get('type')} -> {monitor.get('type')})"
                    )
                    continue
                resp = requests.delete(
                    f"{base_url}/monitor/{monitor_id}",
                    headers=headers,
                )
                raise_for_status(resp, f"Delete monitor {name} ({monitor_id})")
                resp = requests.post(
                    f"{base_url}/monitor",
                    headers=headers,
                    json=monitor,
                )
                raise_for_status(resp, f"Create monitor {name}")
                monitor_id = resp.json().get("id", "unknown")
                print(
                    f"Recreated: {name} "
                    f"({existing.get('type')} -> {monitor.get('type')}, id={monitor_id})"
                )
                continue
            if args.dry_run:
                print(f"[dry-run] Would update: {name} (id={monitor_id})")
                continue
            resp = requests.put(
                f"{base_url}/monitor/{monitor_id}",
                headers=headers,
                json=monitor,
            )
            raise_for_status(resp, f"Update monitor {name} ({monitor_id})")
            print(f"Updated: {name} (id={monitor_id})")
        else:
            if args.dry_run:
                print(f"[dry-run] Would create: {name}")
                continue
            resp = requests.post(
                f"{base_url}/monitor",
                headers=headers,
                json=monitor,
            )
            raise_for_status(resp, f"Create monitor {name}")
            monitor_id = resp.json().get("id", "unknown")
            print(f"Created: {name} (id={monitor_id})")

    if args.reconcile:
        orphans = [
            (m["name"], m["id"])
            for m in existing_monitors
            if m.get("name") not in desired_names
        ]
        if not orphans:
            print("No orphan monitors to reconcile.")
            return
        print(f"\nReconcile: {len(orphans)} orphan monitor(s) to delete:")
        for name, monitor_id in orphans:
            if args.dry_run:
                print(f"  [dry-run] Would delete: {name} (id={monitor_id})")
                continue
            resp = requests.delete(
                f"{base_url}/monitor/{monitor_id}",
                headers=headers,
            )
            raise_for_status(resp, f"Delete orphan monitor {name} ({monitor_id})")
            print(f"  Deleted: {name} (id={monitor_id})")


if __name__ == "__main__":
    main()
