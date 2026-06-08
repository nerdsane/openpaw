# 2026-06-08 OpenAI Codex Auth Gate and Katagami Finalizer Proof

## Incident

Production CurationJobs failed in two independent ways:

- OpenAI Codex provider sessions failed while auth was `Refreshing`.
- Katagami quality finalization attempted `DesignLanguage.MarkQualityPassed`
  while a reviewed language was still `Draft`.

## Source-of-Truth Findings

- Live Railway logs reported `sha-3e4e699e`, matching TemperPaw GitHub
  `origin/main` before this fix.
- TemperPaw deploy image source is GitHub `main` -> GHCR -> Railway.
- Katagami app folders are synced through Genesis app repos, but the TemperPaw
  Docker image bakes them from GitHub Katagami at `KATAGAMI_REF`.
- Katagami GitHub `master` now contains fix commit
  `7b897e1a1af96ff61e7c15a201da41546570a43e`.
- Genesis app repos were synced from the same Katagami app folders:
  - `katagami-commons` -> `9fb0622f3caa7a6d885856f233e42410fc19f3fc`
  - `katagami-curation` -> `a06d07772958fb67827e8ed69a431b039b6453da`

## Code Changes

- `provider_auth_gate` preflights `/paw/setup/openai-codex/status`.
- `Ready` and configured `Refreshing` skip `EnsureFresh`.
- Human-login states fail with Discord `codex auth` device-login guidance.
- Katagami quality finalization now submits Draft languages for review before
  `MarkQualityPassed`, then publishes from `UnderReview`.
- TemperPaw Docker `KATAGAMI_REF` now pins Katagami `7b897e1...`.

## Verification

- Red tests failed before implementation:
  - `provider_auth_gate` missing configured-Refreshing helpers.
  - Katagami finalizer contract missing `ensure_language_under_review`.
- Green checks:
  - `cargo test --manifest-path os-apps/paw-agent/wasm/provider_auth_gate/Cargo.toml --lib`
  - `cargo test -p temperpaw --test session_lifecycle_and_config paw_agent_defines_temper_native_openai_codex_auth_entity`
  - `bash os-apps/paw-agent/wasm/build.sh`
  - `python3 -m unittest discover -s katagami-curation/tests`
  - `cargo test --manifest-path katagami-curation/wasm/finalize_spawned_session/Cargo.toml --lib`
  - `bash katagami-curation/wasm/build.sh`

## Deployment

Pending TemperPaw GitHub main push, GHCR image build, and Railway redeploy.
