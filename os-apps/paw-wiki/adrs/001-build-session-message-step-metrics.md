# ADR 001: Build Session Message Step Metrics

## Status

Accepted

## Context

Datadog traces for production `wasm:build_session_message` show rare but material
tail latency in WikiJob child-session spawning. The observed slow spans are mostly
idle from the guest perspective, which means the guest is waiting on host/OData
work rather than burning CPU inside the WASM module.

The integration currently performs several stateful calls in sequence:

- ensure or reuse the workspace
- create a `Session`
- dispatch `Session.Configure`
- dispatch `WikiJob.SessionSpawned`
- create a `SessionLink`
- dispatch `SessionLink.Configure`

Those calls are semantically important. `SessionLink` is the bounded,
Temper-native monitor for the child session, and failures must remain visible by
failing the parent WikiJob. Before compressing or changing this flow, we need a
production-safe step breakdown that can distinguish ordinary host latency from a
specific OData or entity action bottleneck.

## Decision

Instrument `build_session_message` with histogram metrics for each stateful step
while preserving the existing entity-first behavior and failure semantics.

The module will emit `temper_wiki_build_session_message_step_duration_ms` with
tags:

- `job_type`
- `step`
- `result`

The measured steps are:

- `ensure_workspace`
- `create_session`
- `configure_session`
- `session_spawned`
- `create_session_link`
- `configure_session_link`
- `total`

Each step records `ok` or `error` before the existing error is returned. The
workspace step may record `skipped` when the WikiJob already carries a
`workspace_id` and no lookup is needed. The metric name is app-specific so
WikiJob dashboards can isolate it without mixing it into the core Session phase
metrics.

## Consequences

This does not make the path faster by itself. It makes the next optimization
safe: after deployment, Datadog can show which exact OData/action boundary
dominates p95/p99. That lets us choose between targeted options such as
combining create/configure paths, changing parent notification ordering, or
optimizing a specific host route without guessing.

The instrumentation adds small metric emission overhead to a non-inner-loop
integration. That overhead is acceptable because the path already performs
multiple OData calls and because the metric is required to prevent correctness
regressions while pursuing latency reductions.
