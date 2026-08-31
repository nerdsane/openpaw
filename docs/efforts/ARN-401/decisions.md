# Decisions - unique sandbox exec capture id per entity

## Scope the capture id to the calling entity (not a global nonce)
- **Decision:** Build the capture id from `entity_id` + host clock + per-instance
  counter, rather than a globally-unique nonce.
- **Came up because:** the collision that caused ARN-401 was cross-entity (a
  ReleaseRun rollback vs a different health-probe entity running at once).
- **Options:** global nonce (needs entropy WASM does not have) / entity scope.
- **Chose entity scope over a nonce because:** pure WASM has no host RNG or
  per-dispatch id; entity scope kills the real (cross-entity) collision with the
  inputs actually available. Given up: nothing for the real threat — same-entity
  overlap does not occur (see next decision).
- **Where:** `os-apps/paw-agent/wasm/wasm-helpers/src/sandbox.rs` `unique_run_id`.

## Make the entity-id encoding injective (hex-escape)
- **Decision:** Encode the entity id injectively — alphanumerics and `-` pass
  through; every other byte, `_` included, becomes `_` + two hex digits.
- **Came up because:** Greptile flagged the lossy `→ _` sanitizer: `a/b` and
  `a?b` both collapsed to `a_b`, so distinct entities could still collide.
- **Options:** merge as-is / append a hash suffix (probabilistic) / injective
  hex-escape (provable).
- **Chose injective over a hash because:** it is provably collision-free, keeps a
  readable prefix, and all prior tests still pass. Given up: slightly longer
  tokens for punctuated ids.
- **Where:** `capture_run_id`, commit on `claude/exec-capture-isolation`.

## No same-entity discriminator: the actor model serializes it
- **Decision:** Do not add a per-dispatch discriminator for same-entity execs.
- **Came up because:** Greptile then asked about two `run_tools` dispatches for
  the SAME session colliding within one millisecond.
- **Options:** add sandbox-side `mktemp`/nonce machinery / rely on the execution
  model.
- **Chose rely-on-the-model because:** Temper actors process one message at a
  time (`temper-runtime` `actor/traits.rs`, `actor/cell.rs`: `recv().await` →
  `handle().await` → loop; integrations run inline, the only `tokio::spawn` is the
  actor's own loop). Same-entity dispatches are serialized — the first exec
  writes/reads/deletes before the second starts — so the collision cannot occur.
  Adding machinery would violate "no machinery for a non-problem." Given up:
  nothing; the finding does not apply to this execution model.
- **Where:** analysis in docs/efforts/ARN-401/spec.md; no code change.
