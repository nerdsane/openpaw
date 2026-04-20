#!/usr/bin/env python3
"""Proof harness for the Foresight engine.

Verifies end-to-end: entity specs deployed, ProductModel seeds, Projection
advances, Probes spawn, Observations created, gates enforced.

Usage:
    python scripts/prove_foresight.py [--base-url URL] [--tenant TENANT]
"""

from __future__ import annotations

import argparse
import json
import os
import sys
import time

sys.path.insert(0, os.path.dirname(__file__))

from temperpaw_proof_support import ODataClient, entity_id, nested_str, now_utc

REPO_URL = "https://github.com/arni-labs/deep-sci-fi.git"

assertions_passed = 0
assertions_failed = 0
report_lines: list[str] = []


def assert_true(label: str, condition: bool, detail: str = "") -> None:
    global assertions_passed, assertions_failed
    if condition:
        assertions_passed += 1
        status = "PASS"
    else:
        assertions_failed += 1
        status = "FAIL"
    msg = f"  [{status}] {label}"
    if detail:
        msg += f" — {detail}"
    print(msg)
    report_lines.append(msg)


def wait_for_state(
    client: ODataClient,
    entity_set: str,
    eid: str,
    target_states: list[str],
    timeout: int = 120,
    poll_interval: int = 5,
) -> dict | None:
    deadline = time.time() + timeout
    while time.time() < deadline:
        entity = client.get(entity_set, eid)
        status = nested_str(entity, ["Status", "status"])
        if status in target_states:
            return entity
        time.sleep(poll_interval)
    return None


