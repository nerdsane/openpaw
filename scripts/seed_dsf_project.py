#!/usr/bin/env python3
"""Seed the Deep Sci-Fi project: one Harness, Ren's Soul, six Skills, and ToolHooks.

This script is idempotent — it checks for existing entities before creating new
ones.  It reads skill and soul content from reference-projects/deep-sci-fi/.

Usage:
    python scripts/seed_dsf_project.py [--base-url URL] [--tenant TENANT]
"""

from __future__ import annotations

import argparse
import os
import sys
import urllib.parse
import urllib.request

sys.path.insert(0, os.path.dirname(__file__))

from openpaw_proof_support import ODataClient, entity_id, nested_str, now_utc

REPO_URL = "https://github.com/arni-labs/deep-sci-fi.git"
REFERENCE_DIR = os.path.join(
    os.path.dirname(__file__), os.pardir, "reference-projects", "deep-sci-fi"
)

# ---------------------------------------------------------------------------
# Skill definitions — the six DSF skills
# ---------------------------------------------------------------------------

SKILLS = [
    {
        "name": "swe-conventions",
        "description": "SWE coding conventions for the deep-sci-fi codebase.",
        "scope": "soul",
        "agent_filter": "SWE",
        "file": "dsf-team/skills/swe-conventions.md",
    },
    {
        "name": "sre-monitoring",
        "description": "SRE monitoring, alert triage, and health-scan workflows.",
        "scope": "soul",
        "agent_filter": "SRE",
        "file": "dsf-team/skills/sre-monitoring.md",
    },
    {
        "name": "reviewer-code",
        "description": "Code review conventions and quality checklist.",
        "scope": "soul",
        "agent_filter": "CodeReviewer",
        "file": "dsf-team/skills/reviewer-code.md",
    },
    {
        "name": "reviewer-dst",
        "description": "Deterministic simulation testing review checklist.",
        "scope": "soul",
        "agent_filter": "DSTReviewer",
        "file": "dsf-team/skills/reviewer-dst.md",
    },
    {
        "name": "design-system",
        "description": "Neo-editorial design system conventions and tokens.",
        "scope": "soul",
        "agent_filter": "Design",
        "file": "dsf-team/skills/design-system.md",
    },
    {
        "name": "content-standards",
        "description": "Content quality, world coherence, and scientific grounding.",
        "scope": "soul",
        "agent_filter": "Librarian",
        "file": "dsf-team/skills/content-standards.md",
    },
]


# ---------------------------------------------------------------------------
# Helpers
# ---------------------------------------------------------------------------


def read_reference_file(relative_path: str) -> str:
    """Read a file from the reference-projects/deep-sci-fi directory."""
    full = os.path.normpath(os.path.join(REFERENCE_DIR, relative_path))
    with open(full, "r", encoding="utf-8") as f:
        return f.read()


def find_by_name(client: ODataClient, entity_set: str, name: str) -> dict | None:
    """Return the first entity matching the given name, or None."""
    results = client.list(entity_set, filter_expr=f"name eq '{name}'", top=1)
    return results[0] if results else None


def find_harness_by_repo(client: ODataClient, repo_url: str) -> dict | None:
    """Return the first Harness matching the given repo_url, or None."""
    results = client.list(
        "Harnesses", filter_expr=f"repo_url eq '{repo_url}'", top=1
    )
    return results[0] if results else None


def upload_file(client: ODataClient, name: str, content: str) -> str:
    """Create a File entity and upload content. Returns the file ID."""
    file_entity = client.create("Files", {"Name": name, "MimeType": "text/markdown"})
    file_id = entity_id(file_entity)
    if not file_id:
        raise RuntimeError(f"File entity creation for '{name}' returned no ID")

    # Upload content via PUT $value
    url = f"{client.base_url}/tdata/Files('{file_id}')/$value"
    headers = {
        "X-Tenant-Id": client.tenant,
        "Content-Type": "text/markdown",
    }
    if client.api_key:
        headers["Authorization"] = f"Bearer {client.api_key}"
    data = content.encode("utf-8")
    req = urllib.request.Request(url, data=data, method="PUT", headers=headers)
    try:
        with urllib.request.urlopen(req, timeout=60) as resp:
            resp.read()
    except urllib.error.HTTPError as exc:
        payload = exc.read().decode("utf-8", errors="replace")
        raise RuntimeError(
            f"File upload for '{name}' failed ({exc.code}): {payload}"
        ) from exc

    return file_id


# ---------------------------------------------------------------------------
# Seed functions
# ---------------------------------------------------------------------------


