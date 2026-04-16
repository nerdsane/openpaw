# ADR-0034: Bounded Session Context and LLM Turn Decomposition

**Status:** Proposed
**Date:** 2026-04-15
**Related:** ADR-0005 (Temper-Native Orchestration), ADR-0020 (File-Backed Storage for Document-Sized App Content), ADR-0022 (LLM Calling Infrastructure Optimizations), ADR-0025 (Session Recovery and Conversation Reset), ADR-0032 (TemperFS Agent Operations)

## Context

The current `llm_caller` WASM module has become the operational hotspot in the Paw session loop.

Today a single `call_llm` invocation is responsible for:

- reading the active conversation from the session tree or legacy conversation storage
- repairing interrupted tool-use blocks defensively
- pruning old tool results
- estimating context size and deciding whether compaction is needed
- assembling the full system prompt from soul, instructions, harness, skills, plan mode state, memory, and SDK reference
- translating the prompt and message stream into provider-specific wire formats
- resolving provider configuration and API keys
- performing the outbound LLM HTTP request
- parsing provider usage and stop reasons
- appending the assistant reply back into session storage
- deciding whether the next state is `Executing`, `Steering`, or `Completed`

This violates the architectural spirit of ADR-0005 even though it is technically Temper-native: the Session entity remains the orchestrator, but one WASM integration is performing too many distinct responsibilities inside one memory budget and one failure domain.

This concentration creates three concrete problems.

### 1. Memory safety is implicit rather than designed

Temper's WASM runtime enforces a per-invocation memory cap, with a global default of `64 MiB`. That cap is healthy, but the session loop currently depends on `llm_caller` staying under it while materializing multiple copies of large conversational data at once: raw JSONL session content, parsed message arrays, formatted prompt text, provider-specific request bodies, and large tool result payloads.

The problem is not that long conversations exist on disk. The problem is that one invocation tries to place too much of that history into live memory at the same time.

### 2. Token-aware compaction is not sufficient for byte-aware safety

The current session loop triggers compaction based on token estimates near the provider context window. That is necessary but not sufficient:

- prompt assembly cost is also a function of bytes, not just tokens
- large tool outputs and large pasted documents create heap pressure before token budgets are exhausted
- current compaction summaries preserve work trajectory better than exact document recall

This means a session can remain within an acceptable token window while still becoming fragile from a memory perspective.

### 3. Responsibilities are mixed across the wrong state boundaries

The Session entity already has separate integrations for compaction, steering, approval, recovery, and tool execution. LLM calling is the outlier: one module still combines context preparation, provider I/O, response routing, and persistence.

As a result:

- memory instrumentation is coarse
- failures are hard to localize
- provider concerns and session concerns are coupled
- long-term improvements require editing a 4K+ line WASM crate instead of evolving explicit Session transitions

## Decision

OpenPaw will keep the Session entity as the sole orchestrator for an agent turn, but will decompose the current `call_llm` step into explicit, bounded, Temper-native stages.

### 1. Keep Session as the orchestration boundary

We are not introducing an imperative orchestration layer outside Temper.

The Session entity remains responsible for turn progression. All turn work continues to happen through Session actions and WASM integrations. The redesign is a decomposition of one oversized integration into smaller state transitions, not a move away from Temper-native orchestration.

### 2. Replace the monolithic `call_llm` turn with explicit turn phases

The session loop will be refactored into the following states and integrations:

- `PreparingContext`
  Triggered from `Thinking`. Runs `context_preparer`.
- `CallingProvider`
  Triggered when bounded context is ready. Runs `provider_caller`.
- `ApplyingProviderResponse`
  Triggered after provider completion. Runs `provider_response_applier`.

The existing states remain:

- `Compacting`
- `Executing`
- `Steering`
- `WaitingForApproval`
- `Recovering`

The target turn flow becomes:

`Thinking` -> `PreparingContext` -> `CallingProvider` -> `ApplyingProviderResponse`

From `ApplyingProviderResponse`, the session transitions to:

- `Executing` when tool calls are present
- `Steering` when the provider completed an assistant turn with no tool calls
- `Failed` on unrecoverable parse or persistence errors

This preserves the existing high-level Session lifecycle while making the expensive steps explicit and measurable.

### 3. Introduce a bounded prepared-context artifact

`context_preparer` will read the session tree and assemble a bounded context package.

That package will be written to TemperFS and referenced from Session state using new fields such as:

- `prepared_context_file_id`
- `prepared_context_tokens`
- `prepared_context_bytes`
- `prepared_context_entry_count`
- `prepared_context_content_file_count`

This package is not a new orchestration entity. It is a file-backed artifact referenced by Session state, using existing TemperFS primitives and the Session entity as the authoritative state machine.

The prepared context package is the contract between context assembly and provider calling. `provider_caller` is not allowed to walk the session tree directly.

### 4. Make byte budgets first-class, not implicit

`context_preparer` must enforce both token and byte budgets before the provider call is attempted.

It will:

- compute token estimates for the bounded working context
- compute serialized byte size for the prepared context artifact
- externalize oversized inline content into file-backed references where possible
- refuse to proceed if the prepared context cannot fit within the configured turn budget
- dispatch `NeedsCompaction` when compaction is required to create a bounded context

This turns memory safety from an emergent side effect into an explicit turn contract.

### 5. Keep compaction, but strengthen its semantics

`context_compactor` remains the module responsible for summarizing older work and inserting compaction entries into the session tree.

It will be enhanced to support:

