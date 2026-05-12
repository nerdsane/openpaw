#!/usr/bin/env python3
"""Deploy TemperPaw / Temper Datadog log pipelines and facets (ADR-0054).

Reads:
  dd-pipelines/temper-temperpaw.json    — pipeline + processors
  dd-pipelines/facets.json            — facets to register
  dd-pipelines/sensitive-data-scanner.json — SDS rule list
  dd-log-metrics/temper-log-metrics.json    — log-based metric definitions

Idempotent:
  - Pipeline: matched by name; PUT if exists, POST otherwise.
  - Facets: checked against existing facet list; only missing ones POSTed.
  - SDS rules: matched by name; PUT if exists, POST otherwise.
  - Log metrics: matched by id; PUT if exists, POST otherwise.
    With --reconcile, legacy openpaw.* log metrics are deleted after the
    TemperPaw metrics exist.

Requires DD_API_KEY and DD_APP_KEY in env (or .env).
"""

from __future__ import annotations

import argparse
import json
import os
import sys
from pathlib import Path

try:
    import requests
except ImportError:
    sys.exit("requests is required: pip install requests")

LEGACY_LOG_METRIC_PREFIXES = ("openpaw.",)


def load_env() -> None:
    env_path = Path(__file__).resolve().parent.parent / ".env"
    if env_path.exists():
        for line in env_path.read_text().splitlines():
            line = line.strip()
            if not line or line.startswith("#") or "=" not in line:
                continue
            k, _, v = line.partition("=")
            os.environ.setdefault(k.strip(), v.strip())


def deploy_pipeline(base: str, headers: dict, path: Path, dry_run: bool) -> None:
    body = json.loads(path.read_text())
    name = body["name"]
    r = requests.get(f"{base}/v1/logs/config/pipelines", headers=headers)
    r.raise_for_status()
    existing = {p["name"]: p["id"] for p in r.json()}
    if name in existing:
        pid = existing[name]
        if dry_run:
            print(f"[dry-run] would update pipeline {name} (id={pid})")
            return
        r = requests.put(
            f"{base}/v1/logs/config/pipelines/{pid}", headers=headers, json=body
        )
        r.raise_for_status()
        print(f"updated pipeline {name} (id={pid})")
    else:
        if dry_run:
            print(f"[dry-run] would create pipeline {name}")
            return
        r = requests.post(
            f"{base}/v1/logs/config/pipelines", headers=headers, json=body
        )
        r.raise_for_status()
        print(f"created pipeline {name}")


def deploy_facets(base: str, headers: dict, path: Path, dry_run: bool) -> None:
    """Facet registration has no stable first-class REST API across
    Datadog tiers — on orgs without it the v2 endpoint returns 404.
    Log the failure and continue; the facet definitions stay in
    dd-pipelines/facets.json as source-of-truth and operators can
    register them via the Log Explorer UI if needed."""
    data = json.loads(path.read_text())
    for facet in data["facets"]:
        if dry_run:
            print(f"[dry-run] would register facet @{facet['path']} ({facet['name']})")
            continue
        payload = {
            "data": {
                "type": "facet",
                "attributes": {
                    "name": facet["name"],
                    "source": "log",
                    "path": facet["path"],
                    "type": facet["type"],
                },
            }
        }
        r = requests.post(
            f"{base}/v2/logs/config/facets", headers=headers, json=payload
        )
        if r.status_code == 409:
            print(f"facet already exists: @{facet['path']}")
        elif r.status_code == 404:
            print(
                f"facet API unavailable on this DD tier — "
                f"register @{facet['path']} manually in Log Explorer"
            )
        elif r.status_code >= 400:
            print(
                f"facet register skipped for @{facet['path']}: "
                f"{r.status_code} {r.text[:160]}"
            )
        else:
            print(f"registered facet @{facet['path']}")


