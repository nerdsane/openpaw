# ADR-0062: Directed Evolution Worker Concurrency

- Status: Accepted
- Date: 2026-06-08
- Deciders: TemperPaw maintainers

## Context

The local `paw-codex-worker` can queue many Directed Evolution `WorkItems`,
but boot/backlog pickup processed queued work items with a sequential
`for ... await` loop. Setting `MAX_CONCURRENT_RUNS=4` therefore did not make a
single worker process run four simulated users at once; it only changed the
reported worker capacity. Operators had to start multiple worker processes to
see parallel seed journeys.

## Decision

Directed Evolution boot/backlog pickup will process queued `WorkItems` with a
bounded unordered async executor capped by `MAX_CONCURRENT_RUNS`. Each work item
still uses the existing claim action before execution, so concurrent workers
can race safely and losers observe the already-claimed state.

The worker event stream remains token-only by default. Local Genesis/proxy
setups that require an observe principal for SSE may set
`PAW_CODEX_EVENT_STREAM_PRINCIPAL_KIND` and
`PAW_CODEX_EVENT_STREAM_PRINCIPAL_ID`; those headers are applied only to event
stream requests and do not change WorkItem action identity.

## Consequences

A single local worker process can run multiple simulated-user journeys in
parallel. Worker process pools remain useful for isolation or capacity, but are
no longer required just to satisfy the concurrency setting.

Local workers can also connect to observe-protected streams without weakening
normal WorkItem mutation headers.

## Non-Goals

This does not change `WorkerRun`, `ReviewRun`, or `EvaluationRun` pickup
semantics. It also does not replace polling fallback; event-stream delivery is
handled separately by the runtime/proxy path.
