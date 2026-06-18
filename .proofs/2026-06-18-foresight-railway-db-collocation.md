# Foresight Railway DB Collocation Proof Handoff

Date: 2026-06-18

## Scope

Lane 4 re-verified the dedicated Railway `foresight` topology and prepared the
reversible plan to move current Foresight dev/proof storage off Supabase pooler
and onto a dedicated collocated Railway Postgres database.

No production data was migrated, overwritten, or destroyed in this proof.

## Repo State

- Primary checkout `/Users/seshendranalla/Development/temperpaw` was dirty on
  `codex/infinite-history-bounded-actors` and was not edited.
- Worktree:
  `/Users/seshendranalla/Development/temperpaw-worktrees/foresight-railway-db-collocation`
- Branch: `codex/foresight-railway-db-collocation`
- Base: `origin/main` at `02716509`
- Initial worktree status: clean.

## Railway Topology

Project: `openpaw-seshendranalla`

Production environment: `production`

Observed production services:

| Service | Role | Region | Latest status | Notes |
| --- | --- | --- | --- | --- |
| `foresight` | Dedicated Foresight app | `us-east4-eqdc4a` | `SUCCESS` | Image `ghcr.io/nerdsane/temperpaw:sha-0271650` |
| `Postgres-v79y` | Candidate dedicated Foresight DB | `us-east4-eqdc4a` | `SUCCESS` | Empty 50 GB volume, private host `postgres-v79y.railway.internal` |
| `Postgres` | Existing shared Railway Postgres | `us-east4-eqdc4a` | `SUCCESS` | About 6.4 GB used; do not use for Foresight without explicit approval |
| `openpaw` | Existing OpenPaw app | `us-east4-eqdc4a` | `SUCCESS` | Separate from dedicated `foresight` |
| `datadog-postgres-agent` | Current DBM agent | `us-east4-eqdc4a` | `SUCCESS` | Points at the shared `Postgres` host, not `Postgres-v79y` |

`Postgres-v79y` volume evidence:

- Mount path: `/var/lib/postgresql/data`
- Size: 50 GB
- Used: 0 MB at verification time
- State: `READY`

## Current Foresight DB State

`foresight` is still using the Supabase pooler:

- DB host: `aws-1-us-west-1.pooler.supabase.com`
- Pooler port: `5432`
- SSL mode: required
- Tenant: `PAW_TENANT=deep-sci-fi`
- Storage backends: Postgres for event, platform, and query projection stores
- Current app max Postgres connections: `TEMPER_POSTGRES_MAX_CONNECTIONS=40`
- DBM service tag: `DD_DBM_DATABASE_SERVICE=foresight-supabase`

The target Railway database has internal private host
`postgres-v79y.railway.internal` and should be referenced through Railway
variables, not by copying credentials into the app.

## Baseline Measurements

Railway `foresight` HTTP response sample before the switch:

- Sample: 77 requests
- p50: 2 ms
- p90: 3 ms
- p95: 5 ms
- p99: 3489 ms

Railway `foresight` service metrics over the last hour before the switch:

- CPU average: 0.0358, max 0.1376
- Memory average: 1.6230 GB, max 1.9189 GB
- Network RX average: 0.0045 GB
- Network TX average: 0.0021 GB

Datadog DBM searches for recent `foresight` / `foresight-supabase` samples did
not return samples in the last hour. Treat DBM database-instance coverage as a
gap to fix during or immediately after the Railway DB switch.

## Startup and Schema Behavior

TemperPaw connects to Postgres through `DATABASE_URL` when
`TEMPER_EVENT_STORE=postgres`. Startup uses `TEMPER_POSTGRES_MAX_CONNECTIONS`
for the sqlx pool and runs Postgres migrations before serving.

Source reference: `crates/temperpaw/src/storage.rs`.

That means a fresh `Postgres-v79y` cutover should create the base schema on
startup, but preserving existing `deep-sci-fi` state requires a logical
Supabase export and restore before switching the app.

## Recommended Switch Plan

Stop before mutation and choose one path:

1. Fresh proof database:
   - Leave Supabase untouched as rollback.
   - Set `foresight` `DATABASE_URL` to `${{Postgres-v79y.DATABASE_URL}}`.
   - Set `DD_DBM_DATABASE_SERVICE=foresight-railway-postgres`.
   - Keep `TEMPER_EVENT_STORE`, `TEMPER_PLATFORM_STORE`, and
     `TEMPER_QUERY_PROJECTION_STORE` as `postgres`.
   - Redeploy `foresight`.
   - Verify health, tenant reads, startup migration, and a fresh corridor flow.

2. Data-preserving migration:
   - Freeze `foresight` writes.
   - `pg_dump` the current Supabase database.
   - Restore into `Postgres-v79y`.
   - Validate `_sqlx_migrations`, row counts, tenant coverage, and known
     Foresight entities.
   - Switch `DATABASE_URL` and DBM tag.
   - Redeploy and verify both known state and a fresh corridor flow.

## Rollback Plan

Rollback is to restore the prior `foresight` Supabase `DATABASE_URL`, restore
`DD_DBM_DATABASE_SERVICE=foresight-supabase`, and redeploy. Supabase must remain
unchanged until the Railway DB proof is accepted.

## Completion Status

Blocked pending explicit user authorization for one of:

- fresh empty Railway proof DB; or
- data-preserving migration of current Supabase `deep-sci-fi` data.

No Railway env variable mutation has been performed by Lane 4 yet.
