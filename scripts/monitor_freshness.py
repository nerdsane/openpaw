#!/usr/bin/env python3
"""Open GitHub issues for [TemperPaw]/[Temper] Datadog monitors stuck in
`No Data` state for more than 7 consecutive days.

ADR-0052 sub-decision 3: a No-Data monitor surviving >7 days in production
is either an unwired emitter or a stale query, and either way the human
owner must act. This script is the enforcement mechanism.

Invoked:
    DD_API_KEY=... DD_APP_KEY=... GH_TOKEN=... \\
      python3 scripts/monitor_freshness.py [--dry-run]

Typically scheduled once per day via GitHub Actions.
"""

from __future__ import annotations

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


SEVEN_DAYS_MS = 7 * 24 * 60 * 60 * 1000


def load_env() -> None:
    env_path = Path(__file__).resolve().parent.parent / ".env"
    if env_path.exists():
        for line in env_path.read_text().splitlines():
            line = line.strip()
            if not line or line.startswith("#") or "=" not in line:
                continue
            k, _, v = line.partition("=")
            os.environ.setdefault(k.strip(), v.strip())


def datadog_monitors(api_key: str, app_key: str, site: str) -> list[dict]:
    url = f"https://api.{site}/api/v1/monitor"
    headers = {
        "DD-API-KEY": api_key,
        "DD-APPLICATION-KEY": app_key,
        "Content-Type": "application/json",
    }
    resp = requests.get(url, headers=headers, params={"monitor_tags": "team:temperpaw"})
    resp.raise_for_status()
    return resp.json()


def pick_stale(monitors: list[dict], now_ms: int) -> list[dict]:
    stale = []
    for m in monitors:
        state = m.get("overall_state")
        if state != "No Data":
            continue
        modified = m.get("modified") or m.get("created")
        if not modified:
            continue
        try:
            # Datadog serves ISO-8601.
            ts = time.strptime(modified[:19], "%Y-%m-%dT%H:%M:%S")
            modified_ms = int(time.mktime(ts)) * 1000
        except ValueError:
            continue
        if (now_ms - modified_ms) >= SEVEN_DAYS_MS:
            stale.append(m)
    return stale


def github_issue_exists(gh_token: str, repo: str, monitor_name: str) -> bool:
    url = f"https://api.github.com/repos/{repo}/issues"
    headers = {
        "Authorization": f"Bearer {gh_token}",
        "Accept": "application/vnd.github+json",
    }
    q = f'repo:{repo} is:issue label:monitor-stale "{monitor_name}" in:title'
    resp = requests.get(
        "https://api.github.com/search/issues",
        headers=headers,
        params={"q": q},
    )
    resp.raise_for_status()
    return resp.json().get("total_count", 0) > 0


def open_github_issue(gh_token: str, repo: str, monitor: dict) -> None:
    url = f"https://api.github.com/repos/{repo}/issues"
    headers = {
        "Authorization": f"Bearer {gh_token}",
        "Accept": "application/vnd.github+json",
    }
    title = f"[observability] Monitor stale (No Data >7d): {monitor['name']}"
    body = (
        "## Stale monitor\n\n"
        f"- **Name**: {monitor['name']}\n"
        f"- **ID**: {monitor['id']}\n"
        f"- **Last modified**: {monitor.get('modified')}\n"
        f"- **Query**: `{monitor.get('query', '')}`\n\n"
        "This monitor has been in `No Data` state for 7 consecutive days. "
        "Per ADR-0052 sub-decision 3, one of the following must happen:\n\n"
        "1. Wire the missing emitter.\n"
        "2. Rewrite the query against an equivalent live metric.\n"
        "3. Delete the monitor if the concept is no longer relevant.\n\n"
        "Close this issue after taking one of those actions."
    )
    resp = requests.post(
        url,
        headers=headers,
        json={
            "title": title,
            "body": body,
            "labels": ["observability", "monitor-stale"],
        },
    )
    resp.raise_for_status()
    print(f"opened: {monitor['name']} → {resp.json().get('html_url')}")


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--dry-run",
        action="store_true",
        help="Print what would be opened without creating GitHub issues.",
    )
    parser.add_argument(
        "--repo",
        default=os.environ.get("GH_REPO", "nerdsane/temperpaw"),
        help="GitHub repo (default: nerdsane/temperpaw).",
    )
    args = parser.parse_args()

    load_env()
    api_key = os.environ.get("DD_API_KEY", "")
    app_key = os.environ.get("DD_APP_KEY", "")
    gh_token = os.environ.get("GH_TOKEN", "") or os.environ.get("GITHUB_TOKEN", "")
    site = os.environ.get("DD_SITE", "datadoghq.com")

    if not api_key or not app_key:
        sys.exit("DD_API_KEY and DD_APP_KEY must be set")
    if not gh_token and not args.dry_run:
        sys.exit("GH_TOKEN or GITHUB_TOKEN must be set (for issue creation)")

    monitors = datadog_monitors(api_key, app_key, site)
    now_ms = int(time.time() * 1000)
    stale = pick_stale(monitors, now_ms)

    if not stale:
        print(f"monitor_freshness: no stale monitors; checked {len(monitors)}.")
        return 0

    print(f"monitor_freshness: {len(stale)} stale monitor(s) identified.")
    opened = 0
    for m in stale:
        if args.dry_run:
            print(f"[dry-run] {m['name']} (last modified {m.get('modified')})")
            continue
        if github_issue_exists(gh_token, args.repo, m["name"]):
            print(f"skip (issue already open): {m['name']}")
            continue
        open_github_issue(gh_token, args.repo, m)
        opened += 1

    print(f"monitor_freshness: opened {opened} new issue(s).")
    return 0


if __name__ == "__main__":
    sys.exit(main())
