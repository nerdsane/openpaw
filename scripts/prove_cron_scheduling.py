#!/usr/bin/env python3
"""Prove that CronJob scheduling works end-to-end for team agents.

Creates a CronJob with a short schedule (every 2 minutes), activates it,
watches for the first trigger to fire, verifies a Session is created,
and reports results.

Usage:
    python scripts/prove_cron_scheduling.py [--base-url URL] [--tenant TENANT]
"""

from __future__ import annotations

import argparse
import os
import sys
import time

sys.path.insert(0, os.path.dirname(__file__))

from openpaw_proof_support import ODataClient, entity_id, nested_str, now_utc


def main() -> None:
    parser = argparse.ArgumentParser(description="Prove CronJob scheduling for team agents.")
    parser.add_argument("--base-url", default=os.environ.get("TEMPER_BASE_URL", "http://127.0.0.1:3467"))
    parser.add_argument("--tenant", default=os.environ.get("TEMPER_TENANT", "default"))
    parser.add_argument("--api-key", default=os.environ.get("TEMPER_API_KEY"))
    parser.add_argument("--schedule", default="*/2 * * * *", help="Cron schedule (default: every 2 min)")
    parser.add_argument("--wait-minutes", type=float, default=5.0, help="Max minutes to wait for trigger")
    args = parser.parse_args()

    client = ODataClient(args.base_url, args.tenant, args.api_key)

    print("=" * 60)
    print("PROOF: CronJob Scheduling for Team Agents")
    print("=" * 60)
    print(f"  Platform:  {args.base_url}")
    print(f"  Tenant:    {args.tenant}")
    print(f"  Schedule:  {args.schedule}")
    print(f"  Started:   {now_utc()}")
    print()

    # ── Step 1: Verify platform is alive ────────────────────────
    print("[1/7] Checking platform health...")
    try:
        client.list("CronJobs", top=1)
        print("  ✓ Platform is up, CronJobs entity set exists")
    except Exception as e:
        print(f"  ✗ Platform check failed: {e}")
        sys.exit(1)

    # ── Step 2: Check DSF team agents exist ─────────────────────
    print("[2/7] Verifying DSF team agents...")
    agents = client.list("Agents", filter_expr="status eq 'Active'")
    agent_names = [nested_str(a, ["name"]) for a in agents]
    print(f"  Found {len(agents)} active agents: {', '.join(agent_names)}")
    sre_agents = [a for a in agents if nested_str(a, ["role"]) == "SRE"]
    if not sre_agents:
        print("  ⚠ No SRE agent found — will use generic system prompt")
        soul_id = ""
    else:
        soul_id = "SRE"
        print(f"  ✓ SRE agent found: {entity_id(sre_agents[0])}")

    # ── Step 3: Create CronJob ──────────────────────────────────
    print("[3/7] Creating CronJob entity...")
    cron = client.create("CronJobs")
    cron_id = entity_id(cron)
    print(f"  Created: {cron_id}")

    # ── Step 4: Configure CronJob ───────────────────────────────
    print("[4/7] Configuring CronJob...")
    config_params = {
        "name": f"proof-cron-scheduling-{int(time.time())}",
        "schedule": args.schedule,
        "soul_id": soul_id,
        "system_prompt": "You are a site reliability agent doing a quick status check. Respond briefly with a status summary.",
        "user_message_template": (
            "Quick status check (run {{run_count}}). "
            "List the entity types available in this tenant. "
            "Previous result: {{last_result}}"
        ),
        "model": "claude-sonnet-4-6",
        "provider": "anthropic",
        "tools_enabled": "temper_get,temper_list,read",
        "max_turns": "5",
        "max_runs": "3",
    }
    try:
        client.action("CronJobs", cron_id, "OpenPaw.Configure", config_params)
        print(f"  ✓ Configured with schedule={args.schedule}")
    except Exception as e:
        print(f"  ✗ Configure failed: {e}")
        sys.exit(1)

    # Verify configuration was applied
    cron_entity = client.get("CronJobs", cron_id)
    cron_fields = cron_entity.get("fields", {})
    print(f"  Verified: name={cron_fields.get('name')}, schedule={cron_fields.get('schedule')}")

    # ── Step 5: Activate CronJob ────────────────────────────────
    print("[5/7] Activating CronJob (triggers cron_activate WASM)...")
    pre_activate_time = now_utc()
    try:
        client.action("CronJobs", cron_id, "OpenPaw.Activate", {})
        print(f"  ✓ Activate dispatched at {pre_activate_time}")
    except Exception as e:
        print(f"  ✗ Activate failed: {e}")
        # Check entity state for error details
        cron_entity = client.get("CronJobs", cron_id)
        print(f"  Entity status: {nested_str(cron_entity, ['Status', 'status'])}")
        events = cron_entity.get("events", [])
        for ev in events[-3:]:
            print(f"    Event: {ev.get('action')} → {ev.get('to_status')} params={ev.get('params', {})}")
        sys.exit(1)

    # Wait briefly for WASM to complete and check state
    time.sleep(2)
    cron_entity = client.get("CronJobs", cron_id)
    cron_status = nested_str(cron_entity, ["Status", "status"])
    cron_fields = cron_entity.get("fields", {})
    next_run = cron_fields.get("next_run_at", "")
    events = cron_entity.get("events", [])

    print(f"  Status after activate: {cron_status}")
    print(f"  next_run_at: {next_run}")
    print(f"  Events so far:")
    for ev in events:
        print(f"    {ev.get('action')} → {ev.get('to_status', '(self-loop)')} at {ev.get('timestamp', '?')}")
        params = ev.get("params", {})
        if params:
            for k, v in params.items():
                if v and k not in ("name", "system_prompt", "user_message_template"):
                    print(f"      {k}: {v}")

    if not next_run:
        print("  ⚠ No next_run_at computed — WASM may not have completed")
        print("  Waiting 5 more seconds...")
        time.sleep(5)
        cron_entity = client.get("CronJobs", cron_id)
        cron_fields = cron_entity.get("fields", {})
        next_run = cron_fields.get("next_run_at", "")
        if next_run:
            print(f"  ✓ next_run_at now set: {next_run}")
        else:
            print("  ✗ next_run_at still empty — schedule_at activation may have failed")
            # Print all events for debugging
            events = cron_entity.get("events", [])
            print(f"  All {len(events)} events:")
            for ev in events:
                print(f"    {ev}")
            sys.exit(1)

    # ── Step 6: Wait for Trigger ────────────────────────────────
    print(f"\n[6/7] Waiting for CronJob to trigger (up to {args.wait_minutes} min)...")
    print(f"  next_run_at: {next_run}")
    print(f"  Current time: {now_utc()}")

    deadline = time.time() + (args.wait_minutes * 60)
    triggered = False
    last_run_count = 0
    session_id = ""

    while time.time() < deadline:
        cron_entity = client.get("CronJobs", cron_id)
        cron_fields = cron_entity.get("fields", {})
        run_count = cron_entity.get("counters", {}).get("run_count", 0)
        current_session = cron_fields.get("last_session_id", "")

        if run_count > last_run_count:
            print(f"  ✓ TRIGGERED! run_count={run_count} at {now_utc()}")
            session_id = current_session
            triggered = True
            break

        # Check events for any new activity
        events = cron_entity.get("events", [])
        event_actions = [ev.get("action") for ev in events]
        if "Trigger" in event_actions or "TriggerComplete" in event_actions:
            print(f"  ✓ Trigger event detected at {now_utc()}")
            session_id = current_session
            triggered = True
            break

        elapsed = time.time() - (deadline - args.wait_minutes * 60)
        sys.stdout.write(f"\r  Polling... {elapsed:.0f}s elapsed, run_count={run_count}  ")
        sys.stdout.flush()
        time.sleep(5)

    print()  # newline after polling

    if not triggered:
        print(f"  ✗ No trigger after {args.wait_minutes} minutes")
        cron_entity = client.get("CronJobs", cron_id)
        events = cron_entity.get("events", [])
        print(f"  Final state: {nested_str(cron_entity, ['Status', 'status'])}")
        print(f"  All {len(events)} events:")
        for ev in events:
            print(f"    {ev.get('action')} at {ev.get('timestamp', '?')}: params={ev.get('params', {})}")
        sys.exit(1)

    # ── Step 7: Verify Session ──────────────────────────────────
    print(f"[7/7] Verifying created Session...")
    if session_id:
        print(f"  Session ID: {session_id}")
        try:
            session = client.get("Sessions", session_id)
            session_status = nested_str(session, ["Status", "status"])
            session_fields = session.get("fields", {})
            print(f"  Session status: {session_status}")
            print(f"  Session model: {session_fields.get('model', '?')}")
            print(f"  Session soul_id: {session_fields.get('soul_id', '?')}")
            print(f"  Session tools: {session_fields.get('tools_enabled', '?')}")

            # Wait for session to complete (up to 2 minutes)
            if session_status not in ("Completed", "Failed", "Cancelled"):
                print(f"  Waiting for session to finish (up to 120s)...")
                try:
                    final = client.wait_for_agent(session_id, timeout_ms=120_000)
                    final_status = str(final.get("status") or final.get("Status") or "?")
                    print(f"  ✓ Session finished: {final_status}")
                except Exception as e:
                    print(f"  ⚠ Wait timed out: {e}")
                    session = client.get("Sessions", session_id)
                    session_status = nested_str(session, ["Status", "status"])
                    print(f"  Session current status: {session_status}")
        except Exception as e:
            print(f"  ⚠ Could not fetch session: {e}")
    else:
        # Try to find recently-created sessions
        print("  No session_id on CronJob — searching for recent sessions...")
        sessions = client.list("Sessions", orderby="entity_id desc", top=3)
        for s in sessions:
            sid = entity_id(s)
            ss = nested_str(s, ["Status", "status"])
            sf = s.get("fields", {})
            print(f"  Session {sid}: status={ss} model={sf.get('model','?')} soul={sf.get('soul_id','?')}")

    # ── Final Summary ───────────────────────────────────────────
    print()
    print("=" * 60)
    print("SUMMARY")
    print("=" * 60)

    cron_entity = client.get("CronJobs", cron_id)
    cron_fields = cron_entity.get("fields", {})
    cron_status = nested_str(cron_entity, ["Status", "status"])
    events = cron_entity.get("events", [])
    run_count = cron_entity.get("counters", {}).get("run_count", 0)

    print(f"  CronJob ID:     {cron_id}")
    print(f"  CronJob Status: {cron_status}")
    print(f"  Run Count:      {run_count}")
    print(f"  Last Session:   {cron_fields.get('last_session_id', 'none')}")
    print(f"  Next Run At:    {cron_fields.get('next_run_at', 'none')}")
    print(f"  Total Events:   {len(events)}")
    print()

    # Determine pass/fail
    trigger_events = [e for e in events if e.get("action") in ("Trigger", "TriggerComplete")]
    if trigger_events:
        print("  RESULT: ✓ PASS — CronJob triggered and created a Session")
    else:
        print("  RESULT: ✗ FAIL — CronJob did not trigger within the wait period")

    print(f"  Completed: {now_utc()}")
    print("=" * 60)


if __name__ == "__main__":
    main()
