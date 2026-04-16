# ADR-0035: OTS Trajectory Emission from paw-agent Sessions

**Status:** Accepted
**Date:** 2026-04-16
**Related:** ADR-0005 (Temper-Native Orchestration), ADR-0015 (Convergence Analyst), ADR-0022 (LLM Calling Infrastructure Optimizations), ADR-0032 (TemperFS Agent Operations), ADR-0034 (Bounded Session Context and LLM Turn Decomposition)

## Context

Three trajectory layers exist in the OpenPaw + Temper stack today, each answering a different question:

1. **Temper action trajectories** — `record_dispatch_trajectory` writes one row per state-machine action dispatch to the Turso `trajectories` table. Captures action name, from→to status, success, error, agent_id, session_id. Answers: "what actions did the session machine execute?"
2. **OpenTelemetry spans** — `llm_caller` / `provider_caller` emit OTel spans with `gen_ai_parent_trace_id` / `gen_ai_parent_span_id`. Captures latency and call hierarchy. Answers: "how did this call perform?"
3. **OTS (Open Trajectory Specification)** — `temper-ots` crate defines the data model (`OTSTrajectory`, `OTSTurn`, `OTSDecision`). The Turso `ots_trajectories` table and `POST`/`GET /api/ots/trajectories` endpoints exist. Answers: "what did the agent decide, and why?"

Layer 3 has zero production writers today. The schema, endpoint, storage, and query surface are all in place and unused. paw-foresight's Convergence Analyst (ADR-0015) can only compare probe Observations — not the tool-call paths that produced them — which caps its ability to distinguish independent convergence from copy-paste convergence across probes. Rita's foresight Run 000-010 baseline has identified this as a core limitation of rubric v3 scoring.

This ADR is about becoming the first producer for Layer 3.

## Decision

### 1. Batch-on-completion emission from a new WASM module

A new WASM module `emit_ots_trajectory` fires when the Session entity enters `Completed`, `Failed`, or `Cancelled`. It reads the session's JSONL turn tree and per-session tool-span JSONL, assembles an `OTSTrajectory` document, and POSTs it to `{temper_api_url}/api/ots/trajectories` with `X-Agent-Id`, `X-Session-Id`, `X-Tenant-Id`, `X-Trajectory-Id` headers.

The module is declared as a standalone integration alongside `deliver_reply` on the same terminal-state effects — reply delivery and trajectory emission run independently. Trajectory emission failure cannot poison reply delivery.

Per-turn incremental OTS writes were considered and rejected: the endpoint expects full trajectory documents, per-turn incremental emission would require either a new endpoint (duplicates platform concern) or many-small-writes pattern that breaks idempotency on retry.

### 2. Tool spans persisted as TemperFS JSONL, not inline entity fields

Today `monty_repl` collects `tool_span_events: Vec<Value>` per turn and passes it as the `_dd_llmobs_tool_spans` callback param. The Vec is discarded between turns — no downstream consumer reads the param. Nothing on the Session entity preserves a session-level tool-call history.

To give the emitter something to convert, a new Session state variable `tool_spans_file_id` (string, initial `""`) references a single TemperFS JSONL file per session. `monty_repl` appends one line per tool call per turn. The file grows monotonically and is read once at terminal-state emission.

Alternatives rejected:
- **Inline `tool_spans` field on the Session entity.** Breaks the 32KB WASM visibility ceiling after roughly 15 turns of large tool outputs.
- **Separate `ToolSpan` entity per call with session_id FK.** Requires a new IOA spec, Cedar policies, list queries on Completed, and N additional round-trips for write-once data. Pure overhead.
- **Keep the callback-param-only path and pass the full Vec into the emitter on terminal state.** Vec reconstruction across turns requires some persistence anyway, so this is actually option 1 in disguise.

The TemperFS file mirrors the existing `session_file_id` (conversation-tree JSONL) pattern and reuses the `max_sync_file_bytes` externalization already present.

