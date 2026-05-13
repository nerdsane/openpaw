# ADR-003: Session Event Observability Fields

- Status: Accepted
- Date: 2026-05-11

## Context

Managed agent sessions bridge a `ManagedSession` to an inner
`TemperPaw.Session`. Datadog traces should eventually show one coherent
`temperpaw.agent.session` tree, but platform span-parenting is not enough by
itself for humans and agents who debug from entity state. The managed
`SessionEvent` timeline also needs stable fields that can be queried and joined
without parsing prose or nested JSON content.

## Decision

`SessionEvent` records first-class observability bridge fields:

- `observability_event`
- `managed_session_id`
- `inner_session_id`
- `inner_agent_id`
- `managed_agent_id`
- `parent_session_id`
- `environment_id`
- `action_name`

The managed-agent WASM modules write these fields on the high-signal
chronology rows:

- `session.status_running`
- derived `agent.message`, `agent.thinking`, `agent.tool_use`, and
  `agent.tool_result`
- `session.status_idle`
- `session.status_terminated`

Polling/check events are still omitted from this trace-like vocabulary so the
timeline stays useful instead of repetitive. Status boundary rows keep the same
values in the JSON `Content` field where that field is not already carrying
message content.

## Consequences

- Humans and agents can query the managed-session chronology directly through
  OData and Datadog facets.
- The entity timeline remains useful while live Datadog trace parenting is
  incomplete or unavailable.
- Polling/check events are not expanded with repeated trace-like rows; the
  added fields apply only to useful chronological rows.
