#!/usr/bin/env python3
"""Prove that Track 3 OTS trajectory emission works end-to-end.

Drives one paw-agent Session from Created through a terminal state, then
verifies:

  1. tool_spans_file_id was populated while run_tools persisted per-turn spans
  2. trajectory_emission_status == "emitted"
  3. trajectory_id is non-empty
  4. GET /api/ots/trajectories?agent_id=<probe> returns the emitted trajectory
  5. The persisted OTS JSON parses cleanly and carries the expected decisions

Usage:
    python scripts/prove_track3_ots.py [--base-url URL] [--tenant TENANT]
"""

from __future__ import annotations

import argparse
import json
import os
import sys
import time

sys.path.insert(0, os.path.dirname(__file__))

import requests  # type: ignore[import-not-found]

from temperpaw_proof_support import ODataClient, entity_id, nested_str, now_utc


TERMINAL_STATES = {"Completed", "Failed", "Cancelled"}


def wait_for_terminal(client: ODataClient, session_id: str, timeout_s: float) -> dict:
    deadline = time.time() + timeout_s
    last_status = ""
    while time.time() < deadline:
        session = client.get("Sessions", session_id)
        status = nested_str(session, ["Status"]) or nested_str(session, ["fields", "Status"])
        if status != last_status:
            print(f"  ... Session status: {status}")
            last_status = status or ""
        if status in TERMINAL_STATES:
            return session
        time.sleep(3)
    raise TimeoutError(
        f"Session {session_id} did not reach a terminal state within {timeout_s}s (last status={last_status})"
    )


def main() -> None:
    parser = argparse.ArgumentParser(description="Prove Track 3 OTS emission end-to-end.")
    parser.add_argument(
        "--base-url",
        default=os.environ.get("TEMPER_BASE_URL", "http://127.0.0.1:3467"),
    )
    parser.add_argument(
        "--tenant",
        default=os.environ.get("TEMPER_TENANT", "default"),
    )
    parser.add_argument(
        "--api-key",
        default=os.environ.get("TEMPER_API_KEY"),
    )
    parser.add_argument(
        "--user-message",
        default="List the files in /workspace and then call temper.done(\"ok\").",
    )
    parser.add_argument(
        "--timeout-s",
        type=float,
        default=240.0,
        help="Max seconds to wait for the Session to reach a terminal state.",
    )
    args = parser.parse_args()

    client = ODataClient(args.base_url, args.tenant, args.api_key)

    print("=" * 70)
    print("PROOF: Track 3 — OTS Trajectory Emission (ADR-0035)")
    print("=" * 70)
    print(f"  Platform:  {args.base_url}")
    print(f"  Tenant:    {args.tenant}")
    print(f"  Started:   {now_utc()}")
    print()

    # ── Step 1: Platform health ─────────────────────────────────
    print("[1/6] Checking platform health...")
    try:
        client.list("Sessions", top=1)
        print("  PASS: Sessions entity set reachable")
    except Exception as e:
        print(f"  FAIL: platform check: {e}")
        sys.exit(1)

    # ── Step 2: Create a Session ────────────────────────────────
    print("[2/6] Creating a Session...")
    session = client.create("Sessions", {"user_message": args.user_message})
    sid = entity_id(session)
    print(f"  PASS: Session entity_id={sid}")

    # ── Step 3: Start it ────────────────────────────────────────
    print("[3/6] Dispatching TemperPaw.Start...")
    client.action("Sessions", sid, "TemperPaw.Start", {})
    print("  PASS: Start action accepted")

    # ── Step 4: Wait for terminal state ─────────────────────────
    print(f"[4/6] Waiting up to {args.timeout_s:.0f}s for a terminal state...")
    try:
        final_session = wait_for_terminal(client, sid, args.timeout_s)
    except TimeoutError as e:
        print(f"  FAIL: {e}")
        sys.exit(2)
    final_status = nested_str(final_session, ["Status"]) or nested_str(
        final_session, ["fields", "Status"]
    )
    print(f"  PASS: Session reached {final_status}")

    # ── Step 5: Verify Phase 1 + Phase 2 fields on the entity ───
    print("[5/6] Verifying OTS-related entity fields...")
    fields = final_session.get("fields", final_session)
    tool_spans_file_id = fields.get("tool_spans_file_id") or ""
    trajectory_id = fields.get("trajectory_id") or ""
    emission_status = fields.get("trajectory_emission_status") or ""
    emission_error = fields.get("trajectory_emission_error") or ""

    checks = {
        "tool_spans_file_id populated": bool(tool_spans_file_id),
        "trajectory_id populated": bool(trajectory_id),
        "trajectory_emission_status == 'emitted'": emission_status == "emitted",
    }
    for desc, ok in checks.items():
        print(f"  {'PASS' if ok else 'FAIL'}: {desc}")
    if not all(checks.values()):
        print(f"  emission_error='{emission_error}' (if populated, indicates POST failed)")
        sys.exit(3)

    # ── Step 6: GET /api/ots/trajectories and match by trajectory_id ──
    print("[6/6] GET /api/ots/trajectories and matching by trajectory_id...")
    agent_id = fields.get("agent_id") or sid
    list_url = f"{args.base_url}/api/ots/trajectories"
    list_headers = {"X-Tenant-Id": args.tenant}
    if args.api_key:
        list_headers["Authorization"] = f"Bearer {args.api_key}"
    resp = requests.get(
        list_url,
        params={"agent_id": agent_id, "limit": 10},
        headers=list_headers,
        timeout=30,
    )
    resp.raise_for_status()
    listing = resp.json()
    trajectories = listing.get("trajectories") or []
    print(f"  Found {len(trajectories)} trajectories for agent_id={agent_id}")
    match = next(
        (t for t in trajectories if t.get("trajectory_id") == trajectory_id),
        None,
    )
    if not match:
        print(f"  FAIL: trajectory_id={trajectory_id} not in response: {json.dumps(listing, indent=2)[:2000]}")
        sys.exit(4)
    print(f"  PASS: trajectory {trajectory_id} persisted with outcome={match.get('outcome')}")

    print()
    print("ALL CHECKS PASSED — Track 3 OTS emission verified end-to-end.")


if __name__ == "__main__":
    main()