def main() -> None:
    parser = argparse.ArgumentParser(description="Foresight engine proof harness.")
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
    args = parser.parse_args()

    client = ODataClient(args.base_url, args.tenant, args.api_key)

    print("=== Foresight Engine Proof ===\n")

    # --- Step 1: Verify entity specs deployed ---
    print("Step 1: Entity specs")
    for es in ["ProductModels", "Projections", "Observations", "Directions", "DirectionFeedbacks"]:
        try:
            client.list(es, top=0)
            assert_true(f"{es} queryable", True)
        except Exception as e:
            assert_true(f"{es} queryable", False, str(e))

    # --- Step 2: ProductModel seeds and reaches Active ---
    print("\nStep 2: ProductModel")
    models = client.list("ProductModels", filter_expr=f"repo_url eq '{REPO_URL}'", top=1)
    if models:
        model_id = entity_id(models[0])
        print(f"  Found existing ProductModel: {model_id}")
    else:
        model = client.create("ProductModels")
        model_id = entity_id(model)
        client.action("ProductModels", model_id, "TemperPaw.Foresight.Seed", {
            "repo_url": REPO_URL,
            "signal_source_config": json.dumps({"github": True, "datadog": True}),
        })
        print(f"  Created ProductModel: {model_id}")

    result = wait_for_state(client, "ProductModels", model_id, ["Active"], timeout=120)
    assert_true("ProductModel reaches Active", result is not None)
    if result:
        snapshot_id = nested_str(result, ["model_snapshot_file_id"])
        assert_true("model_snapshot_file_id set", bool(snapshot_id), snapshot_id or "empty")

    # --- Step 3: Projection starts ---
    print("\nStep 3: Projection")
    projections = client.list("Projections", filter_expr=f"product_model_id eq '{model_id}'", top=1)
    if projections:
        proj_id = entity_id(projections[0])
        print(f"  Found existing Projection: {proj_id}")
    else:
        proj = client.create("Projections")
        proj_id = entity_id(proj)
        client.action("Projections", proj_id, "TemperPaw.Foresight.Configure", {
            "product_model_id": model_id,
            "horizon": "3m",
            "max_steps": "3",
            "step_schedule": json.dumps([1, 7, 30]),
            "probe_config": json.dumps([
                {"name": "Proof-Probe-1", "model": "claude-sonnet-4-6", "provider": "anthropic"},
                {"name": "Proof-Probe-2", "model": "claude-sonnet-4-6", "provider": "anthropic"},
            ]),
        })
        client.action("Projections", proj_id, "TemperPaw.Foresight.Start", {})
        print(f"  Created and started Projection: {proj_id}")

    # Wait for at least one step
    print("  Waiting for step advancement...")
    result = wait_for_state(client, "Projections", proj_id, ["Running", "Complete"], timeout=180)
    assert_true("Projection reaches Running or Complete", result is not None)

    # --- Step 4: Probes spawned ---
    print("\nStep 4: Probe agents")
    probes = client.list("Agents", filter_expr="role eq 'probe'", top=10)
    assert_true("Probe agents exist", len(probes) >= 1, f"found {len(probes)}")

    # --- Step 5: Observations created ---
    print("\nStep 5: Observations")
    observations = client.list("Observations", filter_expr=f"projection_id eq '{proj_id}'", top=10)
    assert_true("At least 1 Observation exists", len(observations) >= 1, f"found {len(observations)}")

    if observations:
        obs = observations[0]
        obs_id = entity_id(obs)
        content = nested_str(obs, ["content"])
        assert_true("Observation has content", bool(content), content[:80] if content else "empty")

    # --- Step 6: Gate testing ---
    print("\nStep 6: Gate testing")

    # Create a test observation for gate testing
    test_obs = client.create("Observations")
    test_obs_id = entity_id(test_obs)
    client.action("Observations", test_obs_id, "TemperPaw.Foresight.Record", {
        "probe_agent_id": "test-probe-a",
        "projection_id": proj_id,
        "step_at": "0",
        "content": "Test observation for gate proof",
        "importance": "high",
        "signal_refs": json.dumps(["trends.error_rate"]),
    })

    # Gate 2: Confirm with different probe (should succeed)
    try:
        client.action("Observations", test_obs_id, "TemperPaw.Foresight.Confirm", {
            "confirmer_agent_id": "test-probe-b",
            "confirmation_note": "I independently see the same signal",
        })
        confirmed = client.get("Observations", test_obs_id)
        status = nested_str(confirmed, ["Status", "status"])
        assert_true("Confirm with different probe succeeds", status == "Confirmed", status)
    except Exception as e:
        assert_true("Confirm with different probe succeeds", False, str(e))

    # --- Step 7: Direction creation ---
    print("\nStep 7: Direction creation")
    try:
        direction = client.create("Directions")
        dir_id = entity_id(direction)
        client.action("Directions", dir_id, "TemperPaw.Foresight.Propose", {
            "title": "Test Direction: Auth Module Overhaul",
            "proposer_agent_id": "test-probe-a",
            "projection_id": proj_id,
            "reasoning": "Auth errors climbing, shares connection pool with billing, potential cascade under load.",
            "grounding": json.dumps(["signals.datadog.trends.error_rate", "signals.github.issues_open"]),
            "observation_ids": json.dumps([test_obs_id]),
        })
        assert_true("Direction created and proposed", True)
    except Exception as e:
        assert_true("Direction created and proposed", False, str(e))

    # --- Summary ---
    print(f"\n{'='*50}")
    print(f"PASSED: {assertions_passed}")
    print(f"FAILED: {assertions_failed}")
    print(f"TOTAL:  {assertions_passed + assertions_failed}")

    # --- Write proof report ---
    proof_dir = os.path.join(os.path.dirname(__file__), os.pardir, ".proofs")
    os.makedirs(proof_dir, exist_ok=True)

    # Find next proof number
    existing = [f for f in os.listdir(proof_dir) if f.endswith(".md") and f[0].isdigit()]
    nums = [int(f.split("-")[0]) for f in existing if f.split("-")[0].isdigit()]
    next_num = max(nums, default=0) + 1

    report_path = os.path.join(proof_dir, f"{next_num:03d}-foresight-engine.md")
    with open(report_path, "w") as f:
        f.write(f"# Proof {next_num:03d}: Foresight Engine\n\n")
        f.write(f"**Date:** {now_utc()}\n")
        f.write(f"**Passed:** {assertions_passed}\n")
        f.write(f"**Failed:** {assertions_failed}\n\n")
        f.write("## Assertions\n\n")
        for line in report_lines:
            f.write(f"{line}\n")
        f.write(f"\n## Entities\n\n")
        f.write(f"- ProductModel: {model_id}\n")
        f.write(f"- Projection: {proj_id}\n")
        if probes:
            for p in probes[:5]:
                f.write(f"- Probe Agent: {entity_id(p)}\n")
        f.write(f"\n## Notes\n\n")
        f.write("Foresight engine proof: substrate architecture, Probes ARE the simulation.\n")

    print(f"\nProof report written to: {report_path}")

    sys.exit(1 if assertions_failed > 0 else 0)


if __name__ == "__main__":
    main()
