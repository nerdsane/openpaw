# ADR-029: Pin Temper Bounded Query Probes

- Status: Proposed
- Date: 2026-05-22
- Related:
  - Temper ADR-0119: Bound Query Projection Probes And Pages
  - TemperPaw ADR-028: Restore Prepared Context Inline Budget
  - Temper PR #273
  - TemperPaw PR #328

## Context

The OOM containment source fix landed in Temper commit
`63a2bef13ead464ff7a789ac18a4de99c28b4419`. TemperPaw production still pins
Temper crates and guest `temper-wasm-sdk` modules to the previous revision until
we explicitly roll that dependency forward.

The incident evidence points at two source-side memory hazards:

- pushed-down OData reads could materialize large sparse candidate sets before
  applying the final page;
- replay parity probes could collect tenant-wide entity IDs before truncating to
  the requested observe limit.

TemperPaw ADR-028 already removed the prepared-context inline memory amplifier.
This ADR records the matching rollout pin that brings the source fix into the
server, Docker build, and checked-in guest WASM manifests.

## Decision

Pin TemperPaw's Temper dependencies, Datadog pin contract, Docker build arg, and
guest WASM SDK manifests to
`63a2bef13ead464ff7a789ac18a4de99c28b4419`.

This keeps the runtime server crates and guest SDK contract aligned so deployed
WASM modules compile against the same host boundary as the server that will run
them.

## Consequences

Positive:

- Production can receive the bounded query projection page API.
- Replay parity probes use the store-level limited entity listing.
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
