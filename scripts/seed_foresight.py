#!/usr/bin/env python3
"""Seed Foresight for the Deep Sci-Fi project.

Creates a ProductModel, seeds it with signals, creates a Projection with
a configurable number of Probe sessions, and starts the projection cycle.

Idempotent — checks for existing entities before creating new ones.

Usage:
    python scripts/seed_foresight.py [--base-url URL] [--tenant TENANT]
"""

from __future__ import annotations

import argparse
import json
import os
import sys
import time

sys.path.insert(0, os.path.dirname(__file__))

from openpaw_proof_support import ODataClient, entity_id, nested_str, now_utc

REPO_URL = "https://github.com/arni-labs/deep-sci-fi.git"

SIGNAL_SOURCE_CONFIG = json.dumps({
    "dd_service_tag": "deep-sci-fi",
    "github": True,
    "datadog": True,
})

DEFAULT_PROBE_COUNT = 3

# Adaptive step schedule: 3 daily, 3 weekly, 3 monthly, 3 quarterly
STEP_SCHEDULE = json.dumps([1, 1, 1, 7, 7, 7, 30, 30, 30, 90, 90, 90])


def find_product_model(client: ODataClient) -> dict | None:
    results = client.list(
        "ProductModels",
        filter_expr=f"repo_url eq '{REPO_URL}'",
        top=1,
    )
    return results[0] if results else None


def find_harness(client: ODataClient) -> str | None:
    results = client.list(
        "Harnesses",
        filter_expr=f"repo_url eq '{REPO_URL}'",
        top=1,
    )
    if results:
        return entity_id(results[0])
    return None


def wait_for_state(
    client: ODataClient,
    entity_set: str,
    eid: str,
    target_states: list[str],
    timeout: int = 120,
    poll_interval: int = 3,
) -> dict:
    """Poll until entity reaches one of target_states."""
    deadline = time.time() + timeout
    while time.time() < deadline:
        entity = client.get(entity_set, eid)
        status = nested_str(entity, ["Status", "status"])
        if status in target_states:
            return entity
        print(f"    ... state={status}, waiting")
        time.sleep(poll_interval)
    raise TimeoutError(
        f"{entity_set}('{eid}') did not reach {target_states} within {timeout}s"
    )


def seed_product_model(client: ODataClient) -> str:
    """Create or find the ProductModel for Deep Sci-Fi. Returns entity ID."""
    existing = find_product_model(client)
    if existing:
        mid = entity_id(existing)
        status = nested_str(existing, ["Status", "status"])
        print(f"  [exists] ProductModel ({mid}) -- {status}")
        return mid

    harness_id = find_harness(client) or ""

    model = client.create("ProductModels")
    mid = entity_id(model)

    # Set fields via self-loop or direct field setting
    client.action(
        "ProductModels",
        mid,
        "OpenPaw.Foresight.Seed",
        {
            "project_harness_id": harness_id,
            "repo_url": REPO_URL,
            "signal_source_config": SIGNAL_SOURCE_CONFIG,
        },
    )
    print(f"  [created] ProductModel ({mid}) -- seeding...")
    return mid


def build_probe_config(probe_count: int) -> str:
    probes: list[dict[str, str]] = []
    if probe_count <= 0:
        raise ValueError("probe_count must be > 0")

    probes.append({"name": "Probe-Deep", "model": "claude-opus-4-6"})
    for idx in range(1, probe_count):
        probes.append(
            {
                "name": f"Probe-Broad-{idx}",
                "model": "claude-sonnet-4-6",
            }
        )
    return json.dumps(probes)


def seed_projection(
    client: ODataClient,
    product_model_id: str,
    probe_config: str,
    *,
    reuse_existing: bool = True,
) -> str:
    """Create a Projection for the ProductModel. Returns entity ID."""
    if reuse_existing:
        results = client.list(
            "Projections",
            filter_expr=f"product_model_id eq '{product_model_id}'",
            top=1,
        )
        if results:
            pid = entity_id(results[0])
            status = nested_str(results[0], ["Status", "status"])
            print(f"  [exists] Projection ({pid}) -- {status}")
            return pid

    proj = client.create("Projections")
    pid = entity_id(proj)

    client.action(
        "Projections",
        pid,
        "OpenPaw.Foresight.Configure",
        {
            "product_model_id": product_model_id,
            "horizon": "3m",
            "max_steps": "12",
            "step_schedule": STEP_SCHEDULE,
            "probe_config": probe_config,
        },
    )
    print(f"  [created] Projection ({pid}) -- configured")
    return pid


def start_projection(client: ODataClient, projection_id: str) -> None:
    """Start the Projection (triggers spawn_probes → advance_step chain)."""
    proj = client.get("Projections", projection_id)
    status = nested_str(proj, ["Status", "status"])
    if status != "Created":
        print(f"  [skip] Projection already in state {status}")
        return

    client.action(
        "Projections",
        projection_id,
        "OpenPaw.Foresight.Start",
        {},
    )
    print(f"  [started] Projection ({projection_id})")


def main() -> None:
    parser = argparse.ArgumentParser(
        description="Seed Foresight for the Deep Sci-Fi project."
    )
    parser.add_argument(
        "--base-url",
        default=os.environ.get("TEMPER_BASE_URL", "http://127.0.0.1:3467"),
    )
    parser.add_argument(
        "--tenant", default=os.environ.get("TEMPER_TENANT", "default")
    )
    parser.add_argument(
        "--api-key", default=os.environ.get("TEMPER_API_KEY")
    )
    parser.add_argument(
        "--start", action="store_true",
        help="Start the Projection after seeding (triggers Probe spawning)",
    )
    parser.add_argument(
        "--probe-count",
        type=int,
        default=int(os.environ.get("OPENPAW_PROBE_COUNT", DEFAULT_PROBE_COUNT)),
        help="Number of Probe sessions to configure for the Projection.",
    )
    parser.add_argument(
        "--new-projection",
        action="store_true",
        help="Always create a fresh Projection instead of reusing an existing one for the ProductModel.",
    )
    args = parser.parse_args()

    if args.probe_count <= 0:
        raise SystemExit("--probe-count must be greater than 0")

    client = ODataClient(args.base_url, args.tenant, args.api_key)
    probe_config = build_probe_config(args.probe_count)

    print("=== Seeding Foresight for Deep Sci-Fi ===\n")

    print("1. ProductModel")
    model_id = seed_product_model(client)

    print("\n2. Wait for ProductModel to seed...")
    try:
        wait_for_state(client, "ProductModels", model_id, ["Active"], timeout=120)
        print("  ProductModel is Active")
    except TimeoutError:
        print("  [warn] ProductModel did not reach Active — may still be seeding")

    print("\n3. Projection")
    proj_id = seed_projection(
        client,
        model_id,
        probe_config,
        reuse_existing=not args.new_projection,
    )

    if args.start:
        print("\n4. Starting Projection...")
        start_projection(client, proj_id)
        print("  Probes will be spawned and the first step will advance.")

    print(f"\nDone. ProductModel: {model_id}, Projection: {proj_id}")


if __name__ == "__main__":
    main()
