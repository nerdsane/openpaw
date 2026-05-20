# ADR-0049: Manual Railway Redeploy Workflow

Date: 2026-05-20

## Status

Accepted

## Context

Latency slices now require exact deployment proof: PR merge, Docker image
publication, Railway deploy, live version probe, and Datadog after evidence.
PERF-033 reached merge and Docker publication, but the deploy step was blocked
because local Railway CLI auth expired and production's cached Railway token was
stale.

GitHub already publishes the verified `edge` and `sha-*` GHCR images. The
missing operational boundary is a repository-owned manual redeploy path that can
use GitHub environment secrets without depending on a local browser login.

## Decision

Add a manual `workflow_dispatch` GitHub Actions workflow for Railway redeploys.
The workflow:

- accepts an image tag constrained to `edge`, `latest`, or `sha-*`;
- uses Railway GraphQL with repository or environment secrets for the project,
  environment, service, and token;
- upserts `IMAGE_TAG` with `skipDeploys: true`;
- redeploys the current Railway service deployment with `deploymentRedeploy`;
- optionally polls `/readyz` and authenticated `/paw/version` until the expected
  SHA is live, using either a dispatch-time `base_url` input or a
  `TEMPERPAW_BASE_URL` secret/variable.

The workflow does not build a new image. It only deploys a previously published
GHCR image.

## Consequences

Latency-program deploy proof is no longer dependent on a local Railway CLI
session. Operators can still use the CLI or the production redeploy endpoint,
but this workflow provides a clean manual fallback for exact after-proof
rollouts.

The workflow requires GitHub `production` environment secrets or variables:
`RAILWAY_TOKEN`, `RAILWAY_PROJECT_ID`, `RAILWAY_ENVIRONMENT_ID`,
`RAILWAY_SERVICE_ID`, `TEMPERPAW_BASE_URL`, and optionally `TEMPER_API_KEY` for
version proof. Operators may override the base URL at dispatch time for a
one-off proof target.

The workflow intentionally remains manual. Automatic deployment on every main
push would weaken the measured before/after cadence used by the latency program.
