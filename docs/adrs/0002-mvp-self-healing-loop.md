# ADR-0002: MVP Self-Healing Loop

## Status

Accepted

## Context

Open Paw's first demo needs to show the full self-healing loop on deep-sci-fi: monitoring detects issues, scout triages, developer fixes, PR pushed. This ADR documents the decisions for the MVP iteration.

## Decisions

1. **E2B sandboxes** for developer agents (not Fly Sprites). Existing code, faster to working.
2. **Logfire alerts** (not Datadog) for monitoring. Deep-sci-fi already uses Logfire.
3. **Entity rename**: TemperAgent → Agent, AgentSoul → Soul, etc. Namespace: OpenPaw.
4. **Two new OS apps**: paw-harness (development workflow) and paw-heal (self-healing monitors).
5. **Parallel implementation**: Claude and Codex implement independently on separate worktrees.
6. **Proof reports**: Every step produces a verification report in `.proofs/`.

## Consequences

- E2B sandboxes are ephemeral (24hr max) — acceptable for MVP, Sprites added later
- Logfire integration is specific to deep-sci-fi's setup — generalizes later
- Entity rename requires WASM recompilation
