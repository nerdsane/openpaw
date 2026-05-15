#!/usr/bin/env python3
"""Deploy OpenPaw self-monitoring Datadog monitors.

Reads dd-monitors/temperpaw-monitors.json and creates or updates each monitor
via the Datadog REST API. Idempotent: finds existing monitors by name.

With --reconcile, also deletes monitors tagged team:openpaw that are NOT in
the JSON file. This is the source-of-truth guarantee declared in ADR-0052:
file-first, Datadog state reconciles to match.

Requires DD_API_KEY and DD_APP_KEY in env (or .env file).
"""

import argparse
import json
import os
import sys
import time
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


def datadog_request(method, url, headers, **kwargs):
    """Call Datadog with simple 429 retry/backoff."""
    resp = None
    for attempt in range(6):
        resp = requests.request(method, url, headers=headers, timeout=30, **kwargs)
        if resp.status_code != 429:
            break
        retry_after = resp.headers.get("Retry-After")
        if retry_after and retry_after.isdigit():
            sleep_seconds = int(retry_after)
        else:
            sleep_seconds = min(2**attempt, 30)
        time.sleep(sleep_seconds)
    assert resp is not None
    return resp


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--reconcile",
        action="store_true",
        help="Delete monitors tagged team:openpaw that are not in the JSON file.",
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

    validation_errors = []
    for monitor in monitors:
        resp = datadog_request(
            "POST",
            f"{base_url}/monitor/validate",
            headers=headers,
            json=monitor,
        )
        if resp.status_code >= 400:
            validation_errors.append(
                f"{monitor.get('name', '<unnamed>')}: {resp.status_code} {resp.text}"
            )

    if validation_errors:
        joined = "\n\n".join(validation_errors)
        sys.exit(f"Datadog monitor validation failed:\n\n{joined}")

    resp = datadog_request(
        "GET",
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
            if args.dry_run:
                print(f"[dry-run] Would update: {name} (id={monitor_id})")
                continue
            resp = datadog_request(
                "PUT",
                f"{base_url}/monitor/{monitor_id}",
                headers=headers,
                json=monitor,
            )
            resp.raise_for_status()
            print(f"Updated: {name} (id={monitor_id})")
        else:
            if args.dry_run:
                print(f"[dry-run] Would create: {name}")
                continue
            resp = datadog_request(
                "POST",
                f"{base_url}/monitor",
                headers=headers,
                json=monitor,
            )
            resp.raise_for_status()
            monitor_id = resp.json().get("id", "unknown")
            print(f"Created: {name} (id={monitor_id})")

    if args.reconcile:
        orphans = [
            (name, monitor_id)
            for name, monitor_id in existing_by_name.items()
            if name not in desired_names
        ]
        if not orphans:
            print("No orphan monitors to reconcile.")
            return
        print(f"\nReconcile: {len(orphans)} orphan monitor(s) to delete:")
        for name, monitor_id in orphans:
            if args.dry_run:
                print(f"  [dry-run] Would delete: {name} (id={monitor_id})")
                continue
                resp = datadog_request(
                    "DELETE",
                    f"{base_url}/monitor/{monitor_id}",
                    headers=headers,
                )
            resp.raise_for_status()
            print(f"  Deleted: {name} (id={monitor_id})")


if __name__ == "__main__":
    main()