def seed_harness(client: ODataClient) -> str:
    """Create or find the deep-sci-fi Harness. Returns its entity ID."""
    existing = find_harness_by_repo(client, REPO_URL)
    if existing:
        hid = entity_id(existing)
        status = nested_str(existing, ["Status", "status"])
        print(f"  [exists] Harness repo={REPO_URL} ({hid}) -- {status}")
        return hid

    harness = client.create("Harnesses")
    hid = entity_id(harness)
    client.action(
        "Harnesses",
        hid,
        "OpenPaw.Harness.Configure",
        {
            "repo_url": REPO_URL,
            "tech_stack": "Next.js 14 (App Router), TypeScript, Tailwind, Supabase, Python",
            "conventions": read_reference_file("dsf-team/skills/swe-conventions.md"),
            "work_cycle_type": "WorkCycles",
        },
    )
    client.action(
        "Harnesses",
        hid,
        "OpenPaw.Harness.Activate",
        {"last_activated_at": now_utc()},
    )
    print(f"  [created] Harness repo={REPO_URL} ({hid})")
    return hid


def seed_soul(client: ODataClient) -> str:
    """Create or find Ren's Soul. Uploads SOUL.md and STYLE.md. Returns soul ID."""
    existing = find_by_name(client, "Souls", "Ren")
    if existing:
        sid = entity_id(existing)
        status = nested_str(existing, ["Status", "status"])
        print(f"  [exists] Soul 'Ren' ({sid}) -- {status}")
        return sid

    # Upload soul and style files
    soul_content = read_reference_file("dsf-team/souls/ren/SOUL.md")
    style_content = read_reference_file("dsf-team/souls/ren/STYLE.md")

    soul_file_id = upload_file(client, "ren-soul.md", soul_content)
    print(f"    uploaded SOUL.md -> {soul_file_id}")
    style_file_id = upload_file(client, "ren-style.md", style_content)
    print(f"    uploaded STYLE.md -> {style_file_id}")

    soul = client.create("Souls", {
        "Name": "Ren",
        "Description": "Product lead for Deep Sci-Fi (INTP). Owns scope, prioritization, agent coordination.",
        "ContentFileId": soul_file_id,
    })
    sid = entity_id(soul)
    client.action("Souls", sid, "OpenPaw.Publish", {})
    print(f"  [created] Soul 'Ren' ({sid})")
    return sid


def seed_skill(client: ODataClient, skill_def: dict) -> str:
    """Create or find a Skill entity. Returns its entity ID."""
    name = skill_def["name"]
    existing = find_by_name(client, "Skills", name)
    if existing:
        skid = entity_id(existing)
        status = nested_str(existing, ["Status", "status"])
        print(f"  [exists] Skill '{name}' ({skid}) -- {status}")
        return skid

    # Upload skill content file
    content = read_reference_file(skill_def["file"])
    file_id = upload_file(client, f"{name}.md", content)

    skill = client.create("Skills")
    skid = entity_id(skill)
    client.action(
        "Skills",
        skid,
        "OpenPaw.Register",
        {
            "name": name,
            "description": skill_def["description"],
            "scope": skill_def["scope"],
            "agent_filter": skill_def["agent_filter"],
            "content_file_id": file_id,
        },
    )
    print(f"  [created] Skill '{name}' ({skid})")
    return skid


def seed_tool_hooks(client: ODataClient) -> None:
    """Create ToolHook entities if the entity set exists."""
    try:
        existing = client.list("ToolHooks", top=1)
    except RuntimeError:
        print("  [skip] ToolHooks entity set not available")
        return

    if existing:
        print(f"  [exists] {len(existing)}+ ToolHook(s) already present")
        return

    # Standard tool hooks for the DSF project
    hooks = [
        {
            "name": "pre-commit-lint",
            "description": "Run lint checks before committing",
            "trigger_action": "WorkCycle.StartWork",
            "command": "cd platform && npm run lint",
        },
        {
            "name": "typecheck",
            "description": "Run TypeScript type checking",
            "trigger_action": "WorkCycle.ReportTypecheck",
            "command": "cd platform && npx tsc --noEmit",
        },
    ]
    for hook_def in hooks:
        try:
            hook = client.create("ToolHooks")
            hid = entity_id(hook)
            client.action(
                "ToolHooks",
                hid,
                "OpenPaw.Register",
                hook_def,
            )
            print(f"  [created] ToolHook '{hook_def['name']}' ({hid})")
        except RuntimeError as e:
            print(f"  [skip] ToolHook '{hook_def['name']}': {e}")


# ---------------------------------------------------------------------------
# Main
# ---------------------------------------------------------------------------


def main() -> None:
    parser = argparse.ArgumentParser(
        description="Seed the Deep Sci-Fi project: Harness, Soul, Skills, ToolHooks."
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
    args = parser.parse_args()

    client = ODataClient(args.base_url, args.tenant, args.api_key)

    print("=== Seeding Deep Sci-Fi Project ===\n")

    print("1. Harness")
    harness_id = seed_harness(client)

    print("\n2. Soul (Ren)")
    seed_soul(client)

    print("\n3. Skills")
    for skill_def in SKILLS:
        seed_skill(client, skill_def)

    print("\n4. ToolHooks")
    seed_tool_hooks(client)

    print(f"\nDone. Harness ID: {harness_id}")


if __name__ == "__main__":
    main()
