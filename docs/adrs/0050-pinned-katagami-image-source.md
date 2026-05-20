# ADR-0050: Pinned Katagami Image Source

## Status

Accepted

## Context

TemperPaw's production image bakes the Katagami OS apps into `/app/os-apps`
during the Docker build. A Railway redeploy of the same TemperPaw image
therefore restarts the service but does not pick up newly merged Katagami
changes.

PERF-035 proved this boundary directly. Katagami PR #30 merged the Datadog
count-style curation step metrics, but a same-image Railway redeploy still ran
the old `build_session_message` WASM hash `8a2c2d...`, not the new checked-in
WASM hash from Katagami `d16c992`.

## Decision

Pin the Docker build's Katagami source to the exact Katagami commit required by
the deployment:

- `d16c99213fcd2ff5bff426539eb5831e1ae029a7`

The Dockerfile now fetches the requested ref into a temporary checkout and
checks out `FETCH_HEAD`, which supports branch names, tags, and exact commits.

## Consequences

- A TemperPaw image digest now records the exact Katagami source content used to
  build production curation WASM.
- Rebuilding the TemperPaw image is required for future Katagami app changes;
  a same-image Railway redeploy is not enough.
- PERF-035 Datadog proof can distinguish image packaging from guest telemetry
  behavior.