Per the no-band-aids rule, the `_dd_llmobs_tool_spans` callback param is removed in the same change once `rg` confirms no consumer across the openpaw and temper repos.

### 3. Server-side converter in Temper was rejected

An alternative design would have Temper server auto-convert Session state into OTS on Completed, without a WASM emitter. Rejected because:
- Tool spans would still need cross-turn persistence (see section 2); the converter would just move the emission point, not eliminate the persistence work.
- It bleeds openpaw-specific data shapes into platform code.
- The governed HTTP boundary Cedar relies on disappears.
- Failures are harder to observe — no HTTP trace, no state-machine-visible status field.

OTS emission stays an app-level concern, fully Temper-native through a WASM integration.

### 4. Decision atomicity: one `OTSDecision` per tool call

The `OTSDecision` struct (`temper/crates/temper-ots/src/models/decision.rs:345-442`) supports per-decision-point granularity with four `DecisionType` variants (`ToolSelection`, `ParameterChoice`, `ReasoningStep`, `ResponseFormulation`). The emitter produces exactly one decision per tool call invocation — `decision_type = ToolSelection`:

- `decision_id` = `tool_call_id` (already unique per call)
- `choice.action` = tool name
- `choice.arguments` = arguments parsed as JSON
- `choice.rationale` = the `thinking` block text immediately preceding the call in the session-tree Message entry
- `consequence.success` = `!is_error`
- `consequence.result_summary` = first 500 chars of tool result
- `consequence.error_type` = classification when `is_error`: `"tool_timeout"`, `"tool_error"`, `"cedar_denied"`, `"unknown_error"`

Reasoning steps do not become separate `ReasoningStep` decisions — they are already captured as `OTSMessage.reasoning` on the turn-level message. Parameter choice is rolled into `choice.arguments`. Response formulation is captured as the terminal assistant `OTSMessage`.

### 5. Counter-reasoning, evaluation, and credit assignment are deferred

The OTS schema includes optional fields for `OTSDecision.alternatives` (rejected options), `OTSDecisionEvaluation.counterfactual` (post-hoc "what would've been better"), `OTSDecisionEvaluation` (score + feedback), and `OTSCreditAssignment` (contribution-to-outcome). All serialize out of JSON when `None`.

Standard Anthropic / OpenRouter tool-use responses do not emit alternatives — the model returns only its chosen action. Populating `alternatives` requires changing the agent system prompt to explicitly instruct "for each tool call, list two rejected alternatives and why" — a prompt-engineering change with its own A/B evaluation cost and latency impact. Evaluation and credit assignment require a separate evaluator agent running over completed trajectories.

All four optional fields remain `None` in the initial emission. Populating them is future work tracked as separate ADRs.

### 6. Outcome derivation from terminal state

