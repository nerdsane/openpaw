# ADR-0035: OTS Trajectory Emission from paw-agent Sessions

**Status:** Accepted
**Date:** 2026-04-16
**Amended:** 2026-08-11 — sections 9-13 (ARN-109: real turns, decisions, and content)
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

### 9. Turns come from the SessionEntry tree (amends section 1, 2026-08-11)

Section 1 shipped one synthetic turn per session and deferred real turn
boundaries. In production that produced trajectories with a single turn, no
messages, and an empty `decisions` array — a row that no evaluation agent and no
RL consumer can use. The deferral is now closed.

The emitter reads the session transcript (the `session_file_id` reference, which
resolves either to a TemperFS JSONL file or to the SessionEntry rows) and walks
the chain from the recorded `session_leaf_id` to the root. Each assistant entry
closes one LLM cycle, so it opens a turn; the user, tool-result, steering, and
compaction entries that precede it are that turn's prompt side. The final turn
may have no assistant entry — that is a session interrupted mid-cycle, and it is
kept rather than discarded.

When the leaf is missing or its parent chain is broken (continuation and
recovery races can push the Session field ahead of durable rows), the emitter
falls back to the newest walkable entry, then to raw file order. A damaged tree
degrades the trajectory; it does not empty it.

`turn_count` is not the turn source — it is recorded as `_session_turn_count` so
a consumer can see when the reconstructed count disagrees with the counter the
state machine kept.

### 10. Decisions are reconstructed from the transcript, with spans as enrichment (amends section 4)

Section 2 made the tool-span JSONL the sole input to decisions, and section 4
mapped one span to one decision. That made a single config flag
(`persist_tool_spans_file`, shipped as `"false"`) sufficient to empty every
stored trajectory, which is what happened. Decisions now have two independent
sources:

- **The transcript.** `tool_use` blocks on the assistant entry give the tool
  name and arguments; the `tool_result` blocks that land on the next turn give
  success and result text. Both are already persisted for the model's own
  benefit, so this path costs nothing extra and cannot be switched off.
- **The spans.** They supply wall-clock duration, and they are the only evidence
  left when a message body was externalized to TemperFS. Spans nothing claims
  still become decisions rather than being dropped.

`cause_id` is set to the `tool_call_id` on every decision. It is the join
between a decision and the observation it caused — the `tool_result` block
carrying the same id, which by construction sits on the following turn.

Span persistence is enabled in the spec and defaults to ON in the guest: a
missing config key must not silently cost the training data. The cost this
guards against is real (the span document is rewritten in full on every tool
batch), so span records are compacted before persistence — results capped at 600
characters, arguments at 2000 — and the document is capped at 256KB with an
explicit truncation marker.

### 11. Message content is referenced, not inlined

Inlining message bodies once cost roughly 300MB of a 491MB database
(`.proofs/061`). Trajectories are stored as opaque blobs and are written once
per session, so the same failure is available here.

Bodies that already live in TemperFS are emitted as file references
(`content_file_id`, `content_file_version_id`) and never fetched. Inline text is
bounded twice: 4000 characters per message and 64000 characters per trajectory,
with the dropped character count recorded so a consumer can tell truncation from
absence. Tool arguments over 4000 serialized characters collapse to a preview
plus the original size. The session's own artifacts (session tree, tool spans,
prepared context, provider response, system prompt) are listed as OTS context
resources, which gives consumers the pointers without the payloads.

### 12. Spec identity and harness

`metadata.harness` is `"temperpaw"` — the runtime that produced the run, which a
cross-harness training set has to distinguish.

`metadata.spec_version` identifies the actor spec the run executed under. The
WASM guest context exposes only config, trigger params, entity state, and ids
(`temper-wasm-sdk::Context`); it carries no spec hash, and asking the server for
one would add an HTTP round trip to every terminal transition. So the identity
is declared in the spec's own trigger config as `<app>@<version>` and travels
with the spec that declares it. A repo contract test pins that literal to
`os-apps/paw-agent/app.toml`, so the two cannot drift apart silently.

