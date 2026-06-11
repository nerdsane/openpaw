#!/usr/bin/env python3
"""Hindcast harness: seed the engine at a past vantage with a frozen corpus,
run the corridor, grade implied forecasts against recorded actuals.

The hindcast world runs with hindcast_mode=true: every spawned session has
web tools stripped at Configure time; evidence is the frozen corpus only.
Residual model-prior contamination (the model knows things dated after the
vantage) is real and recorded honestly in the coverage note — scores are
relative (engine A vs engine B), never absolute.

Usage:
  python3 scripts/prove_corridor_hindcast.py --base-url http://127.0.0.1:4500 \
      --api-key <key> --model <model> --provider anthropic \
      --vantage 2025-06-11 --corpus-file <path.md> --actuals-file <path.json> \
      [--timeout-min 45]

Actuals JSON: [{"question_contains": "...", "outcome": "yes"|"no"}, ...]
(or {"event_node_id": "...", "outcome": ...} entries).
"""

from __future__ import annotations

import argparse
import json
import sys
import urllib.request

from prove_corridor_e2e import Client, field, status_of, wait


def upload_file(c: Client, name: str, path: str) -> str:
    # File writes are permitted for system agents in the paw-fs bundle;
    # the api-key-holder/admin principal has no File-create permit.
    sys_headers = [
        ("Authorization", f"Bearer {c.key}"),
        ("X-Tenant-Id", c.tenant),
        ("X-Temper-Principal-Kind", "agent"),
        ("X-Temper-Principal-Id", "hindcast-harness"),
        ("X-Temper-Agent-Type", "system"),
        ("Content-Type", "application/json"),
    ]
    create_req = urllib.request.Request(f"{c.base}/tdata/Files", method="POST")
    for k, v in sys_headers:
        create_req.add_header(k, v)
    import json as _json
    with urllib.request.urlopen(
        create_req, data=_json.dumps({"name": name}).encode(), timeout=120
    ) as r:
        created = _json.loads(r.read().decode())
    fid = created["entity_id"]
    req = urllib.request.Request(
        f"{c.base}/tdata/Files('{fid}')/$value", method="PUT"
    )
    for k, v in sys_headers[:-1]:
        req.add_header(k, v)
    with open(path, "rb") as fh:
        data = fh.read()
    with urllib.request.urlopen(req, data=data, timeout=120) as r:
        if r.status >= 300:
            raise RuntimeError(f"file upload failed: HTTP {r.status}")
    return fid


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--base-url", required=True)
    ap.add_argument("--tenant", default="default")
    ap.add_argument("--api-key", required=True)
    ap.add_argument("--model", required=True)
    ap.add_argument("--provider", required=True)
    ap.add_argument("--vantage", required=True, help="the past date the engine treats as today")
    ap.add_argument("--horizon-months", default="6")
    ap.add_argument("--target-date", required=True, help="vantage + horizon (the graded window end)")
    ap.add_argument("--domain", default="the AI coding tools market (agents, IDEs, CLIs)")
    ap.add_argument("--corpus-file", required=True)
    ap.add_argument("--actuals-file", required=True)
    ap.add_argument("--budget", default="2")
    ap.add_argument("--timeout-min", type=int, default=45)
    args = ap.parse_args()

    c = Client(args.base_url, args.tenant, args.api_key)
    t = args.timeout_min * 60

    print("=== 0. Freeze corpus + actuals into paw-fs ===")
    corpus_id = upload_file(c, f"hindcast-corpus-{args.vantage}", args.corpus_file)
    actuals_id = upload_file(c, f"hindcast-actuals-{args.vantage}", args.actuals_file)
    print(f"corpus={corpus_id} actuals={actuals_id}")

    print("=== 1. Hindcast world (hindcast_mode=true) ===")
    world = c.create(
        "Worlds", {"name": f"Hindcast {args.vantage}", "domain": args.domain}
    )
    wid = world["entity_id"]
    c.action(
        "Worlds",
        wid,
        "Configure",
        {
            "name": f"Hindcast {args.vantage}",
            "domain": args.domain,
            "description": f"Hindcast: vantage {args.vantage}, frozen corpus only. "
            "Nothing dated after the vantage may be stated.",
            "horizon_months": args.horizon_months,
            "target_date": args.target_date,
            "frontier_date": args.target_date,
            "corpus_file_id": corpus_id,
            "endpoint_budget": args.budget,
            "agent_model": args.model,
            "agent_provider": args.provider,
            "hindcast_mode": "true",
        },
    )

    hc = c.create("Hindcasts", {
        "domain": args.domain,
        "vantage_date": args.vantage,
        "horizon_months": args.horizon_months,
        "frozen_corpus_file_id": corpus_id,
        "actuals_file_id": actuals_id,
        "engine_version": "0.2.0",
    })
    hid = hc["entity_id"]
    c.action("Hindcasts", hid, "Start", {"world_id": wid})
    print(f"hindcast={hid} world={wid}")

    print("=== 2. Corridor (seed -> sample -> repair -> score) ===")
    c.action("Worlds", wid, "Seed", {})

    def seeded():
        w = c.get("Worlds", wid)
        s = status_of(w)
        if s == "Active":
            return "Active"
        if s == "Failed":
            raise RuntimeError(field(w, "error_message", "ErrorMessage"))
        return None

    wait("hindcast seed", seeded, t)
    c.action("Worlds", wid, "SampleEndpoints", {})

    def settled():
        w = c.get("Worlds", wid)
        if status_of(w) == "Failed":
            raise RuntimeError(field(w, "error_message", "ErrorMessage"))
        return field(w, "canonical_path_id", "CanonicalPathId") or None

    wait("hindcast corridor settled", settled, t)

    forecasts = c.list("Forecasts", f"world_id eq '{wid}'")
    print(f"forecasts registered: {len(forecasts)}")

    print("=== 3. Grade against actuals ===")
    c.action("Hindcasts", hid, "Grade", {})

    def graded():
        h = c.get("Hindcasts", hid)
        s = status_of(h)
        if s == "Scored":
            return (
                f"mean_brier={field(h, 'mean_brier', 'MeanBrier')} "
                f"graded={field(h, 'graded_count', 'GradedCount')}"
            )
        if s == "Failed":
            raise RuntimeError(field(h, "error_message", "ErrorMessage"))
        return None

    wait("hindcast graded", graded, t)
    h = c.get("Hindcasts", hid)
    print("coverage note:", field(h, "coverage_note", "CoverageNote"))
    print(
        json.dumps(
            {
                "hindcast_id": hid,
                "world_id": wid,
                "mean_brier": field(h, "mean_brier", "MeanBrier"),
                "graded_count": field(h, "graded_count", "GradedCount"),
                "forecasts": len(forecasts),
            },
            indent=2,
        )
    )
    return 0


if __name__ == "__main__":
    sys.exit(main())