- `Completed ∧ has_result → Success`
- `Completed ∧ ¬has_result → PartialSuccess` (defensive; shouldn't occur given the `CompletedHasResult` session invariant)
- `Failed → Failure`
- `Cancelled → PartialSuccess` — user-interrupted sessions with partial work done are more useful signal than noise for evaluation agents

Serialization is snake_case per `temper-ots/src/models/enums.rs:22-29` (the enum variant is `PartialSuccess`, serialized as `"partial_success"`).

### 7. Retry policy: one automatic retry, state-machine-observable

Emission failures surface as a state change on the Session entity via three new fields:

- `trajectory_id` (string) — generated once before first POST, reused on retry for idempotency (`INSERT OR REPLACE` on the Turso side is keyed on this)
- `trajectory_emission_status` (string, initial `"pending"`) — transitions to `"emitted"` or `"failed"`
- `trajectory_emission_error` (string) — last error message

Three new self-loop actions from `Completed | Failed | Cancelled`:
- `MarkTrajectoryEmitted(trajectory_id)` — success path
- `TrajectoryEmissionFailed(error)` — failure path, also fired via the integration's `on_failure` hook
- `RetryTrajectoryEmission` — guarded by `trajectory_emission_status == "failed" AND retry_count < 1`

Retry is one-shot and state-machine-visible, not in-WASM retry loops. Beyond one retry, the Evolution Engine can sweep `trajectory_emission_status = "failed"` rows as an I-Record in a future track.

### 8. paw-foresight consumer via existing MCP surface

The Convergence Analyst session already has access to the `temper_get_trajectories` MCP tool. `handle_probe_done` assembles a `temper.get_trajectories(...)` fetch-instruction block per probe_agent_id and injects it into the analyst's `user_message`. Fetch-on-demand keeps the probe→analyst data path within the governed Temper API surface rather than inline splicing large JSON into the 32KB-ceiling callback param.

## Consequences

### Positive

- paw-foresight's Convergence Analyst can distinguish independent convergence from copy-paste convergence by inspecting per-probe decision traces.
- Session-level tool-call history becomes retrievable via `temper.search_history` and the OTS query API — answers questions that neither Layer 1 action trajectories nor Layer 2 OTel spans can answer alone.
- The emitter is a clean additive change: no modification to `llm_caller`, `provider_caller`, or the new `context_preparer` landed in ADR-0034.
- Retry idempotency is free because `INSERT OR REPLACE` is keyed on `trajectory_id`.
- Trajectory emission failure cannot wedge a successful session — separate integrations, separate failure domains.

### Negative

- One extra TemperFS file per session (`tool_spans_file_id`). Bounded by session turn count.
- One extra HTTP call per terminal session transition (plus potential retry).
- OTS trajectories shipped by this track don't carry counter-reasoning yet. That limits the analytical depth available to evaluation agents in the short term, but the schema remains forward-compatible when the prompt-engineering work lands.

### Neutral

- A new decision point lands on the Session entity: on terminal state, two integrations fire (`deliver_reply`, `emit_ots_trajectory`). The state machine explicitly models both.

## Verification

Per the repository's mandatory red-green TDD and end-to-end proof requirements:

- **Unit tests** — `emit_ots_trajectory` fixture test: canned Session JSONL → hand-built OTS JSON → deserializes as `OTSTrajectory` via a sibling test crate that imports `temper-ots`. Property test covers all `(status, has_result)` combinations for outcome derivation.
- **WASM dispatch harness** — `temper/crates/temper-server/tests/wasm_dispatch.rs` extended with `test_emit_ots_trajectory`: install paw-agent spec + module, create Session with fake file-ids, dispatch `FinalizeResult`, assert POST fired and `MarkTrajectoryEmitted` dispatched.
- **Local E2E** — start temper-server + paw-agent, drive one Session Created → Completed via the ODataClient pattern from `scripts/prove_cron_scheduling.py`, confirm `tool_spans_file_id` populated, `trajectory_emission_status == "emitted"`, GET `/api/ots/trajectories` returns the trajectory.
- **Turso row dump** — confirm the `data` column parses as a valid OTSTrajectory JSON.
- **Proof reports** in `.proofs/` per phase, following `.proofs/TEMPLATE.md`.

The foresight meta-loop behavioural rerun (Run 011) is explicitly deferred — that proof happens on main after merge, in a separate foresight run tracked separately from this ADR.

## Rejected Alternatives

### 1. Server-side converter in Temper

See Decision section 3.

### 2. Inline tool spans on Session entity

See Decision section 2. 32KB WASM visibility ceiling.

### 3. Separate `ToolSpan` entity per call

See Decision section 2. Too many round-trips for write-once data.

### 4. Per-turn incremental OTS writes (Approach B from openpaw#61)

See Decision section 1. The endpoint expects full trajectory documents.

### 5. Include counter-reasoning in the initial emission

See Decision section 5. Requires LLM prompt change; separate track.

### 6. Extend Temper's OTS schema with openpaw-specific fields

Rejected. The OTS schema is a shared platform contract (`temper-ots` crate). openpaw-specific metadata, if any, can live in `metadata.tags` without schema changes.
