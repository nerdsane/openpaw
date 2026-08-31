# Spec: unique sandbox exec capture id per entity
Status: accepted. Intent: docs/efforts/ARN-401/intent.md

## Requirements
- The capture id used to build `/tmp/.paw-{out,err,rc}-<id>` must be distinct for
  any two execs that can run concurrently on one sandbox.
- The id must be filename- and shell-safe; an absent calling entity falls back to
  a safe token.
- Pure and unit-testable (no host calls inside the id-building function).

## Design
`unique_run_id(ctx)` composes the id from three parts:
- `ctx.entity_id` — the calling entity. Two different entities (the real
  concurrency, e.g. a rollback vs a health probe) never collide because their
  ids differ.
- `get_time_millis()` — the host clock, separating repeats over time.
- a per-instance `AtomicU32` counter — separating repeats within one instance.

`capture_run_id(entity_id, now_millis, seq)` is the pure builder. The entity id
is encoded **injectively**: ASCII alphanumerics and `-` pass through; every other
byte — `_` included — becomes `_` + two hex digits. Because `_` is itself
escaped, it only ever marks an escape, so two distinct entity ids can never map
to the same token (the earlier lossy `→ _` scheme collided `a/b` with `a?b`). An
empty entity id falls back to `exec`.

## Policy / invariants
- Determinism: the builder is pure; the only non-determinism (clock, counter)
  lives in the caller, matching the existing exec path.
- Safety: the encoded token cannot contain `/`, whitespace, quotes, or `.`, so it
  cannot escape `/tmp/.paw-*` or break the shell redirection.

## Same-entity concurrency (why entity scope suffices)
Temper entities are actors that process one message at a time
(`temper-runtime` `actor/traits.rs`: "Called sequentially — one message at a
time"; `actor/cell.rs` is a strict `recv().await` → `handle(...).await` → loop,
and WASM integrations run inline within the awaited `handle()` — the only
`tokio::spawn` is the actor's own run-loop). So two dispatches for the SAME
entity are serialized: the first exec writes, reads, and deletes its capture
files before the second begins. Concurrency is only ever cross-entity, which the
entity-scoped id covers. Entity scope is therefore sufficient; no per-dispatch
nonce is needed (and none is available in pure WASM).

## Deferred / out of scope
- Streaming / cancel / long-run (>poll-cap) exec — ARN-443 part D.
- Rebuilding + republishing the `computer_exec` blob to Genesis — done separately
  (Genesis already carries the rebuilt blob); not part of this repo-side change.