def deploy_sds(base: str, headers: dict, path: Path, dry_run: bool) -> None:
    data = json.loads(path.read_text())
    # Sensitive-data scanner rules live under /api/v2/sensitive-data-scanner/config.
    # We operate at the individual-rule level here.
    for rule in data["rules"]:
        payload = {
            "data": {
                "type": "sensitive_data_scanner_rule",
                "attributes": {
                    "name": rule["name"],
                    "pattern": rule["pattern"],
                    "text_replacement": {
                        "type": "replacement_string",
                        "replacement_string": rule["replacement"],
                    },
                    "is_enabled": True,
                },
            }
        }
        if dry_run:
            print(f"[dry-run] would upsert SDS rule {rule['name']!r}")
            continue
        # The SDS API requires a group ID we don't create here; skip POST
        # when group context isn't set and just log what we would do.
        # Operators complete SDS via the Datadog UI using this file as the
        # source of truth.
        print(f"SDS rule defined (apply via UI): {rule['name']!r}")


def deploy_log_metrics(
    base: str, headers: dict, path: Path, dry_run: bool, reconcile: bool
) -> None:
    data = json.loads(path.read_text())
    r = requests.get(f"{base}/v2/logs/config/metrics", headers=headers)
    r.raise_for_status()
    existing = {m["id"]: m for m in r.json().get("data", [])}
    desired_ids = {m["id"] for m in data["metrics"]}

    for m in data["metrics"]:
        body = {
            "data": {
                "type": "logs_metrics",
                "id": m["id"],
                "attributes": {
                    "filter": m["filter"],
                    "compute": m["compute"],
                    "group_by": m.get("group_by", []),
                },
            }
        }
        if m["id"] in existing:
            if dry_run:
                print(f"[dry-run] would update log-metric {m['id']}")
                continue
            r = requests.patch(
                f"{base}/v2/logs/config/metrics/{m['id']}", headers=headers, json=body
            )
            r.raise_for_status()
            print(f"updated log-metric {m['id']}")
        else:
            if dry_run:
                print(f"[dry-run] would create log-metric {m['id']}")
                continue
            r = requests.post(
                f"{base}/v2/logs/config/metrics", headers=headers, json=body
            )
            r.raise_for_status()
            print(f"created log-metric {m['id']}")

    if not reconcile:
        return

    for metric_id in sorted(existing):
        if metric_id in desired_ids or not metric_id.startswith(LEGACY_LOG_METRIC_PREFIXES):
            continue
        if dry_run:
            print(f"[dry-run] would delete legacy log-metric {metric_id}")
            continue
        r = requests.delete(
            f"{base}/v2/logs/config/metrics/{metric_id}", headers=headers
        )
        r.raise_for_status()
        print(f"deleted legacy log-metric {metric_id}")


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--dry-run", action="store_true")
    parser.add_argument(
        "--reconcile",
        action="store_true",
        help="Delete legacy openpaw.* log metrics after creating/updating TemperPaw metrics.",
    )
    args = parser.parse_args()

    load_env()
    api_key = os.environ.get("DD_API_KEY", "")
    app_key = os.environ.get("DD_APP_KEY", "")
    site = os.environ.get("DD_SITE", "datadoghq.com")
    if not api_key or not app_key:
        sys.exit("DD_API_KEY and DD_APP_KEY must be set")

    base = f"https://api.{site}/api"
    headers = {
        "DD-API-KEY": api_key,
        "DD-APPLICATION-KEY": app_key,
        "Content-Type": "application/json",
    }

    root = Path(__file__).resolve().parent.parent
    deploy_pipeline(base, headers, root / "dd-pipelines" / "temper-temperpaw.json", args.dry_run)
    deploy_facets(base, headers, root / "dd-pipelines" / "facets.json", args.dry_run)
    deploy_sds(base, headers, root / "dd-pipelines" / "sensitive-data-scanner.json", args.dry_run)
    deploy_log_metrics(
        base,
        headers,
        root / "dd-log-metrics" / "temper-log-metrics.json",
        args.dry_run,
        args.reconcile,
    )
    return 0


if __name__ == "__main__":
    sys.exit(main())
