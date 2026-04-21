# ADR-0038: Queue Depth vs Steady-State Concurrency

- Status: Accepted
- Date: 2026-04-20
- Deciders: OpenPaw maintainers
- Related:
  - ADR-0036 (session-liveness-migration-and-heartbeat-retirement.md): retired `submit_next_queued_regeneration` on the assumption admission caps would subsume it
  - temper ADR-0051 (admission-control-in-dispatch.md): the admission primitive this ADR scopes
  - `os-apps/paw-agent/specs/session.ioa.toml` (admission caps)
  - `reference-projects/katagami-curation/wasm/finalize_spawned_session/src/lib.rs` (fan-out site)

## Context

On 2026-04-19, a Katagami curation run wedged the server with 27 active sessions stuck in `CallingProvider` / `Executing` for 37+ hours. Root cause was a separate bug (a global monty_repl semaphore defaulting to 2 — fixed on branch `fix/session-pileup-remediation`), but the incident exposed a gap in how ADR-0036 reasoned about capacity.

ADR-0036 retired the dequeue pattern `submit_next_queued_regeneration` (which submitted regeneration jobs serially, one-per-completion) with the rationale:

> With ADR-0051 caps on Session, callers can Submit all 11 regenerate_embodiment jobs at once and Temper queues them FIFO.

That rationale treats admission caps as interchangeable with in-flight concurrency control. They are not.

## Decision

Document — as an architectural invariant — that **admission caps throttle arrival rate, not steady-state in-flight work**, and encode the consequences for code authors.

### What admission caps do

`[admission]` on an entity spec (ADR-0051, applied here in `session.ioa.toml`) bounds:

- `max_concurrent_creates` — how many fresh entity creations can be in-flight at once.
- `max_concurrent_actions = { "Action" = N }` — how many concurrent dispatches of a given action are allowed.
- `queue_depth` — how deep the wait queue gets before new submissions are rejected.
- `queue_timeout_seconds` — how long a submission waits in the queue before it fails fast.

These are **burst protection primitives.** They prevent the system from being asked to accept more work per second than it can safely absorb. Once a permit is granted and the work begins, admission caps no longer apply to that unit of work.

### What admission caps do NOT do

They do not bound **total concurrent in-flight work**. With `max_concurrent_creates = 10` and `queue_depth = 100`, a caller can Submit 110 entities in a burst, admission will absorb them (10 immediately, 100 queued), and within seconds all 110 are active simultaneously. From that point on, contention for downstream resources (LLM provider, sandbox pool, database writes) is unconstrained by this primitive.

Admission caps describe **how fast work may be admitted**, not **how much work may be running at once.**

### The 2026-04-19 shape

Katagami's `finalize_spawned_session` fan-out loops over `discovered_movements` and submits one synth job per direction with no wait between iterations. With 11 movements, 11 synth sessions get created in rapid succession. Admission caps (10 concurrent creates, queue depth 100) absorb the burst trivially. Within seconds, 11 sessions are active and competing for the shared LLM provider path. ADR-0036's rationale assumed admission would prevent exactly this — it did not, because that's not what admission does.

### Where steady-state concurrency belongs

Three valid places, in increasing order of blast radius:

1. **Orchestrator-side**: the caller that fans out work paces itself. This is what `submit_next_queued_regeneration` did — it walked the queue one job at a time, submitting the next only after the prior completed. Appropriate for bursts where the caller has visibility into completion.

2. **Downstream gate on the contended resource**: e.g., a per-provider token-bucket rate limiter around LLM calls, or a bounded sandbox pool. Appropriate when many unrelated callers share one scarce resource.

3. **Per-tenant admission cap on the downstream entity**: e.g., a cap on concurrent `Session.call_provider` actions (not `Session.Configure` creates). This is admission applied at the correct layer — gating the action that touches the scarce resource, not the entity-creation action upstream of it.

A global single-value semaphore defaulting to 2 across all tenants — the pattern the 2026-04-19 incident exposed — is none of the above. It is cost-control theater that throttles the platform to a fixed bottleneck regardless of capacity or demand. Don't do that.

### What this means for authors

- When you retire an orchestrator-side pacing pattern (as ADR-0036 did with `submit_next_queued_regeneration`), do not assume admission caps replace it. They protect against burst, not steady-state.
- When you write fan-out code, ask: *what bounds the number of concurrently running downstream units of work?* If the answer is "admission queue_depth," that is probably wrong.
- When you want to limit real concurrency, put the gate on the contended resource (provider call, sandbox acquisition), not on the entity creation.

## Consequences

### Positive

- The rationale in ADR-0036 is corrected: admission caps were a *complementary* fix to burst dispatch, not a replacement for orchestrator-side pacing.
- Future fan-out code (Katagami, paw-consilium, paw-research) can reason about concurrency at the right layer.
- Retiring the monty_repl semaphore on `fix/session-pileup-remediation` becomes defensible: the correct gate is a per-tenant action cap or a provider-side rate limiter, not a global fixed number.

### Negative / follow-ups

- We do not yet have a downstream rate limiter on LLM provider calls. With the monty gate removed, the next bottleneck to discover is likely Anthropic / OpenRouter 429s, Turso WAL write contention, or Modal sandbox exhaustion. This is intentional — see the "next failure" tracker in the `fix/session-pileup-remediation` work.
- Katagami's `finalize_spawned_session:352-446` fan-out still submits all discovered movements at once. Whether that needs orchestrator-side pacing depends on what we observe once downstream gates are proven. Do not add a limit before there's evidence of need; do not assume admission caps cover it.

## Open Question

Whether to introduce a per-action admission cap on `Session.call_provider` (or `CurationJob.Submit` for synth jobs specifically) as the permanent answer. This would be admission applied at the correct layer — on the action that hits the scarce resource, not on Session creation. Deferred to whichever incident surfaces next after the monty gate rip.
