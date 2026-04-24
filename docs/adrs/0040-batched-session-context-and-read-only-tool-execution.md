# ADR-0040: Batched Session Context and Read-Only Tool Execution

- Status: Proposed
- Date: 2026-04-23
- Deciders: OpenPaw maintainers
- Related:
  - ADR-0034: bounded session context and LLM turn decomposition
  - ADR-0037: end-to-end tracing and traceparent propagation
  - ADR-0038: queue depth vs steady-state concurrency
  - `os-apps/paw-agent/wasm/llm_caller/src/lib.rs`
  - `os-apps/paw-agent/wasm/monty_repl/src/dispatch.rs`
  - `os-apps/paw-agent/wasm/monty_repl/src/lib.rs`

## Context

The April 23 quality-review session spent more wall-clock time between model turns than inside the model itself. Two structural causes stood out:

1. `monty_repl` executed safe read-only tool snippets one-by-one, even when a model turn emitted several independent web reads that could run concurrently.
2. `context_preparer` rebuilt session context through many serialized `File/$value` reads while expanding session-tree context and loading skill files.

This was not a prompt-quality issue or a timeout issue. The runtime was doing too many sequential reads in hot paths that are naturally parallel.

ADR-0034 already committed OpenPaw to bounded, externalized session context. This ADR carries that forward by making the read side of the session loop explicitly batch-aware.

## Decision

OpenPaw treats independent read-only session work as a batchable primitive in two places: tool execution and context preparation.

### 1. `monty_repl` batches safe read-only snippets

Contiguous safe snippets inside the same checkpoint window may be planned and executed as one host batch instead of one HTTP call at a time.

The first batchable surface includes:

- `temper.web_search(...)`
- `temper.web_fetch(...)`
- read-only catalog/spec helpers such as `temper.show_spec(...)`, `temper.specs()`, `temper.get_insights()`, `temper.get_decisions()`, `temper.list_policies()`, and `temper.list_apps()`

General `temper.get(...)` / `temper.list(...)` remain outside the first batchable set because Cedar approval boundaries may make later calls unsafe to speculatively run.

**Why this approach**: it captures the high-volume, high-latency research calls from quality-review and synthesis sessions without weakening approval semantics.

### 2. `context_preparer` batches externalized file reads

When session-tree context or skill discovery requires multiple TemperFS `File/$value` reads, `llm_caller` now issues them through the host batch HTTP primitive and falls back to the existing single-read path for any request that does not succeed cleanly.

This batching is used for:

- session-tree context content files
- skill file body reads during skill advertisement

Compaction summaries remain forgiving: if an externalized summary file cannot be read, the inline summary is still used. Message/steering content remains strict: failed content-file reads still fail the turn rather than silently changing the conversation.

**Why this approach**: the session loop needed lower latency, not weaker correctness. Batched optimistic reads with per-file fallback preserve the existing semantics while removing avoidable serialization.

### 3. Exact-match WebQuery results are reused before new work is spawned

Before creating new research work, `monty_repl` first reuses completed `WebQuery` entities that already match the exact request.

**Why this approach**: the best batch is the one we do not execute. Reuse avoids repeated research turns and reduces both latency and cost.

## Readiness Gates

- `monty_repl` unit tests cover batch planning and checkpoint boundaries.
- `llm_caller` unit tests cover batched context-ref selection and the differing fallback behavior for compaction vs message entries.
- Fresh WASM builds succeed for `context_preparer`, `provider_caller`, `provider_response_applier`, and `monty_repl`.
- A live local Session E2E completes on the patched server with built WASM artifacts.

## Consequences

### Positive

- Read-heavy turns spend less wall-clock time waiting on serialized tool/file I/O.
- Context preparation scales better with externalized session trees and skill-heavy prompts.
- The batching boundary is explicit and reviewable instead of being hidden in prompt behavior.

### Negative

- The host/runtime contract grows: OpenPaw now depends on the Temper batch HTTP primitive.
- Batch planning logic is stricter and more complex than naive sequential execution.

### Risks

- Over-expanding the batchable surface could cross approval or sequencing boundaries. Mitigation: keep the first tranche read-only and explicitly exclude general OData reads that may require Cedar gating.
- Batched context reads could hide per-file failures. Mitigation: each file still falls back to the existing single-read path, and message-file failures still propagate as errors.

## Non-Goals

- Batching arbitrary mutating tool calls.
- Replacing checkpoint boundaries or turn decomposition.
- Eliminating context compaction; ADR-0034 still governs the long-context strategy.

## Alternatives Considered

1. **Keep sequential execution and rely on faster models** — Rejected. The investigation showed tool and file I/O dominated the wall-clock time.
2. **Batch every `temper.get` / `temper.list` call** — Rejected. That risks running past Cedar approval boundaries.
3. **Rewrite context preparation around a new server-side materialization API first** — Deferred. Valuable follow-up, but the host batch primitive fixes the current hot path with less platform churn.

## Rollback Policy

Disable the batch planners in `monty_repl` and `llm_caller`, returning both paths to sequential reads. Exact-match `WebQuery` reuse can remain independently if only batching needs to be reverted.
