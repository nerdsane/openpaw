# Proof 075: WorkCycle Codex Plan Mode Plans

Date: 2026-05-12

## Scope

- Replace thin Patrol WorkCycle plans with structured plan-mode markdown.
- Add a revisable WorkCycle plan path for future local Codex worker runs.
- Update the existing Discord typing-indicator WorkCycle plan:
  `wc-019e0a9c-5148-7241-a558-8b2c3c66a618`.

## Red

Added `patrol_work_cycles_have_revisable_codex_plan_mode_plans` in
`crates/temperpaw/tests/paw_patrol_foundation.rs`.

Initial targeted run failed before implementation because `WorkCycle` did not
define `plan_revision_count` or `RevisePlan`.

## Green

Implemented:

- `WorkCycle.RevisePlan` action from `Planned`, `AwaitingHumanStartApproval`,
  and `InProgress`.
- `plan_revision_count` counter and CSDL exposure.
- Cedar permit for system agents to revise plans.
- Structured initial plan markdown in Patrol intake/lifecycle WASM modules.
- Local Codex worker read-only plan pass before mutating implementation.
- Worker plan output dispatch to `WorkCycle.RevisePlan`.
- Active plan injection into the mutating Codex prompt.
- Dashboard entity primary-field ordering for `plan_summary` and
  `plan_revision_count`.
- ADR:
  `os-apps/paw-patrol/adrs/0004-workcycle-codex-plan-mode-contract.md`.

## Verification

Commands run:

```text
cargo test -p temperpaw --test paw_patrol_foundation patrol_work_cycles_have_revisable_codex_plan_mode_plans -- --nocapture
cargo test -p temperpaw --test paw_patrol_foundation -- --nocapture
cargo test -p paw-codex-worker -- --nocapture
cargo check --manifest-path os-apps/paw-patrol/wasm/patrol_request_router/Cargo.toml
cargo check --manifest-path os-apps/paw-patrol/wasm/signal_router/Cargo.toml
cargo check --manifest-path os-apps/paw-patrol/wasm/finding_lifecycle/Cargo.toml
cargo check --manifest-path os-apps/paw-patrol/wasm/repo_sweep_lifecycle/Cargo.toml
cargo check --manifest-path os-apps/paw-patrol/wasm/daily_brief_lifecycle/Cargo.toml
cargo check --manifest-path os-apps/paw-patrol/wasm/patrol_run_lifecycle/Cargo.toml
npm ci
npm run check
cargo check --workspace
cargo check -p paw-codex-worker
RUSTUP_TOOLCHAIN=nightly bash os-apps/paw-fs/wasm/blob_adapter/build.sh
RUSTUP_TOOLCHAIN=nightly bash os-apps/paw-fs/wasm/workspace_fs/build.sh
RUSTUP_TOOLCHAIN=nightly bash os-apps/paw-agent/wasm/build.sh
RUSTUP_TOOLCHAIN=nightly bash os-apps/paw-research/wasm/build.sh
RUSTUP_TOOLCHAIN=nightly bash os-apps/paw-channels/wasm/build.sh
RUSTUP_TOOLCHAIN=nightly bash os-apps/paw-ingest/wasm/build.sh
RUSTUP_TOOLCHAIN=nightly bash os-apps/paw-patrol/wasm/build.sh
RUSTUP_TOOLCHAIN=nightly bash os-apps/paw-skills/wasm/build.sh
```

Results:

- Patrol foundation: 55 passed.
- Paw Codex worker: 50 passed.
- Dashboard `svelte-check`: 0 errors, 0 warnings.
- Six changed Patrol WASM crates checked successfully.
- Full workspace check completed successfully.
- `npm ci` reported 4 dependency audit findings in the existing dashboard
  dependency tree: 1 low, 1 moderate, 2 high. No dependency versions were
  changed.
- A throwaway local server booted from the clean worktree after required WASM
  artifacts were built.

## Live Entity Update

OData read after update confirmed the existing WorkCycle plan begins:

```text
# WorkCycle Plan

## Context
Discord DM typing is unreliable for Paw responses. The request says typing should start promptly when a human sends a DM, renew continuously while the response is pending, and stop only after final response, explicit failure, cancellation, or confirmed dead worker.
```

The live server currently advertises only `ApproveHumanStart` and `Fail` for
`wc-019e0a9c-5148-7241-a558-8b2c3c66a618`; it predates the new `RevisePlan`
action. `WritePlan` is also invalid from `AwaitingHumanStartApproval`. For this
one existing record, the plan was updated with an admin OData `PATCH`.

Future deployed WorkCycles should use the new entity action:
`WorkCycle.RevisePlan`.

## Local E2E

Booted a fresh server from the clean worktree with an isolated home and Turso
database:

```text
HOME=/tmp/temperpaw-plan-mode-e2e-home
PORT=4577
TURSO_URL=file:/tmp/temperpaw-plan-mode-e2e.db
TEMPER_API_KEY=plan-mode-e2e
OTEL_ENABLED=false
TEMPERPAW_WASM_STARTUP_POLICY=warn
TEMPERPAW_QUERY_PROJECTION_BACKFILL_ON_STARTUP=0
RUST_MIN_STACK=16777216
RUSTUP_TOOLCHAIN=nightly
cargo run -p temperpaw --bin temperpaw-server
```

Observed:

```text
Temper Paw is running.
API:       http://localhost:4577/tdata
Dashboard: http://localhost:4577/dashboard
```

Then exercised the entity path over OData:

```text
POST /WorkCycles -> 201 Created
POST /WorkCycles('{id}')/TemperPaw.Patrol.Configure -> 200 OK
POST /WorkCycles('{id}')/TemperPaw.Patrol.WritePlan -> 200 OK
POST /WorkCycles('{id}')/TemperPaw.Patrol.RequestHumanStartApproval -> 200 OK
GET  /WorkCycles('{id}') -> status=AwaitingHumanStartApproval, actions include RevisePlan
POST /WorkCycles('{id}')/TemperPaw.Patrol.RevisePlan -> 200 OK
GET  /WorkCycles('{id}') -> status=AwaitingHumanStartApproval, plan_revision_count=1
```

The revised `plan_summary` contained `## Codex Plan Mode`, proving the new
state-machine action updates the visible plan without leaving
`AwaitingHumanStartApproval`.

## Dashboard Check

- API read confirmed `plan_summary` contains the structured plan.
- Browser verification of the exact entity route was blocked by the current
  dashboard setup guard redirecting this browser profile to
  `/dashboard/welcome` because setup is 3 of 4 steps complete.
- Dashboard code validation passed through `npm run check`, and the entity
  detail view now prioritizes `plan_summary` and `plan_revision_count`.

## Residual Risk

- The running local/edge server must be restarted or redeployed before
  `RevisePlan` and `plan_revision_count` are available on live WorkCycles.
- The setup-guard redirect should be considered during the next dashboard pass
  if operators need to inspect entities before all first-run setup steps are
  complete.
