# ADR-0043: Session Phase Latency Budgets and Response Application Contract

**Status:** Proposed
**Date:** 2026-04-24
**Related:** ADR-0005 (Temper-Native Orchestration), ADR-0022 (Lazy Sandbox Provisioning), ADR-0034 (Bounded Session Context and LLM Turn Decomposition), ADR-0037 (End-to-End Tracing), ADR-0039 (Orphaned Session Recovery), ADR-0041 (Session Hot Fields Stay Out of the Query Plane)

## Context

The 2026-04-24 dark academia `source_search` investigation showed that the long wall clock was not primarily model inference. For session `ss-019dc0bd-fdf2-70a2-adef-a164901ec9a0`, the provider HTTP calls totaled about 129 seconds, while the session ran for about 38 minutes.

The slow parts were platform/application phases around the provider call:

- `ProvisionWorkspace` spent minutes in a phase ADR-0022 describes as fast local storage setup.
- `ContextReady` / provider wrapper spans included artifact reads/writes and callback work that were much larger than provider HTTP time.
- `ApplyingProviderResponse` stalled after the provider returned a tool-use response and never reached `ProcessToolCalls`.
- Query projection updates could sit on the post-dispatch critical path and add multi-second latency to unrelated state transitions.

These are architectural boundary failures. The Session entity remains the right orchestrator, but every non-terminal phase needs a crisp contract: either it advances quickly, or it fails with the exact slow substep.

## Decision

Fresh Session turns adopt explicit phase latency budgets and phase-level telemetry.

Initial budgets:

- `workspace_provisioner`: target p95 <= 10s.
- `context_preparer`: local budget 120s, target p95 <= 20s.
- `provider_caller`: local budget 600s, with provider HTTP measured separately.
- `provider_response_applier`: local budget 30s, target p95 <= 5s.

The `ApplyingProviderResponse` phase is only allowed to:

1. read the prepared-context artifact,
2. read the provider-response artifact,
3. append one assistant entry to the session tree when a session tree exists,
4. dispatch the next Session action.

It must not rebuild or serialize the full historical conversation for fresh session-tree turns. Legacy inline/conversation-file sessions keep the existing conversation payload fallback until they age out.

Temper query-plane projection maintenance is no longer part of the action-dispatch success path. The entity transition is already durable once the action succeeds; projection maintenance runs in the background with explicit lag/error metrics and alerts.

## Implementation

OpenPaw:

- Emit `temper_session_phase_duration_ms` and `temper_session_phase_step_duration_ms` from workspace, context, provider, and provider-response phases.
- Emit `temper_session_phase_budget_exceeded_total` before the state timeout when a local phase budget is exceeded.
- Make `provider_response_applier` skip legacy conversation payload construction when `PreparedContextArtifact.use_session_tree` is true.
- Keep `conversation` params only for legacy sessions without a session tree.

Temper:

- Enqueue live query-projection updates after dispatch instead of awaiting them inline.
- Emit `temper_query_projection_update_enqueued_total`, `temper_query_projection_update_duration_ms`, and `temper_query_projection_update_error_total`.
- Wrap background projection work in a `dispatch.phase.query_projection` span.
- Treat guest-emitted metric kind `count` as a counter, matching the convention already used by OpenPaw WASM modules.

## Consequences

Positive:

- A provider response that has already returned cannot spend minutes applying without tagged substep evidence.
- Fresh session-tree turns avoid large duplicate conversation serialization in the response-apply phase.
- Query projection slowness no longer delays the user-visible state transition.
- Operators can distinguish provider HTTP time, artifact I/O, response application, and projection lag directly in Datadog.

Negative:

- Query projection reads become eventually consistent after live dispatch. Existing startup/backfill correctness remains unchanged.
- A failed background projection update no longer fails the originating action; monitors become the safety rail.
- The staged WASM crates still duplicate turn code. This ADR does not solve that structural duplication.

## Verification

- Unit tests cover session-tree response application avoiding legacy conversation payloads while legacy inline mode still emits them.
- Temper dispatch tests cover the background projection policy, and query projection tests poll for eventual projection updates.
- Dashboard and monitor definitions include session phase duration, session phase budget failures, background query projection duration, and projection update errors.

## Rollback

Revert the OpenPaw and Temper PRs together. If only the Temper change must roll back, projection updates return to inline dispatch latency while OpenPaw phase telemetry remains valid. If only OpenPaw rolls back, Temper projection safety metrics remain useful but `ApplyingProviderResponse` can again build large legacy payloads for fresh session-tree turns.
