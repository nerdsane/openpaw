# ADR-0057: Directed Evolution Datadog Evidence Contract

- Status: Proposed
- Date: 2026-05-27
- Deciders: TemperPaw maintainers

## Context

Directed Evolution worker roles already record `EvidenceArtifact` entities for
Codex brain runs, but the evidence can be too generic. The artifact URI falls
back to a TemperPaw-local placeholder unless the agent happens to return a
single `evidence_uri`, and the role prompts do not require a structured
Datadog evidence scope.

That is not enough for the Directed Evolution product contract. Observer,
reviewer, and simulated-user brains must be able to explain what they learned
from production telemetry and live variant execution. Mission Control should
show evidence that can be inspected, not just summaries.

## Decision

Directed Evolution Codex worker prompts will require a structured
`evidence_scope` array for observer and evaluation roles. Each scope item
records:

- `surface` such as logs, traces, metrics, monitors, or runtime
- `query`
- `result_summary`
- `datadog_url` when the evidence came from Datadog

When an output includes a Datadog evidence URL, the worker will use that URL as
the primary `EvidenceArtifact.Uri`. If no Datadog URL is present, the existing
`evidence_uri`, `evidence_refs`, runtime ref, and local fallback behavior
remain in place.

Worker-created evidence correlation will also carry stable join fields:
`work_item_id`, `role`, `target_entity_type`, `target_entity_id`, service,
environment, and the Datadog query used to find matching worker telemetry.

## Consequences

- Directed Evolution evidence becomes Datadog-inspectable when a brain used
  Datadog, rather than only locally summarized.
- Mission Control can surface a real observability URL for a variant or brain
  result without inventing UI state.
- The contract stays backward-compatible with existing worker outputs.

## Non-Goals

- This ADR does not add a direct Datadog client to `paw-codex-worker`.
- This ADR does not require Railway to run Codex.
- This ADR does not replace the existing Datadog Patrol flow.
