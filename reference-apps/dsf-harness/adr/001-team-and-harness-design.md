# ADR-001: Deep Sci-Fi Team and Harness Design

**Status:** Accepted
**Date:** 2026-03-31
**Deciders:** Sesh (human), Ren (product lead)

## Context

Deep Sci-Fi is an AI social platform for sci-fi world-building with a mature codebase: Next.js 14 + FastAPI + PostgreSQL/pgvector, 12 Playwright E2E specs, Hypothesis DST, a 3-level CI harness, and production traffic. It needs a bespoke agent team — not generic SWE+SRE — because:

1. The codebase has domain-specific quality requirements (world coherence, scientific grounding) that generic agents don't understand.
2. The existing 3-level harness (pre-commit, pre-push, CI/post-deploy) should be enforced by the platform, not by agent discipline.
3. Different roles need different levels of identity — the product lead needs personality and judgment; SWE agents just need accurate conventions.

## Decision 1: Five Roles — One Soul, Four Skills

**Roles:**
- **Ren** (Product Lead, INTP) — the only soul. Owns scope, prioritization, agent coordination, ship/no-ship decisions. Has a full soul document with personality, worldview, communication style.
- **SWE** — skills only. Receives `swe-conventions.md` with repo layout, testing commands, migration rules, CI gates.
- **SRE** — skills only. Receives `sre-monitoring.md` with Datadog patterns, alert triage, health scan workflow.
- **Design** — skills only. Receives `design-system.md` with neo-editorial system, design tokens, component conventions.
- **Librarian** — skills only. Receives `content-standards.md` with world coherence assessment, scientific grounding checks.

**Rationale:** Only Ren needs persistent identity because product decisions require judgment shaped by personality and values. Other roles need accurate domain knowledge, not personality. Skill documents give them the right perspective without the overhead of soul management.

## Decision 2: State Machine Gates on WorkCycle

The existing 3-level harness is encoded as boolean gate fields on a `WorkCycleDSF` entity type:

| Gate | Type | Required for |
|------|------|-------------|
| `has_plan` | boolean | StartWork |
| `migrations_ok` | boolean | BeginTesting |
| `typecheck_ok` | boolean | BeginTesting |
| `unit_tests_ok` | boolean | BeginTesting |
| `dst_ok` | boolean | PassTests |
| `policy_gates_ok` | boolean | PassTests |
| `e2e_ok` | boolean | PassTests (optional) |
| `tests_passed` | boolean | Approve |

Agents report gate results via `Report*` actions (self-loops in InProgress state). The platform rejects transitions when guards aren't met — agents physically cannot skip checks.

**Rationale:** Encoding the harness as state machine guards makes it enforceable at the platform level. An agent can't say "I'll run tests later" — the transition to Testing literally won't work until migrations, typecheck, and unit tests are reported as passing.

## Decision 3: Auto-Injection of Harness Conventions

Agent entities carry a `project_harness_id` field pointing to a `ProjectHarness` entity. The `load_harness_block()` function in the LLM caller fetches the harness conventions and injects them into the agent's system prompt — similar to how CLAUDE.md works for Claude Code.

This means:
- Agents don't need to be told about repo layout, testing commands, or migration rules — they receive it automatically.
- Changing conventions in one place (the ProjectHarness entity) updates all agents on next invocation.
- Different projects can have completely different harness conventions.

**Rationale:** Convention injection is more reliable than training agents to remember rules. It's also more maintainable than updating multiple agent configurations when conventions change.

## Decision 4: Two-Layer Tool Governance

**Layer 1: Cedar-backed ToolHooks** — govern ALL tools including bash/CLIs like `gh`. Cedar policies define what each role can do with each tool. Examples:
- SWE can `gh pr create` but cannot `gh pr merge`
- SRE can `gh run view` but cannot modify workflows
- No agent can `git push --force` to main

**Layer 2: Temper-native API tools** — dedicated tools for Railway (GraphQL) and Vercel (REST) with vault-managed credentials. These tools call APIs directly instead of shelling out, enabling fine-grained Cedar governance.

**Rationale:** Layer 1 handles the common case (CLI tools) with broad policies. Layer 2 handles infrastructure APIs where we need credential scoping and audit trails. Together they provide comprehensive tool governance without requiring every tool to be a first-class Temper integration.

## Decision 5: Rust Cron Trigger Replaces WASM Polling

The original design had a CronScheduler WASM integration that polled entity state every 15 seconds (240 HTTP requests/hour). Replace this with a Rust cron trigger using `tokio::time::sleep_until` that fires at scheduled times and creates CronJob entities with a single action dispatch.

**Rationale:** 240 HTTP requests/hour for a cron scheduler is wasteful. A Rust trigger with `sleep_until` generates zero traffic between scheduled times and is more precise.

## Decision 6: Governed Sandbox Environments

The `Computer` entity type is enriched with:
- `tools_installed` — explicit list of available CLI tools
- `credentials_scoped` — which vault credentials are injected
- `network_allow` — allowlist of network destinations

This makes the agent's sandbox environment auditable and governable. Cedar policies can reference these fields to restrict what agents can do in their environment.

**Rationale:** Agents running in unrestricted environments with ambient credentials are a security risk. Explicit scoping makes the attack surface visible and controllable.

## Decision 7: Bespoke-First, Abstract Later

Build everything custom for deep-sci-fi first. Don't try to make it generic yet. Once we have a working reference implementation, identify patterns that should be generalized back into the `paw-harness` os-app.

**Rationale:** Premature abstraction creates leaky abstractions. Building bespoke first means we understand the actual requirements before generalizing. Deep-sci-fi becomes the reference implementation that proves the patterns work.

## Consequences

- **More upfront work** — custom state machine, custom skills, custom policies. But each piece is simple and well-defined.
- **Cleaner architecture** — the harness is enforced by the platform, not by agent discipline. Gates are guards, not suggestions.
- **Reference implementation** — deep-sci-fi proves the bespoke team + harness pattern works. Good patterns get extracted into paw-harness later.
- **Single soul simplicity** — only Ren needs soul management. Other roles are stateless skill injections, reducing complexity.
- **Audit trail** — every gate report, every tool invocation, every state transition is recorded. Full traceability from task to deploy.
