# Foresight Railway DB Collocation Proof Handoff

Date: 2026-06-18

## Scope

Lane 4 re-verified the dedicated Railway `foresight` topology and moved the
current Foresight dev/proof storage off the Supabase pooler and onto a
dedicated collocated Railway Postgres database after explicit approval for the
fresh empty proof DB path.

No Supabase data was migrated, overwritten, or destroyed in this proof.

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

Before the switch, `foresight` was still using the Supabase pooler:

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

## Applied Change

Explicit approval was given for the fresh empty Railway proof DB path, not a
data-preserving migration.

Changed only Railway service `foresight` in project `openpaw-seshendranalla`,
environment `production`, which is the current dedicated Foresight proof
deployment. The shared `openpaw` service and shared `Postgres` database were
not targeted.

Applied variables:

- `DATABASE_URL=${{Postgres-v79y.DATABASE_URL}}`
- `DD_DBM_DATABASE_SERVICE=foresight-railway-postgres`
- `DD_TAGS=team:foresight db:railway-postgres db_service:foresight-railway-postgres`

Railway redeployed `foresight` successfully:

- Deployment ID: `55c673b1-1aa2-455f-befe-065b35683e70`
- Deployment status: `SUCCESS`
- Created at: `2026-06-18T18:55:53.710Z`

Post-switch variable readback resolved `DATABASE_URL` to the target Railway
private host `postgres-v79y.railway.internal`, not the Supabase pooler host
`aws-1-us-west-1.pooler.supabase.com`.

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

## Post-Switch Verification

Railway deployment/readiness:

- `foresight` deployment `55c673b1-1aa2-455f-befe-065b35683e70` reached
  `SUCCESS`.
- Environment status showed `foresight` and `Postgres-v79y` as `SUCCESS`.
- `GET https://foresight-production-72d1.up.railway.app/healthz` returned HTTP
  200.

Database/schema:

Read-only `psql` against `Postgres-v79y` via the Railway TCP proxy returned:

- `db=railway`
- `public_tables=24`
- `_sqlx_migrations` table present
- `migration_rows=7`

Service/OData:

- First authenticated `$metadata` request briefly returned HTTP 503 while the
  fresh DB deployment settled.
- After a short settle window, authenticated
  `GET /tdata/$metadata` with tenant `deep-sci-fi` returned HTTP 200 and a full
  OData metadata document.
- Authenticated `GET /tdata/Worlds?$top=1` with tenant `deep-sci-fi` returned
  HTTP 200 and an empty `value` array, expected for the fresh proof DB.

Parser smoke:

- Ran targeted Lane 1 parser regression smoke:
  `cargo test openai_completed_text_does_not_clobber_streamed_function_call`
  from `os-apps/paw-agent/wasm/provider_caller`.
- Result: pass, 1 test passed.

## Startup and Schema Behavior

TemperPaw connects to Postgres through `DATABASE_URL` when
`TEMPER_EVENT_STORE=postgres`. Startup uses `TEMPER_POSTGRES_MAX_CONNECTIONS`
for the sqlx pool and runs Postgres migrations before serving.

Source reference: `crates/temperpaw/src/storage.rs`.

That means a fresh `Postgres-v79y` cutover should create the base schema on
startup, but preserving existing `deep-sci-fi` state requires a logical
Supabase export and restore before switching the app.

## Switch Plan Used

1. Left Supabase untouched as rollback.
2. Set `foresight` `DATABASE_URL` to `${{Postgres-v79y.DATABASE_URL}}`.
3. Set `DD_DBM_DATABASE_SERVICE=foresight-railway-postgres`.
4. Added service tags for the Railway Postgres path.
5. Kept `TEMPER_EVENT_STORE`, `TEMPER_PLATFORM_STORE`, and
   `TEMPER_QUERY_PROJECTION_STORE` as `postgres`.
6. Let Railway redeploy `foresight`.
7. Verified health, schema migration, authenticated OData reads, and the parser
   smoke.

## Rollback Plan

Rollback is to restore the prior `foresight` Supabase `DATABASE_URL`, restore
`DD_DBM_DATABASE_SERVICE=foresight-supabase`, and redeploy. Supabase must remain
unchanged until the Railway DB proof is accepted.

Use the stored Railway variable history or prior secret record to restore the
Supabase pooler URL. Also restore `DD_TAGS=team:foresight` if the DB path tag
should be removed during rollback.

## Completion Status

Complete for the fresh empty Railway proof DB path.

Residual follow-ups:

- DBM coverage for `Postgres-v79y` still needs a dedicated Datadog DBM agent or
  equivalent monitoring update; existing `datadog-postgres-agent` points at the
  shared `Postgres` host.
- Lane 2 still owns reducing database work after collocation: snapshots,
  projections, OTS, and `Session` / `SessionEntry` scans.
