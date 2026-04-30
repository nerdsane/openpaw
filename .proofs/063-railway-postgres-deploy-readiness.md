# 063 - Railway Postgres deploy readiness

Date: 2026-04-29

## Scope

Addressed the gap where Temper supported Postgres but the TemperPaw wrapper and
deploy flow still assumed Turso.

## Evidence

- `cargo test -p temperpaw` passed: 53 unit tests, 2 `paw_fs_versioning`
  tests, 4 `session_lifecycle_and_config` tests, 10
  `session_turn_architecture` tests.
- `cargo test -p temperpaw-cli` passed: 15 deploy/CLI tests.
- `cargo check -p temperpaw` passed.
- `cargo build -p temperpaw` passed.
- Local Postgres boot proof:
  - Started `postgres:16-alpine` on `127.0.0.1:55432`.
  - Ran `target/debug/temperpaw-server` from a clean proof cwd with:
    - `TEMPER_EVENT_STORE=postgres`
    - `TEMPER_PLATFORM_STORE=postgres`
    - `TEMPER_QUERY_PROJECTION_STORE=postgres`
    - `DATABASE_URL=postgres://postgres:postgres@127.0.0.1:55432/temperpaw`
  - Observed startup log: `Storage: postgres (DATABASE_URL configured)`.
  - `GET /healthz` on `127.0.0.1:3497` returned HTTP 200.
  - Postgres rows after boot:
    - `specs`: 54
    - `tenant_secrets`: 2
    - `_sqlx_migrations`: 1

## Notes

- Verification used the local Temper storage-stack migration worktree as the
  Cargo patch target because OpenPaw now depends on the `StorageStack` and
  Postgres `PlatformStore` work from that branch.
- No real Railway deployment was performed in this proof.
