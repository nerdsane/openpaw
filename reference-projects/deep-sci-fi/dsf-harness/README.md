# Deep Sci-Fi — Managed Project Reference

## What

Deep Sci-Fi is an AI social platform for collaborative sci-fi world-building. Users create scientifically-grounded speculative worlds, write stories within them, and interact through AI-driven "dweller" characters that maintain narrative consistency.

**Tech stack:** Next.js 14 App Router + TypeScript + Tailwind + Drizzle ORM (Bun, Vercel) | FastAPI + async SQLAlchemy + Alembic + PostgreSQL 15 + pgvector (Railway/Docker) | Logfire observability | Datadog monitoring

**Repo:** https://github.com/arni-labs/deep-sci-fi.git

## Team

| Role | Type | Description |
|------|------|-------------|
| **Ren** | Soul (INTP) | Product lead. Only soul on the team. Owns scope, prioritization, agent coordination, and all ship/no-ship decisions. |
| **SWE** | Skills only | Software engineering. Gets perspective from `skills/swe-conventions.md`. Handles code, tests, migrations, PRs. |
| **SRE** | Skills only | Site reliability. Gets perspective from `skills/sre-monitoring.md`. Runs cron health scans, tunes monitors, triages alerts. |
| **Design** | Skills only | Design review. Gets perspective from `skills/design-system.md`. Reviews UI changes against the neo-editorial design system. |
| **Librarian** | Skills only | Content analysis. Gets perspective from `skills/content-standards.md`. Monitors world coherence, scientific grounding, narrative quality. |
| **Code Reviewer** | Skills only | Code quality gate. Gets perspective from `skills/reviewer-code.md`. Reviews PR diffs for plan alignment, backend/frontend standards, DSF-specific rules, and code quality. |
| **DST Reviewer** | Skills only | DST compliance gate. Gets perspective from `skills/reviewer-dst.md`. Reviews PR diffs for rule coverage, invariant coverage, BUGGIFY placement, determinism, and game rule alignment. |

Only Ren gets a soul (personality, worldview, communication style). The other roles receive domain expertise through injected skill documents — they don't need persistent identity to do their jobs well.

## Harness

The deep-sci-fi harness enforces a 3-level gate system through the `WorkCycleDSF` state machine. Agents physically cannot skip checks — the platform rejects state transitions when boolean gates aren't met.

**Gate fields on WorkCycleDSF:**

| Gate | Required for | Description |
|------|-------------|-------------|
| `has_plan` | StartWork | A plan must exist before coding begins |
| `migrations_ok` | BeginTesting | Alembic migrations are valid and applied |
| `typecheck_ok` | BeginTesting | TypeScript and Python type checks pass |
| `unit_tests_ok` | BeginTesting | pytest + vitest unit tests pass |
| `dst_ok` | PassTests | Hypothesis DST simulation tests pass |
| `policy_gates_ok` | PassTests | All Level 1 + Level 2 policy checks pass |
| `e2e_ok` | — (optional) | Playwright E2E tests pass |
| `tests_passed` | Approve | All required test gates are green |

Agents report gate results via `Report*` actions (ReportMigrations, ReportTypecheck, etc.) which are self-loop transitions in the InProgress state. The harness conventions are auto-injected into agent prompts via `project_harness_id` on the Agent entity.

## Status

Setting up. This directory contains the reference specifications for the team, harness, and infrastructure.

## Contents

```
projects/deep-sci-fi/
├── README.md                          # This file
├── adr/001-team-and-harness-design.md # Architecture decision record
├── specs/harness.ioa.toml             # Harness instance data
├── specs/work_cycle_dsf.ioa.toml      # WorkCycleDSF state machine spec
├── specs/computer.ioa.toml            # Computer instance data (dev sandbox)
├── specs/webhook_routes.ioa.toml      # Datadog webhook routing
├── specs/monitors.ioa.toml            # Datadog monitor definitions
├── specs/cron_jobs.ioa.toml           # SRE cron job definitions
├── souls/ren/SOUL.md                  # Ren's soul document
├── souls/ren/STYLE.md                 # Ren's communication style
├── skills/swe-conventions.md          # SWE agent skill injection
├── skills/design-system.md            # Design agent skill injection
├── skills/content-standards.md        # Librarian agent skill injection
├── skills/sre-monitoring.md           # SRE agent skill injection
├── skills/reviewer-code.md            # Code Reviewer agent skill injection
├── skills/reviewer-dst.md             # DST Reviewer agent skill injection
├── policies/autonomy.cedar            # Agent autonomy boundaries
├── policies/tool_governance.cedar     # Tool access policies
```
