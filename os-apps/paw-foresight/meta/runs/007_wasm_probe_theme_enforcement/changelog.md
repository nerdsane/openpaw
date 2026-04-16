# Run 007 Changelog

## Changed File
`os-apps/paw-foresight/wasm/spawn_orchestrator/src/lib.rs`

## What Changed

Moved probe session creation from the orchestrator (LLM-driven, unreliable) into the WASM
module (deterministic, structurally enforced). Each of 6 probe sessions now receives a
hard-coded theme constraint for its directions via the WASM.

## Before
- WASM created 1 orchestrator session
- Orchestrator's instructions included probe creation, convergence, direction consolidation,
  and synthesis delegation (~8KB of prose instructions)
- Orchestrator decided how to create probes and what personas to assign
- All 6 probes produced governance-heavy directions because no theme constraints were enforced
- Across Runs 001-006, the orchestrator consistently shortcut convergence, consolidation, and delegation

## After
- WASM creates **6 probe sessions** directly, each with a hard-coded prompt including:
  - Persona (practitioner, critic, adjacent-domain)
  - Step and time range
  - **MANDATORY theme constraint** for directions
  - Domain context summary and web search instructions
- WASM then creates **1 orchestrator session** with simplified instructions:
  - Wait for 6 probes (session IDs provided by WASM)
  - Read observations and directions
  - Delegate synthesis to a dedicated session
- Added `create_configured_session()` helper function (Agent + Session + Configure)
- Added `build_probe_prompt()` helper for filling the probe template

## Theme Assignments (Structural)

| Probes | Theme Constraint | Expected Direction Themes |
|--------|-----------------|--------------------------|
| Practitioner-S0, S1 | technical-architecture OR evaluation/testing | 4 directions |
| Critic-S0, S1 | economics/market OR organizational/adoption | 4 directions |
| Adjacent-S0, S1 | cross-domain analogies | 4 directions |

This guarantees at least 3 distinct theme categories across 12 directions, without relying
on the orchestrator to enforce diversity post-hoc.

## Key Design Decisions

1. **Probes access knowledge independently** — Each probe reads the ForesightModel via
   `temper.get()` and uses `temper.web_search()` for domain context, rather than relying
   on the orchestrator to write a shared state file. This decouples probe execution from
   orchestrator behavior.

2. **Orchestrator still handles synthesis** — Synthesis delegation is kept in the orchestrator's
   instructions because synthesis requires reading all observations and directions, which
   is naturally an orchestration task.

3. **Probe max_turns = 15** — Probes have simple tasks (read, search, create observations,
   create directions, done). 15 turns is generous. Orchestrator keeps max_turns = 100 for
   polling and synthesis delegation.

## Diff Summary
The entire `ORCHESTRATION_INSTRUCTIONS` const was replaced with a shorter version that
only handles waiting for probes and synthesizing. A new `PROBE_PROMPT_TEMPLATE` const and
`build_probe_prompt()` function were added. The `run()` function now creates 7 sessions
(6 probes + 1 orchestrator) instead of 1.
