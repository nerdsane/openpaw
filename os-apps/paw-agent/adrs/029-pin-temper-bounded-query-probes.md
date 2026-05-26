# ADR-029: Pin Temper Bounded Query Probes

- Status: Proposed
- Date: 2026-05-22
- Related:
  - Temper ADR-0119: Bound Query Projection Probes And Pages
  - TemperPaw ADR-028: Restore Prepared Context Inline Budget
  - Temper PR #273
  - Temper PR #281
  - TemperPaw PR #328

## Context

The current Temper source fix set includes the OOM containment work and
stack-safe Genesis policy recovery in Temper commit
`7f7602680ae65953540f7b89bf249970fd74beac`. TemperPaw production still pins
Temper crates and guest `temper-wasm-sdk` modules to the previous revision until
we explicitly roll that dependency forward.

The incident evidence points at two source-side memory hazards:

- pushed-down OData reads could materialize large sparse candidate sets before
  applying the final page;
- replay parity probes could collect tenant-wide entity IDs before truncating to
  the requested observe limit.

TemperPaw ADR-028 already removed the prepared-context inline memory amplifier.
This ADR records the matching rollout pin that brings the source fixes into the
server, Docker build, and checked-in guest WASM manifests.

## Decision

Pin TemperPaw's Temper dependencies, Datadog pin contract, Docker build arg, and
guest WASM SDK manifests to
`7f7602680ae65953540f7b89bf249970fd74beac`.

This keeps the runtime server crates and guest SDK contract aligned so deployed
WASM modules compile against the same host boundary as the server that will run
them.

## Consequences

Positive:

- Production can receive the bounded query projection page API.
- Replay parity probes use the store-level limited entity listing.
- Genesis-sourced Katagami policy rows recover on restart without reloading a
  duplicate multi-megabyte generated policy under thousands of synthetic names.
- A stale or missing configured Genesis bootstrap ref no longer takes down an
  existing database-backed Paw instance; startup logs the failure and continues
  with durable installed-app recovery.
- Datadog contract tests prevent a partial rollout with stale guest SDK pins.

Tradeoffs:

- This is a rollout dependency change, not by itself a new latency win.
- Production acceptance still requires Docker publication, Railway deployment,
  live probes, and Datadog RSS/query evidence.

## Verification

- Run the Datadog observability contract to prove all active pins match.
- Run `cargo check --locked -p temperpaw`.
- Build the OS app WASM bundle.
- After deploy, verify `/paw/version`, hot SessionEntries probes, RSS stability,
  and absence of repeated restart cadence in Datadog.
