# ADR-004: Directed Evolution Local Codex Worker

- Status: Proposed
- Date: 2026-05-26

## Context

Directed Evolution needs many brain instances: observer, direction framer,
variant generator, simulated user, reviewer, selector, and narrator. The human
facing director brain remains the live Codex chat session, but the background
roles must run as bounded jobs and write structured results back to Temper.

Deployed Genesis or Railway services should not run Codex directly in v1.
TemperPaw already manages Codex sessions and bridges them to Temper entities,
which makes it the right local execution layer. The worker must still honor
TemperPaw's entity-first architecture: orchestration state belongs in Temper
entities, while TemperPaw claims explicit work and self-reports completion.

## Decision

Add a Directed Evolution worker path to the managed-agent layer. The worker
claims `WorkItem` entities from the Directed Evolution control plane, starts a
bounded Codex session for the requested brain role, captures evidence and
observability metadata, and records results back through entity actions.

The worker does not own the Directed Evolution state machine. It only moves
state by dispatching explicit actions on `WorkItem`, `BrainRun`, `Variant`,
`StageResult`, `Trial`, `Direction`, `Episode`, or `Promotion` entities.

### Brain Roles

The worker supports role-specific prompts and output schemas for:

- `observer`
- `direction_framer`
- `variant_generator`
- `simulated_user`
- `reviewer`
- `selector`
- `narrator`

The same worker machinery can run each role, but each role has a bounded
contract: allowed context, expected output schema, evidence requirements,
timeout, and permitted tools.

### Simulated Users Are Agents

Simulated user work items launch real agent sessions. The harness may provide
accounts, seeded data, routing, and a goal, but the simulated user must decide
how to use the app. It must not be a script that deterministically performs the
winning behavior.

### Evidence And Observability

Every claimed work item records:

- `work_item_id`
- `brain_run_id`
- role
- parent session id, if any
- target entity ids
- app ref or variant ref
- Datadog trace/span ids when available
- artifact paths or evidence refs
- terminal status and failure reason

The worker emits the RFC-0001 correlation tags into local telemetry so local
Codex work and deployed app traces can be joined in Datadog.

### No Hidden Promotion Logic

The worker may submit selector outputs, but it does not decide promotion
outside the control plane. Promotion occurs only when the Directed Evolution
entities accept the selection and dispatch the promotion action.

## Consequences

- TemperPaw becomes the local execution plane for Directed Evolution without
  becoming a second orchestration engine.
- The same Codex-session machinery can run variant generation, simulated
  users, review, selection, and narration.
- Mission Control can link brain output to entity state and observability
  evidence.

## Verification

- Unit tests cover work-item claim/result serialization and role schema
  validation.
- Integration tests run a fake local control plane and prove claim -> Codex
  session stub -> result-posting transitions.
- An end-to-end proof runs at least one real background Codex job before merge.
- A simulated user proof shows an agent making decisions from a goal, not a
  deterministic script replay.
