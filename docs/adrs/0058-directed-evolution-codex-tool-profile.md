# ADR-0058: Directed Evolution Codex Tool Profile

- Status: Proposed
- Date: 2026-05-27
- Deciders: TemperPaw maintainers

## Context

Directed Evolution uses local Codex workers as the first implementation of the
brain. Observer, reviewer, and simulated-user roles need access to authenticated
tooling such as Datadog MCP so their evidence can be independently inspected.

The worker currently launches child Codex sessions with `--ignore-user-config`.
That keeps generic repo work isolated, but it also hides MCP server definitions.
A live observer run proved the problem: it created a useful direction, but its
`evidence_scope` reported that Datadog tools were not available inside the child
Codex session.

## Decision

Keep isolated child Codex execution as the default. Add an explicit local
operator opt-in that injects only the Datadog MCP server definition while still
passing `--ignore-user-config`:

- `PAW_CODEX_ENABLE_DATADOG_MCP=1`
- optional `PAW_CODEX_DATADOG_MCP_URL` override

This is a worker capability profile, not a Railway deployment change. The
trusted local TemperPaw worker can expose Datadog MCP to child Codex brain
runs without inheriting arbitrary user model, instruction, profile, or sandbox
configuration. Temper-native Directed Evolution apps continue to hot-load into
the running Temper server through Genesis.

## Consequences

- Default worker runs stay isolated and deterministic with respect to Codex
  config.
- Directed Evolution evidence roles can be run locally with Datadog MCP access
  and return real `evidence_scope.datadog_url` values.
- Operators must treat Datadog MCP access as trusted-local mode because child
  Codex sessions can use the configured Datadog tool surface.
- No Codex process is expected to run inside the Railway-hosted Temper server.

## Non-Goals

- This ADR does not add a Datadog API client to `paw-codex-worker`.
- This ADR does not require a Railway redeploy for app/spec/WASM iteration.
- This ADR does not change Genesis app hot-loading semantics.
