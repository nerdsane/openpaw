# ADR-002: Parent Session Provenance For Approval Routing

- Status: Accepted
- Date: 2026-05-11

## Context

Managed SWE sessions are modeled as `ManagedSession` entities that bridge into
inner `OpenPaw.Session` entities through the `session_orchestrator` integration.
When an inner session hit a governed tool denial, `request_approval` could route
notifications only if the inner session preserved a parent session relationship
back to the chat-bound PO session.

Normal `temper.spawn_session()` flows already set `parent_session_id`, but
`temper.create("ManagedSessions", ...)` did not stamp that provenance and the
managed-session spec had no field to persist it.

## Decision

`ManagedSession` records `parent_session_id`. When an agent creates a
`ManagedSession` from inside an active `Session`, the Monty REPL stamps the
current session id into the created entity unless the caller supplied an
explicit parent. The managed session orchestrator propagates that value into the
inner `Session.Configure` action as `parent_session_id`.

## Consequences

- Approval notifications from delegated SWE sessions can route back through the
  PO session's Discord or Slack channel binding.
- Managed-agent execution remains Temper-native: provenance is entity state, and
  routing behavior is derived from state transitions.
- Callers can still override the parent explicitly for future handoff flows.
