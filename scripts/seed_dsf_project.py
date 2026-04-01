#!/usr/bin/env python3
"""Seed the DSF project as lead agent.

Installs reference apps and creates all entity instances:
- Ren's Soul (with SOUL.md + STYLE.md content uploaded to TemperFS)
- 6 Skills (with markdown content uploaded to TemperFS)
- Harness (with full conventions)
- ToolHooks (Cedar-backed command governance)

Idempotent: safe to run multiple times. Existing entities are detected
by OData $filter and skipped.

Usage: python3 scripts/seed_dsf_project.py [--base-url URL] [--tenant TENANT]
"""

from __future__ import annotations

import argparse
import json
import os
import sys
import urllib.error
import urllib.request

from openpaw_proof_support import (
    DEFAULT_BASE_URL,
    DEFAULT_TENANT,
    ODataClient,
    entity_id,
    now_utc,
    require,
)

# ---------------------------------------------------------------------------
# Skill metadata keyed by filename stem.  description, scope, and
# agent_filter mirror the Skill spec's Register action params.
# ---------------------------------------------------------------------------
SKILL_DEFS: dict[str, dict[str, str]] = {
    "swe-conventions": {
        "description": "Deep-sci-fi SWE conventions and codebase guide",
        "scope": "deep-sci-fi",
        "agent_filter": "SWE",
    },
    "design-system": {
        "description": "Neo-editorial design system for deep-sci-fi",
        "scope": "deep-sci-fi",
        "agent_filter": "",
    },
    "content-standards": {
        "description": "Scientific grounding and content coherence standards",
        "scope": "deep-sci-fi",
        "agent_filter": "",
    },
    "sre-monitoring": {
        "description": "Datadog monitoring patterns and scan workflow",
        "scope": "deep-sci-fi",
        "agent_filter": "SRE",
    },
    "reviewer-code": {
        "description": "Code quality review protocol",
        "scope": "deep-sci-fi",
        "agent_filter": "CodeReviewer",
    },
    "reviewer-dst": {
        "description": "DST compliance review protocol",
        "scope": "deep-sci-fi",
        "agent_filter": "DSTReviewer",
    },
}

# ToolHook definitions — Register action params from tool_hook.ioa.toml
TOOL_HOOK_DEFS: list[dict[str, str]] = [
    {
        "name": "block-gh-pr-merge",
        "hook_type": "before",
        "tool_pattern": "bash",
        "hook_action": "block",
        "soul_id": "SWE",
        "cedar_enabled": "true",
        "command_pattern": "gh pr merge",
        "project_scope": "deep-sci-fi",
    },
    {
        "name": "log-gh-pr-create",
        "hook_type": "before",
        "tool_pattern": "bash",
        "hook_action": "log",
        "soul_id": "SWE",
        "cedar_enabled": "true",
        "command_pattern": "gh pr create",
        "project_scope": "deep-sci-fi",
    },
]


# ---------------------------------------------------------------------------
# Helpers
# ---------------------------------------------------------------------------

def install_os_app(base_url: str, tenant: str, app_name: str) -> None:
    """POST to the platform's OS-app install endpoint.

    Tolerates 409 (already installed) and missing endpoints gracefully.
    """
    req = urllib.request.Request(
        f"{base_url}/api/os-apps/{app_name}/install",
        data=json.dumps({"tenant": tenant}).encode(),
        method="POST",
        headers={
            "Content-Type": "application/json",
            "X-Tenant-Id": tenant,
            "X-Temper-Principal-Kind": "admin",
        },
    )
    try:
        with urllib.request.urlopen(req, timeout=60) as resp:
            print(f"  Installed {app_name}: HTTP {resp.status}")
    except urllib.error.HTTPError as exc:
        body = exc.read().decode("utf-8", errors="replace")
        if exc.code == 409 or "already installed" in body.lower():
            print(f"  {app_name}: already installed (409)")
        else:
            print(f"  {app_name}: install returned {exc.code} — {body[:200]}")
    except Exception as exc:  # noqa: BLE001
        print(f"  {app_name}: install error — {exc}")


