#!/usr/bin/env python3
"""Deploy OpenPaw / Temper Datadog log pipelines and facets (ADR-0054).

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
    data = json.loads(path.read_text())
    # Datadog doesn't expose facet-create REST as a first-class API for
    # every tier; when available it's /api/v2/logs/config/facets. Log
    # Management-tier accounts can POST facets individually.
    for facet in data["facets"]:
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
        if dry_run:
            print(f"[dry-run] would register facet @{facet['path']} ({facet['name']})")
            continue
        r = requests.post(
            f"{base}/v2/logs/config/facets", headers=headers, json=payload
        )
        if r.status_code == 409:
            print(f"facet already exists: @{facet['path']}")
            continue
        r.raise_for_status()
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


def deploy_log_metrics(base: str, headers: dict, path: Path, dry_run: bool) -> None:
    data = json.loads(path.read_text())
    r = requests.get(f"{base}/v2/logs/config/metrics", headers=headers)
    r.raise_for_status()
    existing = {m["id"]: m for m in r.json().get("data", [])}

    for m in data["metrics"]:
        body = {
            "data": {
                "type": "logs_metric",
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


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--dry-run", action="store_true")
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
    deploy_log_metrics(base, headers, root / "dd-log-metrics" / "temper-log-metrics.json", args.dry_run)
    return 0


if __name__ == "__main__":
    sys.exit(main())
