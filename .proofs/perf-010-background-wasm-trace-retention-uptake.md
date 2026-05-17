# Proof Report: PERF-010 — Background WASM Trace Retention Uptake

## Date

2026-05-17

## Branch / Commit

- Branch: `codex/bump-temper-wasm-trace-retention-20260517`
- Base: `origin/main` at `32b29ba72de447c37c6ab31a2bdd0912b932ce7d`
- Temper revision adopted: `ed3d0c6678f528bd5031dc79fb4ae7628a599fe9`
- Local commit: pending at proof creation

## What Was Done

TemperPaw now pins Temper crates and packaged WASM SDK dependencies to Temper
commit `ed3d0c6678f528bd5031dc79fb4ae7628a599fe9`, which contains Temper
ADR-0098 and removes `dispatch.background_wasm_integrations` from the reduced
background trace sampler. This is a dependency uptake and deployment packaging
slice, not a new TemperPaw architecture decision; the architecture decision
lives in Temper as `docs/adrs/0098-background-wasm-trace-retention.md`.

Updated:

- top-level TemperPaw `Cargo.toml` / `Cargo.lock`
- nested OS-app WASM `Cargo.toml` / checked-in `Cargo.lock` files that pin
  `temper-wasm-sdk`
- `Dockerfile` `TEMPER_OBSERVABILITY_REV`
- `datadog_observability_contract` expected Temper revision

## Verification Flow

| Step | Expected | Actual | Status |
|------|----------|--------|--------|
| Stale revision scan | No references to prior Temper rev `0bfaa4ce57c8a0a0c619bf8ece3c9f1fdff8814a` remain in shipped dependency surfaces | `rg` over `os-apps`, top-level lock/manifests, `Dockerfile`, `crates`, `.github`, `scripts`, `deploy`, and `docs` returned no matches | PASS |
| Datadog observability contract | Contract expects and accepts Temper rev `ed3d0c6678f528bd5031dc79fb4ae7628a599fe9` | `cargo test --locked -p temperpaw --test datadog_observability_contract -- --nocapture` passed `31/31` | PASS |
| Package check | TemperPaw and worker compile with locked dependencies | `cargo check --locked -p temperpaw -p paw-codex-worker` passed | PASS |
| Formatting | No rustfmt drift | `cargo fmt --all -- --check` passed | PASS |
| Whitespace | No patch whitespace errors | `git diff --check` passed | PASS |
| Worker script syntax/runtime smoke | CI script syntax and smoke checks pass | all `bash -n` checks, `ci-actions-runtime-smoke.sh`, and `production-git-ancestry-guard-smoke.sh` passed | PASS |
| Clippy | No warnings for shipped Rust packages | `cargo clippy --locked -p temperpaw -p paw-codex-worker --all-targets -- -D warnings` passed | PASS |
| Package tests | TemperPaw, worker, and review-gate tests pass | `cargo test --locked -p temperpaw --quiet`, `cargo test --locked -p paw-codex-worker --quiet`, and `cargo test --manifest-path os-apps/paw-patrol/wasm/review_gate_lifecycle/Cargo.toml --quiet` passed | PASS |
| OS-app WASM build | Packaged WASM modules rebuild against `ed3d0c66` | CI WASM build loop passed for paw-agent, paw-channels, paw-fs blob adapter/workspace_fs, paw-ingest, paw-managed-agents, paw-skills, paw-research, and paw-patrol | PASS |
| Dashboard build | Dashboard production build passes after installing CI dependencies | `npm install` then `npm run build` passed in `dashboard/`; `npm install` reported existing audit findings but no dashboard dependency files changed | PASS |

## What Worked

- The top-level Rust graph resolves Temper crates from `ed3d0c66`.
- Nested WASM builds resolve `temper-wasm-sdk` from `ed3d0c66`, which is the key
  packaging risk for this rollout.
- The Datadog observability contract now guards this exact revision so a future
  accidental downgrade should fail locally and in CI.

## What Didn't Work

- `npm run build` failed before dependency installation because `vite` was not
  present in the fresh worktree. This matches CI's explicit dependency install
  step, so `npm install` was run and the subsequent build passed.

## Limitations

- This proof is local/packaging proof only. It does not prove production traces
  yet.
- The live acceptance criterion remains: after merge, Docker publish, and
  Railway deploy, a mock Session trace must retain
  `dispatch.background_wasm_integrations`, the relevant `wasm:<module>` spans,
  and `dispatch.dispatch_wasm_callback`.
- Existing `npm audit` findings were surfaced by install but are unrelated to
  this dependency-revision uptake because dashboard dependencies were not
  changed.

## What Still Doesn't Work

- Production is still running the previous TemperPaw image at proof creation.
- Datadog live trace evidence for the retained WASM subtree still needs the
  TemperPaw PR, merge, Docker publication, Railway deployment, and live e2e
  Session proof.

## Artifacts

- Temper ADR: `/Users/seshendranalla/Development/temper-worktrees/latency-integration-wakeup-20260517/docs/adrs/0098-background-wasm-trace-retention.md`
- Living dashboard: `/Users/seshendranalla/Development/temper-worktrees/latency-session-projection-shape-20260517/docs/temper-latency-observability-report.html`
- Browser-synced dashboard copy: `/Users/seshendranalla/Development/temper-worktrees/latency-observability-auth-timeout-20260515-1324/docs/temper-latency-observability-report.html`

## Architecture Diagram

```text
Temper ADR-0098 / sampler fix
  |
  v
Temper commit ed3d0c66
  |
  +--> TemperPaw server crates pin ed3d0c66
  |
  +--> Dockerfile TEMPER_OBSERVABILITY_REV pins ed3d0c66
  |
  +--> packaged OS-app WASM SDK pins ed3d0c66
         |
         v
Railway deployment after merge
  |
  v
Datadog Session trace should retain:
  dispatch.background_wasm_integrations
  wasm:<module>
  dispatch.dispatch_wasm_callback
```
