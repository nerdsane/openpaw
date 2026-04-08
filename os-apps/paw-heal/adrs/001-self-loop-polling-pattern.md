# ADR-001: Self-Loop Polling for AlertCycle

**Status:** Accepted
**Scope:** integrations
**Author:** OpenPaw maintainers
**Date:** 2026-04-08

## Context

Healing workflows need to wait for external systems such as CI, deployment targets, or monitors to settle. Doing that wait in Rust background tasks would hide orchestration from Temper and bypass the entity-first architecture described in ADR-0005.

## Decision

`AlertCycle`-style workflows use self-loop actions plus bounded counters for polling. A WASM integration performs the external check, dispatches the next self-loop action when another check is needed, and self-reports completion or escalation when the workflow is done. No business-logic `tokio::spawn` or daemon-side watcher owns the loop.

## Consequences

### Positive
- The full wait/check/retry sequence is visible in entity history and trajectory records.
- Cedar, timeouts, and audit tooling apply to the whole healing loop.
- Agents can evolve the workflow by changing specs and integrations instead of editing daemon code.

### Negative
- Poll cadence must be tuned in the state machine instead of an imperative loop.
- Long stabilization windows produce more explicit state transitions than a hidden background task would.
