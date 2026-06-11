#!/usr/bin/env python3
"""Prune retired paw-foresight 0.1 permits from a tenant's Cedar policy.

Why this exists (ADR-003, risk register "Cedar text accumulation"):
os-app installs APPEND bundle policy text to the tenant policy and never
remove anything (temper-platform os_apps/mod.rs). After the corridor
retirement commit, stores that ever installed the 0.1 bundle still carry its
broad permits, so the retirement (read-only for non-system principals) does
not bind until those permits are pruned.

This script:
  1. GETs the effective tenant policy text.
  2. Splits it into Cedar statements (permit/forbid ... ;).
  3. Drops statements that grant non-read actions on the five retired entity
     types to unrestricted principals (the 0.1 surface), and drops exact
     duplicate statements (append-accumulation).
  4. Keeps everything else byte-for-byte — including runtime-approved
     decision rules — and PUTs the cleaned text back (validated server-side).

Usage:
  python3 scripts/prune_foresight_legacy_policy.py \
      --base-url http://127.0.0.1:4500 --tenant default --api-key <key> [--dry-run]
"""

from __future__ import annotations

import argparse
import json
import re
import sys
import urllib.request

LEGACY_RESOURCES = (
    "ForesightModel",
    "Projection",
    "Observation",
    "Direction",
    "DirectionFeedback",
)

LEGACY_WASM_MODULES = (
    "spawn_seed_agent",
    "spawn_probes",
    "handle_probe_done",
    "handle_convergence",
    "handle_projection_updated",
)


def split_statements(text: str) -> list[str]:
    """Split Cedar policy text into statements, keeping leading comments."""
    statements: list[str] = []
    buf: list[str] = []
    for line in text.splitlines():
        buf.append(line)
        if line.strip().endswith(";"):
            statements.append("\n".join(buf).strip())
            buf = []
    tail = "\n".join(buf).strip()
    if tail:
        statements.append(tail)  # trailing comments without a statement
    return statements


def statement_body(stmt: str) -> str:
    """The statement minus comment lines, whitespace-normalized for matching."""
    code = "\n".join(
        line for line in stmt.splitlines() if not line.strip().startswith("//")
    )
    return re.sub(r"\s+", " ", code).strip()


def is_retired_legacy_permit(stmt: str) -> bool:
    """The 0.1 surface: permits granting more than read/list on a legacy
    resource to principals that are not system-scoped."""
    body = statement_body(stmt)
    if not body.startswith("permit"):
        return False
    resource_hit = any(f"resource is {r}" in body for r in LEGACY_RESOURCES)
    module_hit = any(m in body for m in LEGACY_WASM_MODULES)
    if not resource_hit and not module_hit:
        return False
    if module_hit:
        return True  # old WASM http/secret permits: modules no longer exist
    # Read-only residue permits stay; system-scoped permits stay.
    mutating = bool(
        re.search(r'Action::"(?!read"|list")', body)
    )  # any action besides read/list
    system_scoped = 'principal.agent_type == "system"' in body
    broad_action = re.search(r"permit\s*\(\s*principal\s*,\s*action\s*,", body)
    return (mutating or bool(broad_action)) and not system_scoped


def prune(text: str) -> tuple[str, list[str], int]:
    statements = split_statements(text)
    kept: list[str] = []
    removed: list[str] = []
    seen_bodies: set[str] = set()
    dupes = 0
    for stmt in statements:
        body = statement_body(stmt)
        if body and body in seen_bodies:
            dupes += 1
            continue
        if is_retired_legacy_permit(stmt):
            removed.append(body[:100])
            continue
        if body:
            seen_bodies.add(body)
        kept.append(stmt)
    return "\n\n".join(kept) + "\n", removed, dupes


def http(method: str, url: str, api_key: str, tenant: str, body: dict | None = None):
    req = urllib.request.Request(url, method=method)
    req.add_header("Authorization", f"Bearer {api_key}")
    req.add_header("X-Tenant-Id", tenant)
    req.add_header("X-Temper-Principal-Kind", "admin")
    data = None
    if body is not None:
        req.add_header("Content-Type", "application/json")
        data = json.dumps(body).encode()
    with urllib.request.urlopen(req, data=data) as resp:
        return json.loads(resp.read().decode())


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--base-url", required=True)
    ap.add_argument("--tenant", default="default")
    ap.add_argument("--api-key", required=True)
    ap.add_argument("--dry-run", action="store_true")
    args = ap.parse_args()

    url = f"{args.base_url}/api/tenants/{args.tenant}/policies"
    current = http("GET", url, args.api_key, args.tenant)
    text = current.get("policy_text", "")
    if not text:
        print("tenant policy is empty; nothing to prune")
        return 0

    cleaned, removed, dupes = prune(text)
    print(f"statements removed (retired 0.1 surface): {len(removed)}")
    for r in removed:
        print(f"  - {r}")
    print(f"duplicate statements collapsed: {dupes}")
    print(f"policy size: {len(text)} -> {len(cleaned)} bytes")

    if args.dry_run:
        print("dry run: not writing")
        return 0
    if not removed and not dupes:
        print("nothing to change")
        return 0

    result = http("PUT", url, args.api_key, args.tenant, {"policy_text": cleaned})
    print(f"PUT result: {result}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
