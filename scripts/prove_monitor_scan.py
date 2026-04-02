#!/usr/bin/env python3
"""Prove the MonitorScan entity lifecycle through governed actions."""

from __future__ import annotations

import argparse
import json
import os

from openpaw_proof_support import (
    DEFAULT_BASE_URL,
    DEFAULT_REPO_URL,
    DEFAULT_TENANT,
    ODataClient,
    entity_id,
    now_utc,
    require,
    suffix,
)


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--base-url", default=os.getenv("OPENPAW_BASE_URL", DEFAULT_BASE_URL))
    parser.add_argument("--tenant", default=os.getenv("PAW_TENANT", DEFAULT_TENANT))
    parser.add_argument("--repo-url", default=DEFAULT_REPO_URL)
    args = parser.parse_args()

    client = ODataClient(
        base_url=args.base_url,
        tenant=args.tenant,
        api_key=os.getenv("TEMPER_API_KEY") or None,
    )
    run_suffix = suffix()

    harness = client.create("Harnesses", {"Id": f"monitor-scan-harness-{run_suffix}"})
    harness_id = entity_id(harness)
    require(harness_id, "failed to create Harness")
    client.action(
        "Harnesses",
        harness_id,
        "OpenPaw.Harness.Configure",
        {
            "repo_url": args.repo_url,
            "tech_stack": "Next.js frontend, Python backend",
            "conventions": "MonitorScan proof harness.",
        },
    )
    client.action(
        "Harnesses",
        harness_id,
        "OpenPaw.Harness.Activate",
        {"last_activated_at": now_utc()},
    )

    scan = client.create("MonitorScans", {"Id": f"monitor-scan-{run_suffix}"})
    scan_id = entity_id(scan)
    require(scan_id, "failed to create MonitorScan")
    client.action(
        "MonitorScans",
        scan_id,
        "OpenPaw.Heal.Configure",
        {
            "project_harness_id": harness_id,
            "scan_type": "bootstrap",
            "commit_sha": f"proof-{run_suffix}",
        },
    )
    client.action("MonitorScans", scan_id, "OpenPaw.Heal.StartScan", {})
    client.action(
        "MonitorScans",
        scan_id,
        "OpenPaw.Heal.ScanComplete",
        {
            "monitors_created": "3",
            "monitors_updated": "1",
        },
    )

    scan_entity = client.get("MonitorScans", scan_id)
    require(
        str(scan_entity.get("Status") or scan_entity.get("status") or "") == "Complete",
        f"MonitorScan did not reach Complete: {scan_entity}",
    )

    summary = {
        "project_harness_id": harness_id,
        "monitor_scan_id": scan_id,
        "monitor_scan": scan_entity,
    }
    print(json.dumps(summary, indent=2))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
