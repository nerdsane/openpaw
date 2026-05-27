# ADR-0060: Directed Evolution Public Runtime Prompts

- Status: Proposed
- Date: 2026-05-27
- Deciders: TemperPaw maintainers

## Context

A live repair-autostart cycle proved the Directed Evolution worker can drive a
real signal through observer, repair-autostart, variant generation, hot-loaded
variant tenants, reviewer, simulated-user, and elimination state. It also exposed
a worker/runtime prompt mismatch.

Evaluation work items carried a `RuntimeRef` for a hot-loaded Genesis tenant, but
their prompt still said `TemperApiBase: http://127.0.0.1:8080` when the app
tenant did not have a resolved `temper_public_api_url` secret. One simulated-user
agent compensated by starting `temper serve` locally and leaving it in the
foreground, which stalled the brain run until the local server process was
terminated.

The local worker already knows the public Genesis URL used for hot-loading.

## Decision

For reviewer and simulated-user Directed Evolution roles, the local Codex worker
will rewrite loopback `TemperApiBase` prompt lines to the configured public
runtime URL:

- `DIRECTED_EVOLUTION_PUBLIC_API_URL`, when set
- otherwise `DIRECTED_EVOLUTION_GENESIS_URL` / `GENESIS_URL`

The worker also appends runtime execution discipline to those prompts:

- use `TemperApiBase` plus the tenant parsed from `RuntimeRef`
- do not start a foreground long-lived server
- if a local server is unavoidable, stop it before returning JSON
- fail with clear evidence rather than hanging when runtime execution is
  unavailable

Variant-generator prompts are not rewritten because they work in a repository
worktree and may need local tools before publishing/hot-loading the candidate.

## Consequences

- Simulated-user and reviewer brains should exercise the already-hot-loaded
  Genesis runtime instead of treating localhost as the runtime by default.
- Brain runs are less likely to hang behind foreground `temper serve`.
- This remains a trusted local worker capability and does not require Codex to
  run in Railway.

## Non-Goals

- This ADR does not change Directed Evolution app state machines.
- This ADR does not add a Datadog API client to `paw-codex-worker`.
- This ADR does not replace app-side `temper_public_api_url` configuration.