def upload_file(client: ODataClient, name: str, content: str) -> str:
    """Create a TemperFS File entity and upload markdown content.

    Returns the File entity ID.
    """
    file_entity = client.create("Files", {"Name": name, "MimeType": "text/markdown"})
    fid = entity_id(file_entity)
    require(bool(fid), f"failed to create File entity for {name}")

    # Upload content via PUT to $value stream endpoint
    req = urllib.request.Request(
        f"{client.base_url}/tdata/Files('{fid}')/$value",
        data=content.encode("utf-8"),
        method="PUT",
        headers={
            "Content-Type": "text/markdown",
            "X-Tenant-Id": client.tenant,
            "X-Temper-Principal-Kind": "admin",
        },
    )
    urllib.request.urlopen(req, timeout=30)
    return fid


# ---------------------------------------------------------------------------
# Main
# ---------------------------------------------------------------------------

def main() -> int:
    parser = argparse.ArgumentParser(description="Seed the DSF reference project.")
    parser.add_argument("--base-url", default=os.getenv("OPENPAW_BASE_URL", DEFAULT_BASE_URL))
    parser.add_argument("--tenant", default=os.getenv("PAW_TENANT", DEFAULT_TENANT))
    args = parser.parse_args()

    client = ODataClient(
        base_url=args.base_url,
        tenant=args.tenant,
        api_key=os.getenv("TEMPER_API_KEY") or None,
    )

    ref_base = os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "reference-projects", "deep-sci-fi")

    # ------------------------------------------------------------------
    # Step 1: Install DSF reference OS apps
    # ------------------------------------------------------------------
    print("Step 1: Installing DSF reference apps...")
    for app in ["dsf-harness", "dsf-team"]:
        install_os_app(args.base_url, args.tenant, app)

    # ------------------------------------------------------------------
    # Step 2: Create Ren's Soul
    # ------------------------------------------------------------------
    print("\nStep 2: Creating Ren's Soul...")
    existing_souls = client.list("Souls", filter_expr="Name eq 'Ren'")
    if existing_souls:
        soul_id = entity_id(existing_souls[0])
        print(f"  Ren already exists: {soul_id}")
    else:
        # Read and combine SOUL.md + STYLE.md
        soul_path = os.path.join(ref_base, "dsf-team", "souls", "ren", "SOUL.md")
        style_path = os.path.join(ref_base, "dsf-team", "souls", "ren", "STYLE.md")
        soul_content = open(soul_path, encoding="utf-8").read()
        style_content = open(style_path, encoding="utf-8").read()
        combined = soul_content + "\n\n---\n\n" + style_content

        # Upload combined content to TemperFS
        file_id = upload_file(client, "ren-soul.md", combined)

        # Create Soul with fields in POST body (matches bootstrap_soul pattern)
        soul_entity = client.create("Souls", {
            "Name": "Ren",
            "Description": "Product lead for Deep Sci-Fi (INTP)",
            "ContentFileId": file_id,
        })
        soul_id = entity_id(soul_entity)
        require(bool(soul_id), "failed to create Soul entity")

        # Publish (Draft -> Active)
        client.action("Souls", soul_id, "OpenPaw.Publish", {})
        print(f"  Created and published Ren: soul={soul_id}, file={file_id}")

    # ------------------------------------------------------------------
    # Step 3: Create Skills
    # ------------------------------------------------------------------
    print("\nStep 3: Creating Skills...")
    skills_dir = os.path.join(ref_base, "dsf-team", "skills")

    for skill_stem, meta in SKILL_DEFS.items():
        skill_name = f"dsf-{skill_stem}"
        existing = client.list("Skills", filter_expr=f"name eq '{skill_name}'")
        if existing:
            print(f"  {skill_name}: already exists ({entity_id(existing[0])})")
            continue

        # Read skill markdown
        skill_path = os.path.join(skills_dir, f"{skill_stem}.md")
        content = open(skill_path, encoding="utf-8").read()

        # Upload to TemperFS
        file_id = upload_file(client, f"{skill_name}.skill.md", content)

        # Create Skill entity (starts Active) and Register
        sk = client.create("Skills")
        sk_id = entity_id(sk)
        require(bool(sk_id), f"failed to create Skill entity for {skill_name}")

        client.action("Skills", sk_id, "OpenPaw.Register", {
            "name": skill_name,
            "description": meta["description"],
            "content_file_id": file_id,
            "scope": meta["scope"],
            "agent_filter": meta["agent_filter"],
        })
        print(f"  {skill_name}: created (skill={sk_id}, file={file_id})")

    # ------------------------------------------------------------------
    # Step 4: Create Harness
    # ------------------------------------------------------------------
    print("\nStep 4: Creating Harness...")
    repo_url = "https://github.com/arni-labs/deep-sci-fi.git"
    existing_harnesses = client.list("Harnesses", filter_expr=f"repo_url eq '{repo_url}'")
    if existing_harnesses:
        harness_id = entity_id(existing_harnesses[0])
        print(f"  Harness already exists: {harness_id}")
    else:
        # Read conventions from dsf_work_cycle spec (authoritative source)
        conventions = (
            "3-level CI gate: unit+integration → DST (Hypothesis) → Playwright E2E. "
            "All API endpoints require response_model declarations. "
            "Post-deploy verification is non-negotiable (health checks, smoke tests, Logfire 500 monitoring). "
            "Prefer focused fixes; validate in platform/; open PR with concrete reproduction notes. "
            "Alembic migrations must be reviewed for backward compatibility."
        )
        tech_stack = (
            "Next.js 14 App Router + RSC, TypeScript, Tailwind CSS, Framer Motion, D3.js | "
            "FastAPI, async SQLAlchemy, Alembic, PostgreSQL 15, pgvector | "
            "Hypothesis DST, pytest, Vitest, Playwright E2E | "
            "Logfire observability, Datadog monitoring | "
            "Vercel (frontend), Railway + Docker (backend), GitHub Actions CI/CD"
        )

        h = client.create("Harnesses")
        harness_id = entity_id(h)
        require(bool(harness_id), "failed to create Harness")

        client.action("Harnesses", harness_id, "OpenPaw.Harness.Configure", {
            "repo_url": repo_url,
            "tech_stack": tech_stack,
            "conventions": conventions,
            "work_cycle_type": "DsfWorkCycles",
        })
        client.action("Harnesses", harness_id, "OpenPaw.Harness.Activate", {
            "last_activated_at": now_utc(),
        })
        print(f"  Created and activated harness: {harness_id}")

    # ------------------------------------------------------------------
    # Step 5: Create ToolHooks
    # ------------------------------------------------------------------
    print("\nStep 5: Creating ToolHooks...")
    for hook_def in TOOL_HOOK_DEFS:
        hook_name = hook_def["name"]
        existing = client.list("ToolHooks", filter_expr=f"name eq '{hook_name}'")
        if existing:
            print(f"  {hook_name}: already exists ({entity_id(existing[0])})")
            continue

        th = client.create("ToolHooks")
        th_id = entity_id(th)
        require(bool(th_id), f"failed to create ToolHook for {hook_name}")

        client.action("ToolHooks", th_id, "OpenPaw.Register", hook_def)
        print(f"  {hook_name}: created ({th_id})")

    # ------------------------------------------------------------------
    # Summary
    # ------------------------------------------------------------------
    print("\n--- DSF project seeded successfully ---")
    print(f"  Harness: {harness_id}")
    print(f"  Soul:    Ren")
    print(f"  Skills:  {len(SKILL_DEFS)}")
    print(f"  Hooks:   {len(TOOL_HOOK_DEFS)}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
