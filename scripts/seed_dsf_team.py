#!/usr/bin/env python3
"""Seed the Deep Sci-Fi team: one Team entity and seven Agent entities.

This script is idempotent — it checks for existing entities by name before
creating new ones. It uses the ODataClient from openpaw_proof_support.py.

Usage:
    python scripts/seed_dsf_team.py [--base-url URL] [--tenant TENANT]
"""

from __future__ import annotations

import argparse
import os
import sys

sys.path.insert(0, os.path.dirname(__file__))

from openpaw_proof_support import ODataClient, entity_id, nested_str


# ---------------------------------------------------------------------------
# Agent definitions
# ---------------------------------------------------------------------------

AGENTS = [
    {
        "name": "Ren",
        "role": "Lead",
        "description": "Product lead. Owns scope, prioritization, agent coordination, ship/no-ship decisions.",
        "soul_id": "Ren",
        "model": "claude-sonnet-4-6",
        "provider": "anthropic",
        "tools_enabled": "temper_create,temper_get,temper_list,temper_action,temper_wait,read,write,edit,bash",
        "max_turns": "200",
        "skill_ids": "",
    },
    {
        "name": "SWE",
        "role": "SWE",
        "description": "Software engineer. Writes code, runs tests, creates PRs.",
        "soul_id": "SWE",
        "model": "claude-sonnet-4-6",
        "provider": "anthropic",
        "tools_enabled": "read,write,edit,bash,temper_action,temper_get,temper_list",
        "max_turns": "200",
        "skill_ids": "",
    },
    {
        "name": "SRE",
        "role": "SRE",
        "description": "Site reliability engineer. Monitors, triages alerts, runs health scans.",
        "soul_id": "SRE",
        "model": "claude-sonnet-4-6",
        "provider": "anthropic",
        "tools_enabled": "temper_get,temper_list,temper_action,bash,read",
        "max_turns": "200",
        "skill_ids": "",
    },
    {
        "name": "Design",
        "role": "Design",
        "description": "Design system steward. Maintains neo-editorial system, design tokens, component conventions.",
        "soul_id": "",
        "model": "claude-sonnet-4-6",
        "provider": "anthropic",
        "tools_enabled": "read,write,edit,bash,temper_get,temper_list",
        "max_turns": "200",
        "skill_ids": "",
    },
    {
        "name": "Librarian",
        "role": "Librarian",
        "description": "Content standards guardian. Assesses world coherence, scientific grounding.",
        "soul_id": "",
        "model": "claude-sonnet-4-6",
        "provider": "anthropic",
        "tools_enabled": "temper_get,temper_list,temper_action,read",
        "max_turns": "200",
        "skill_ids": "",
    },
    {
        "name": "CodeReviewer",
        "role": "CodeReviewer",
        "description": "Code reviewer. Reviews PRs for correctness, style, and convention compliance.",
        "soul_id": "",
        "model": "claude-sonnet-4-6",
        "provider": "anthropic",
        "tools_enabled": "bash,temper_action,temper_get,temper_list,read",
        "max_turns": "200",
        "skill_ids": "",
    },
    {
        "name": "DSTReviewer",
        "role": "DSTReviewer",
        "description": "Domain-specific testing reviewer. Runs and reviews Hypothesis DST, property-based tests.",
        "soul_id": "",
        "model": "claude-sonnet-4-6",
        "provider": "anthropic",
        "tools_enabled": "bash,temper_action,temper_get,temper_list,read",
        "max_turns": "200",
        "skill_ids": "",
    },
]


def find_by_name(client: ODataClient, entity_set: str, name: str) -> dict | None:
    """Return the first entity matching the given name, or None."""
    results = client.list(entity_set, filter_expr=f"name eq '{name}'", top=1)
    return results[0] if results else None


def seed_team(client: ODataClient) -> str:
    """Create or find the Deep Sci-Fi Team. Returns its entity ID."""
    existing = find_by_name(client, "Teams", "Deep Sci-Fi Team")
    if existing:
        tid = entity_id(existing)
        status = nested_str(existing, ["Status", "status"])
        print(f"  [exists] Team 'Deep Sci-Fi Team' ({tid}) — {status}")
        return tid

    team = client.create("Teams")
    tid = entity_id(team)
    client.action("Teams", tid, "OpenPaw.Configure", {
        "name": "Deep Sci-Fi Team",
        "description": "AI agent team for the Deep Sci-Fi social platform. Seven roles: Lead, SWE, SRE, Design, Librarian, CodeReviewer, DSTReviewer.",
        "harness_id": "",  # Link to Harness once created
    })
    print(f"  [created] Team 'Deep Sci-Fi Team' ({tid})")
    return tid


def seed_agent(client: ODataClient, team_id: str, agent_def: dict) -> str:
    """Create or find an Agent entity. Returns its entity ID."""
    name = agent_def["name"]
    existing = find_by_name(client, "Agents", name)
    if existing:
        aid = entity_id(existing)
        status = nested_str(existing, ["Status", "status"])
        print(f"  [exists] Agent '{name}' ({aid}) — {status}")
        return aid

    agent = client.create("Agents")
    aid = entity_id(agent)
    client.action("Agents", aid, "OpenPaw.Configure", {
        "name": name,
        "role": agent_def["role"],
        "description": agent_def["description"],
        "soul_id": agent_def["soul_id"],
        "team_id": team_id,
        "model": agent_def["model"],
        "provider": agent_def["provider"],
        "tools_enabled": agent_def["tools_enabled"],
        "max_turns": agent_def["max_turns"],
        "skill_ids": agent_def["skill_ids"],
    })
    print(f"  [created] Agent '{name}' ({aid})")
    return aid


def main() -> None:
    parser = argparse.ArgumentParser(description="Seed the Deep Sci-Fi team and agents.")
    parser.add_argument("--base-url", default=os.environ.get("TEMPER_BASE_URL", "http://127.0.0.1:3467"))
    parser.add_argument("--tenant", default=os.environ.get("TEMPER_TENANT", "default"))
    parser.add_argument("--api-key", default=os.environ.get("TEMPER_API_KEY"))
    args = parser.parse_args()

    client = ODataClient(args.base_url, args.tenant, args.api_key)

    print("Seeding Deep Sci-Fi Team...")
    team_id = seed_team(client)

    print(f"\nSeeding agents (team_id={team_id})...")
    for agent_def in AGENTS:
        seed_agent(client, team_id, agent_def)

    print("\nDone. Team and all agents seeded.")


if __name__ == "__main__":
    main()
