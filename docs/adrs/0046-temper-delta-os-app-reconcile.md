# ADR-0046: Temper Delta OS-App Reconcile for Startup

**Status:** Accepted
**Date:** 2026-04-27
**Related:** ADR-0001, ADR-0005, ADR-0045, Temper ADR-0062

## Context

OpenPaw startup already uses Temper's `reconcile_os_app` API for required startup
apps. Production traces after a dependency-only deploy showed that this was still
too coarse: a rebuilt WASM artifact changed the app bundle digest, and Temper
treated that as a full app install. Startup then re-persisted specs, re-upserted
WASM bytes, and re-bootstrapped content for apps whose non-WASM state had not
changed.

This is a platform reconcile issue, not an OpenPaw startup orchestration issue.
Adding app-specific Rust shortcuts in `crates/temperpaw/` would violate the
entity-first architecture and make the startup path harder to audit.

## Decision

OpenPaw will consume Temper's component-aware OS-app reconcile contract from
Temper ADR-0062:

1. Temper compares installed app subdigests for specs, policies, WASM, content,
   and seed data.
2. Temper runs only the changed install phases.
3. WASM module bytes are treated as content-addressed artifacts; SQL stores
   module metadata and legacy inline rows remain readable.
4. OpenPaw startup continues to call `reconcile_os_app` for required startup
   apps and does not add a separate imperative reconcile layer.
5. When `paw-agent` is part of the startup surface, OpenPaw does not bootstrap
   the built-in default agent specs first. The `paw-agent` OS app owns those
   specs, which prevents `Agent` and `Plan` from being rewritten before every
   startup reconcile.
6. `/readyz` remains blocked until required startup apps are usable; this change
   reduces required work rather than hiding it.

## Consequences

WASM-only deploys should no longer re-run app spec persistence, verification
bootstrap, agents, skills, system files, ADRs, or seed data for unchanged app
components.

Spec-only changes should not rewrite WASM module artifacts or app content.

OpenPaw's main follow-up after the Temper change lands is dependency consumption:
update the Temper git revision in `Cargo.lock`, deploy, and confirm the phase 6b
startup traces show delta reconcile work instead of full app install work for
unchanged components.

Skipping the built-in default agent bootstrap while `paw-agent` owns the startup
agent surface removes an otherwise hidden spec-content conflict: Phase 4b used
to write platform default `Agent`/`Plan` specs, and Phase 6b then wrote the
`paw-agent` versions. That made warm boots dirty even when the installed app
digest matched.

## Verification

Verification requires:

- Temper tests for idempotent spec and WASM persistence
- Temper tests for component-aware reconcile planning and installer behavior
- OpenPaw startup traces showing phase 6b app reconcile avoids unrelated phases
  after a WASM-only or content-only bundle digest change
- an OpenPaw local cold/warm startup e2e showing unchanged app skips do not bump
  spec or WASM metadata versions
- a deployment proof that `/readyz` still waits for required usable surfaces

## Non-Goals

- Adding OpenPaw-specific Rust orchestration for app reconcile
- Marking readiness healthy before required startup apps are usable
- Replacing all blob storage with external R2/S3 in OpenPaw startup code
