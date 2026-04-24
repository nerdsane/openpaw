# ADR-0040: Remove `llm_caller` and Make Staged Turn WASMs Authoritative

**Status:** Accepted
**Date:** 2026-04-24
**Supersedes:** ADR-0034
**Related:** ADR-0005, ADR-0020, ADR-0022, ADR-0025, ADR-0032, ADR-0034

## Context

ADR-0034 identified the right Session-turn shape:

- `PreparingContext`
- `CallingProvider`
- `ApplyingProviderResponse`

But the implementation stopped halfway. The Session spec had staged integrations, while the real logic still lived in one giant `llm_caller` crate and the staged WASMs were thin wrappers back into it.

That left OpenPaw with two problems at once:

- the legacy `call_llm` / `llm_caller` turn path still existed beside the staged flow
- the staged WASMs were not real hot-loadable owners of their behavior

This kept dead code, duplicate turn logic, and architectural ambiguity alive in the most important loop in the app.

## Decision

OpenPaw removes the legacy `call_llm` / `llm_caller` Session-turn path entirely.

The authoritative Session-turn flow is now:

`PreparingContext -> CallingProvider -> ApplyingProviderResponse -> Executing / Steering / Completed`

The authoritative WASM owners are:

- `context_preparer` for context loading, repair, pruning, prompt assembly, prompt caching, and prepared-context artifact creation
- `provider_caller` for provider/model resolution, request shaping, outbound HTTP, retries, and provider-response artifact creation
- `provider_response_applier` for assistant-response persistence, session-tree append, large-content externalization, and next-action routing
- `monty_repl` for tool execution
- `context_compactor` for compaction
- `steering_checker` for steering follow-up

The `Session` entity remains the sole orchestrator. No imperative orchestration layer is introduced outside Temper primitives.

## Hot-Loadability Rule

Major turn behavior must live in standalone WASM modules that map cleanly to Session state transitions.

Small shared Rust libraries are still allowed, but only for low-risk reusable code such as:

- tool catalog constants and alias normalization
- artifact structs and JSON builders
- low-level TemperFS/runtime helpers

Large hidden shared crates must not become a second monolithic turn engine behind the staged WASMs.

## Consequences

### Positive

- there is one Session-turn engine instead of two
- stage ownership now matches spec states
- the hot-loadable deployment boundary matches the operational boundary
- dead code in `call_llm` and the `llm_caller` module goes away
- duplicate prompt/executor tool catalog logic can be reduced around a shared source of truth

### Tradeoff

- some helper logic remains duplicated across stage WASMs until further cleanup lands

This is acceptable because the first priority is to make the staged WASMs the real deployable owners of behavior and remove the legacy turn path completely.

## Implementation Notes

- remove `[[integration]] name = "call_llm"` from `Session`
- remove `llm_caller` from policy/build wiring
- delete the `os-apps/paw-agent/wasm/llm_caller` crate
- keep tiny shared crates small and boring
- treat any future turn-stage refactor that recentralizes behavior behind one internal crate as a regression against this ADR
