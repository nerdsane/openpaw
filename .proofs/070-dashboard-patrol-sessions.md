# Dashboard Patrol + Sessions Verification

Date: 2026-05-08
Branch: `codex/dashboard-patrol-sessions`

## Scope

- Made the Paw Patrol app console readable across Patrol entity sets.
- Made generic entity detail pages show linked IDs, event timelines, readable JSON, and Patrol proof fields.
- Reworked the Sessions page into a chronological ledger with active/completed/failed filters.
- Fixed dashboard action dispatch to use the live `@odata.actions` target instead of a hard-coded namespace.

## Static Verification

- `cargo fmt --check`
- `git diff --check`
- `cargo test -p temperpaw --test paw_patrol_foundation dashboard_has_generic_app_console_and_paw_patrol_view_manifest -- --nocapture`
- `npm --prefix dashboard run check`
- `npm --prefix dashboard run build`

## Live Local Verification

Started a local TemperPaw backend on `127.0.0.1:3467` with:

- isolated Turso database: `/tmp/temperpaw-dashboard-live.db`
- isolated home directory: `/tmp/temperpaw-dashboard-live-home`
- Discord and Slack env vars unset
- local dashboard account: `dashboard-smoke@example.test`

Built required local WASM artifacts so startup reconciled the real app specs. Seeded a Patrol smoke story through authorized dashboard-session actions:

- `Signal` -> `FactoryCase` -> `WorkCycle`
- `WorkerRun`, `ReviewRun`, `EvaluationRun`
- `ProofPacket`
- `PatrolRun`
- `ObservabilityFinding`
- three `Session` rows for completed, failed, and active chronology checks

Browser smoke was run with headless Chrome/Playwright against the live local dashboard:

- `/dashboard/apps/paw-patrol`
- `/dashboard/sessions`
- `/dashboard/entities/WorkCycles/wc-019e07b9-881d-77b0-b8b5-f3008019d032`
- `/dashboard/entities/ProofPackets/en-019e07b9-897f-7632-a615-6641aad21e9b`

Result:

```text
browser-smoke-ok: patrol, sessions, workcycle detail, proof detail
```

Screenshots were captured locally under `/tmp/temperpaw-dashboard-*.png`.