- byte-aware cut-point selection in addition to token-aware cut-point selection
- explicit accounting for compaction summary token estimates
- content-mode-aware behavior so document-like content is not treated as ordinary work trajectory text

Compaction remains a way to keep the active prompt bounded. It is not a replacement for exact recall. Exact historical content must continue to be recoverable from the session tree through file-backed entries and `temper.search_history`.

### 6. Externalize large content earlier in the lifecycle

Oversized content must become file-backed before it reaches provider-call assembly whenever possible.

This applies to:

- large user-pasted documents
- oversized tool results
- oversized assistant content blocks

The rule is:

- the session tree remains the durable archive
- the prepared context remains a bounded working set
- exact older content is recovered through retrieval, not by keeping everything inline forever

### 7. Narrow the provider module to provider concerns only

`provider_caller` will own:

- provider selection
- provider-specific request translation
- outbound HTTP
- retry policy
- response usage extraction
- raw provider response capture

It will not own:

- session-tree traversal
- system prompt assembly from many platform files
- compaction decisions
- response persistence into session history
- tool/steering/completion routing

This keeps provider integration logic isolated so future provider work does not require editing Session context assembly logic.

### 8. Add first-class observability for session context health

This redesign must ship with new metrics and dashboard coverage. Metrics are part of the design, not follow-up cleanup.

The initial metric set is:

- `temper_session_context_tokens`
- `temper_session_context_bytes`
- `temper_session_context_entries_loaded`
- `temper_session_context_content_files_loaded`
- `temper_session_context_prepare_duration_ms`
- `temper_session_context_externalized_content_total`
- `temper_session_compaction_trigger_total`
- `temper_session_compaction_bytes_replaced`
- `temper_session_provider_request_bytes`
- `temper_session_provider_response_bytes`
- `temper_session_provider_call_duration_ms`
- `temper_session_memory_limit_exceeded_total`

Recommended tags:

- `module`
- `provider`
- `session_mode`
- `result`
- `content_kind`

High-cardinality identifiers such as `session_id` and `agent_id` must not be attached to these metrics.

The OpenPaw Datadog dashboard in `dd-dashboards/openpaw-overview.json` must gain panels for:

- active context size over time
- compaction trigger rate
- provider request and response body sizes
- context preparation duration
- memory-limit failures

At least one monitor must be added for non-zero memory-limit failures.

### 9. Ship with an operational guardrail while the decomposition lands

The long-term fix is architectural decomposition plus bounded context contracts. However, the current system needs an immediate production guardrail while that work lands.

Therefore the first implementation phase will also set an explicit `max_memory` for the current `llm_caller` integration. This guardrail is not the architectural fix; it is a stability patch that buys room for the proper redesign.

## Consequences

### Positive

- memory budgeting becomes explicit at the Session boundary
- provider-specific failures become easier to isolate from context-preparation failures
- long conversations become bounded working contexts rather than unbounded live prompt assembly
- document preservation and exact recall improve because retrieval is treated as a first-class path
- future provider work becomes lower-risk because provider logic is isolated
- the Datadog dashboard can show leading indicators of context health before users experience failures

### Negative

- the Session state machine becomes more explicit and therefore more complex
- more artifacts are written to TemperFS per turn
- rollout requires coordinated changes across Session spec, multiple WASM crates, metrics, and dashboard queries
- temporary duplication may exist while the old `llm_caller` is narrowed incrementally rather than replaced in one cut

## Rollout Plan

### Phase 0: Stabilize and instrument

- set an explicit `max_memory` on the current `llm_caller` integration
- add context and provider payload metrics to the current path
- add dashboard panels and a monitor for memory-limit failures
- add a reproducible proof case with a large pasted document and tool-heavy session

### Phase 1: Extract context preparation

- add `PreparingContext` state and `context_preparer` integration
- move session-tree reads, pruning, byte accounting, and compaction decisions out of `llm_caller`
- persist prepared context to a TemperFS artifact referenced from Session state

### Phase 2: Narrow provider calling

- add `CallingProvider` state and `provider_caller` integration
- limit provider work to translation, HTTP, retry, and response capture
- emit provider request and response size metrics from this module

### Phase 3: Apply provider responses explicitly

- add `ApplyingProviderResponse` state and `provider_response_applier`
- move session persistence and transition routing out of the provider module
- ensure assistant content externalization occurs here when needed

### Phase 4: Improve compaction semantics

- add byte-aware cut-pointing
- fix compaction token accounting
- add content-mode-aware handling for document-like content

## Verification

This work must follow the repository's mandatory red-green TDD and end-to-end proof requirements.

At minimum, the implementation must include:

- unit tests for byte budgeting and externalization decisions
- state-machine tests for the new Session transitions
- provider-caller tests for request/response size instrumentation
- end-to-end proof of a long conversation with large pasted content
- end-to-end proof that exact older content remains recoverable through `temper.search_history`
- end-to-end proof that new metrics appear in the Datadog dashboard data source and that dashboard JSON includes the new panels

## Rejected Alternatives

### 1. Only raise `llm_caller` memory

Rejected because it masks the design problem without making turn memory bounded by construction.

### 2. Split `llm_caller` into helper functions inside the same WASM crate only

Rejected because it improves code organization but does not create real Session-level boundaries, separate observability, or distinct failure domains.

### 3. Keep exact long-term context inline and never retrieve older content on demand

Rejected because it scales poorly in both memory and token cost, especially for pasted documents and tool-heavy sessions.

### 4. Introduce an imperative Rust orchestration loop outside the Session entity

Rejected by ADR-0005. The redesign must remain fully Temper-native.
