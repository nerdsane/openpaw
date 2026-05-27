# ADR-0062: Directed Evolution Datadog Required Evidence

- Status: Proposed
- Date: 2026-05-27
- Deciders: TemperPaw maintainers
- Related:
  - ADR-0057: Directed Evolution Datadog Evidence Contract
  - ADR-0058: Directed Evolution Codex Tool Profile

## Context

The Directed Evolution worker can run Codex with Datadog MCP tools when
`PAW_CODEX_ENABLE_DATADOG_MCP=1`, and it records brain outputs as
`EvidenceArtifact` entities. The existing prompts ask reviewer and
simulated-user brains to inspect Datadog "when available."

That wording is not strong enough for stages whose app-owned
`RequiredEvidenceJson` includes `datadog_evidence_scope`. Those stages should
not pass unless the Codex brain actually queries Datadog and returns an
inspectable Datadog evidence URL, or fails honestly because Datadog could not
be reached.

## Decision

TemperPaw will align Directed Evolution Codex prompts with the app-owned
Datadog evidence gate:

- Reviewer and simulated-user prompts must treat `datadog_evidence_scope` as a
  mandatory evidence requirement when it appears in `RequiredEvidence`.
- The prompt must tell Codex to use authenticated Datadog MCP tools for logs,
  traces, or metrics evidence tied to the variant tenant/app.
- If Datadog cannot be queried, the brain must return `passed: false` with a
  clear `failure_reason` instead of passing with local-only evidence.
- Worker evidence correlation continues to include stable Datadog join fields
  and the first Datadog URL returned by the brain remains the primary
  `EvidenceArtifact.Uri`.

## Consequences

- The worker's brain behavior matches the Directed Evolution state machine's
  stricter evidence semantics.
- A missing Datadog MCP configuration becomes an honest failed evaluation
  instead of a weak pass.
- No direct Datadog client is added to `paw-codex-worker`; Datadog reasoning
  remains agent-driven through Codex.

## Non-Goals

- This ADR does not require every role to query Datadog.
- This ADR does not remove runtime OData checks or simulated-user behavior.
- This ADR does not make Railway host Codex; the worker can still run locally.