Alternatives rejected: an extra request to read the installed-app version (a
round trip per session for a value the spec already knows), and a hand-written
hash literal (drifts the moment someone forgets to update it).

### 13. Token counts always, token ids only when the serving stack sends them

Per-turn prompt and completion token counts come from the provider response and
are recorded on the assistant entry when it is written, so they are exact rather
than reconstructed. They surface as `_prompt_tokens` and `_completion_tokens`;
session totals surface as `_token_usage`. The OTS schema has no field for token
counts, and the underscore prefix marks non-standard fields the same way
`_duration_ms` already does on decisions.

`prompt_token_ids`, `completion_token_ids`, `response_mask`, and `logprobs` are
emitted with exactly those names when the pipeline recorded them, and are absent
otherwise. RL consumers need token ids because retokenizing text drifts, but no
provider is asked for them: the OpenAI-compatible and Responses stream parsers
capture them if the server streams them (flattening OpenAI's
`logprobs.content[].logprob` shape to the flat array the contract requires), the
Anthropic Messages stream carries none, and nothing issues a second request.
Malformed signals are dropped rather than passed through — a fabricated mask is
worse than a missing one.

Per-turn timestamps have the same shape of problem. The entity event log is a
hot tail that drops older events at snapshot boundaries, so it cannot date a
long session's turns. Every SessionEntry is therefore stamped with its own
`ts_ms` at creation, and the event log is only a fallback for entries written
before that stamp existed.

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

### Verification of the 2026-08-11 amendment (ARN-109)

- **Round trip against the kernel structs** — `emit_ots_trajectory` takes
  `temper-ots` as a host-only dev-dependency and deserializes its own output
  into `OTSTrajectory`, asserting the reconstructed turns, message roles,
  content types, decision types, and durations. A field-name or type drift on
  either side fails the build instead of storing an unreadable row. Terminal
  states other than success round-trip too.
- **Unit tests** — turn reconstruction from a two-cycle transcript, leaf
  fallback and parent-cycle guards, decision/observation pairing with
  `cause_id`, per-message and per-trajectory inline budgets, oversized tool
  arguments, externalized-body references, token-signal validation, timestamp
  derivation, and the RFC-3339 conversion.
- **Repo contract tests** — `crates/temperpaw/tests/ots_trajectory_contract.rs`
  pins span persistence to on, `spec_version` to `app.toml`, the OTS field names
  the kernel deserializes, the inline budget, trajectory-id idempotency, and the
  requirement that every terminal action still emits.
- **Bounded-write tests** — `monty_repl` span compaction and the span-file size
  ceiling, including that a truncated document still parses line by line.
- **Guest build** — every touched module rebuilt for `wasm32-unknown-unknown`
  (`monty_repl` for `wasm32-wasip1`); the `temper-ots` dev-dependency is never
  part of a guest build.

The live local end-to-end run (`scripts/prove_track3_ots.py` against a local
temper-server with the paw-agent app installed and real provider credentials)
gates the deploy and is recorded on the pull request, not here.

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

### 7. Fetch externalized message bodies at emission time (2026-08-11)

Rejected. A session can externalize many entries, so this is N TemperFS reads on
a terminal transition, and it puts the full bodies back into the stored blob —
the exact failure `.proofs/061` records. The emitter references the files
instead; a consumer that wants a body can read it through the governed API.

### 8. Read the installed-app version for `spec_version` (2026-08-11)

Rejected. An HTTP round trip per terminal session to learn a value the spec
already knows. See decision section 12.

### 9. Plumb a token-id request flag into provider calls (2026-08-11)

Rejected for this track. Asking providers for logprobs or token ids changes the
request, costs latency and money on every turn, and most providers in this stack
cannot return them at all. The emitter carries the fields when the serving stack
volunteers them and leaves them absent otherwise; turning them on deliberately
for a training run is a separate decision with its own cost analysis.
