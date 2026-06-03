# 081 Session Auth, Context, and Cron Recovery

Date: 2026-06-03

## Scope

Investigated and fixed the production failures behind:

- OpenAI Codex 401 `token_revoked` not routing to refresh.
- Session context appearing reset / empty.
- Katagami/CronJob Sessions spawning with empty `user_message`, `model`, or `provider`.
- Provider failures involving unresolved `{secret:openai_api_key}` placeholders.

## Production Evidence

Datadog logs for `service=temperpaw` over the last 7 days showed 5 production `token_revoked` / invalidated OAuth token errors from 2026-06-02T18:35:15Z through 2026-06-03T13:02:56Z.

Datadog error logs over the last 24 hours also showed:

- `context_preparer: session-tree walk from leaf 't-2' returned 0 entries against a non-empty tree...`
- `failed to read events for replay -- starting fresh`
- OpenAI Codex 503 once

Railway production OData inspection showed active Katagami/CronJobs with populated `user_message` but empty `user_message_template`, plus empty `model` / `provider` fields. The pre-fix trigger path rendered only the empty template and spawned Sessions before model/provider defaults survived into the child.

## Fixes

- OpenAI Codex wire detection now treats `token_revoked`, `token_invalidated`, and invalidated OAuth-token 401 bodies as auth-expired outcomes.
- SessionEntry reads synthesize only the known missing virtual initial root shape from `Session.user_message`.
- Cron trigger compute falls back from empty `user_message_template` to existing `CronJob.user_message`.
- Cron trigger compute resolves missing `model` / `provider` from tenant defaults.
- Temper spawn `copy_fields` precedence now preserves explicit callback params over stale copied parent fields.
- paw-agent `app.toml` now declares spec-triggered WASM modules including cron, OpenAI Codex auth, approval handlers, and workspace restoration.
- Provider credential selection skips unresolved `{secret:...}` templates before choosing API-key fallbacks.

## Red/Green Evidence

Red tests observed before implementation:

- `codex_revoked_oauth_token_routes_to_auth_recovery` failed against OpenAI Codex 401 `token_revoked`.
- `session_entries_jsonl_repairs_missing_virtual_initial_root` failed before the helper existed.
- `render_user_message_falls_back_to_existing_message_when_template_empty` and `resolved_cron_session_config_uses_trigger_defaults_for_missing_model_provider` failed before cron helpers existed.
- `spawn_initial_params_keep_explicit_action_params_over_copied_fields` failed before the Temper merge helper existed.
- `paw_agent_manifest_declares_hot_session_wasm_startup_policy` failed on missing `sandbox_provisioner` manifest registration.
- `api_key_resolution_skips_unresolved_secret_templates` failed by selecting `{secret:openai_api_key}` over a real fallback.

Green checks:

- `cargo test` in `openai-codex-wire`: 5 passed.
- `cargo test` in `wasm-helpers`: 35 passed.
- `cargo test` in `cron_compute_next`: 2 passed.
- `cargo test` in `provider_caller`: 27 passed.
- `cargo test -p temperpaw paw_agent_manifest_declares_hot_session_wasm_startup_policy`: passed.
- `cargo test -p temperpaw --test session_turn_architecture`: 22 passed.
- `cargo test -p temperpaw --test datadog_observability_contract`: 32 passed.
- `cargo test -p temperpaw --test paw_fs_hot_path`: 12 passed.
- Targeted paw-patrol schedule-boundary tests: passed.
- `cargo build -p temperpaw`: passed.
- `bash os-apps/paw-agent/wasm/build.sh`: all paw-agent WASM modules built.

Temper platform checks:

- `cargo test -p temper-server spawn_initial_params_keep_explicit_action_params_over_copied_fields`: passed.
- `cargo test -p temper-server test_spawn_with_copy_fields`: passed.
- `cargo test -p temper-jit test_spawn_with_copy_fields_passes_through`: passed.
- `cargo test -p temper-spec parse_spawn_effect`: passed.
- Pre-push hook passed rustfmt, clippy, readability, and many non-Docker tests, then stopped because local Docker daemon was not running for Docker-backed Postgres integration tests.

## Local E2E

Started local server:

`PORT=3797 TEMPER_API_KEY=local-e2e-key PAW_TENANT=local_e2e TEMPERPAW_WASM_STARTUP_POLICY=load-only TURSO_URL=file:/tmp/temperpaw-token-context-rca-e2e.db LLM_PROVIDER=openai_codex LLM_MODEL=gpt-5 OTEL_ENABLED=false cargo run -p temperpaw`

Created a CronJob seeded with:

- `user_message = "Katagami fallback proof: write a short status line and stop."`
- `user_message_template = ""`
- `model = ""`
- `provider = ""`

Activated and manually triggered it. Result:

- CronJob status: `Active`
- CronJob fields after trigger: `model="gpt-5"`, `provider="openai_codex"`, `run_count=1`, `last_session_id=019e8de4-d29f-7d82-be0c-633e98c8e27c`
- Spawned Session fields: matching `user_message`, `model="gpt-5"`, `provider="openai_codex"`, `soul_id="Ren"`, `max_turns="1"`
- No `context_preparer requires Session.model` error.

The local Session later failed only at the expected auth boundary because the fresh local DB had no OpenAI Codex refresh token.

## Deployment Status

Pending at proof creation: commit, push, Railway deploy, and post-deploy production verification.
