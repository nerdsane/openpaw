# 081 Session Auth, Context, and Cron Recovery

Date: 2026-06-03

## Scope

Investigated and fixed the production failures behind:

- OpenAI Codex 401 `token_revoked` not routing to refresh.
- Session context appearing reset / empty.
- Katagami/CronJob Sessions spawning with empty `user_message`, `model`, or `provider`.
- Provider failures involving unresolved `{secret:openai_api_key}` placeholders.
- Active CronJobs whose `schedule_at` timers were lost across deploy because startup recovery populated indexes/projections without hydrating scheduled actors.

## Production Evidence

Datadog logs for `service=temperpaw` over the last 7 days showed 5 production `token_revoked` / invalidated OAuth token errors from 2026-06-02T18:35:15Z through 2026-06-03T13:02:56Z.

Datadog error logs over the last 24 hours also showed:

- `context_preparer: session-tree walk from leaf 't-2' returned 0 entries against a non-empty tree...`
- `failed to read events for replay -- starting fresh`
- `failed to persist trajectory entry from outbox`
- OpenAI Codex 503 once

Additional Datadog inspection after the `sha-4d64e800` deployment found 4
trajectory persistence failures between 2026-06-03T15:20:24Z and
2026-06-03T16:01:17Z. The latest production-tagged event on
`version=sha-4d64e800` failed persisting `Session.ContextReady` with:

`EOF while parsing a string at line 1 column 4096`

Root cause: Temper's trajectory storage adapter serialized request-body JSON and
then sliced the serialized string at 4096 bytes, which could produce invalid JSON
before inserting into Postgres `JSONB`.

Railway production OData inspection showed active Katagami/CronJobs with populated `user_message` but empty `user_message_template`, plus empty `model` / `provider` fields. The pre-fix trigger path rendered only the empty template and spawned Sessions before model/provider defaults survived into the child.

After the first Railway image deploy, production still invoked stale Genesis-pinned paw-agent artifacts:

- Railway `/paw/version`: `sha-a27f4977` / `a27f49775b645bed9be69acb727dd2b29dbb74ef`
- Production `cron_compute_next` invocation hash before live repair: `a54a6c4c...` / old behavior.
- Fixed local `cron_compute_next` hash after OData casing repair: `7831f6a70c7bdd6f7b97dd933eac62870d4f8de64beede71cb7ce222de2b0092`.
- `TEMPERPAW_GENESIS_BOOTSTRAP_REFS` pinned `temperpaw/paw-agent@93677c779776a17089c2ee0ccc65e0650b1f6688`, so container replacement alone did not replace live app spec/WASM bytes.

## Fixes

- OpenAI Codex wire detection now treats `token_revoked`, `token_invalidated`, and invalidated OAuth-token 401 bodies as auth-expired outcomes.
- SessionEntry reads synthesize only the known missing virtual initial root shape from `Session.user_message`.
- Cron trigger compute falls back from empty `user_message_template` to existing `CronJob.user_message`.
- Cron trigger compute reads both snake_case and PascalCase OData message fields (`user_message` / `UserMessage`).
- Cron trigger compute resolves missing `model` / `provider` from tenant defaults.
- CronJob carries explicit defaults for the optional `Session.Configure` surface and copies them during declarative spawn, satisfying production spec lint before hot-loading.
- Temper spawn `copy_fields` precedence now preserves explicit callback params over stale copied parent fields.
- Temper trajectory request-body truncation now stores either the original small JSON
  or a valid `_temper_truncated` envelope under 4096 bytes.
- Temper startup schedule recovery now scans only entity types whose specs declare `schedule_at`, force-hydrates those scheduled actors, and re-arms timers using the hydrated current-state event history. Recovered timers are deduped by tenant/entity/action/sequence.
- TemperPaw startup runs that schedule recovery phase before marking `/readyz` ready.
- paw-agent `app.toml` now declares spec-triggered WASM modules including cron, OpenAI Codex auth, approval handlers, and workspace restoration.
- Provider credential selection skips unresolved `{secret:...}` templates before choosing API-key fallbacks.

## Red/Green Evidence

Red tests observed before implementation:

- `codex_revoked_oauth_token_routes_to_auth_recovery` failed against OpenAI Codex 401 `token_revoked`.
- `session_entries_jsonl_repairs_missing_virtual_initial_root` failed before the helper existed.
- `render_user_message_falls_back_to_existing_message_when_template_empty` and `resolved_cron_session_config_uses_trigger_defaults_for_missing_model_provider` failed before cron helpers existed.
- `render_user_message_reads_camel_case_cron_fields` failed before the OData casing fallback existed.
- `spawn_initial_params_keep_explicit_action_params_over_copied_fields` failed before the Temper merge helper existed.
- `trajectory_request_body_truncation_preserves_valid_json` failed with
  `EOF while parsing a string` at column 4096 before Temper's truncation envelope.
- `startup_schedule_at_recovery_rearms_timer_types_without_full_hydration` failed to compile before Temper exposed startup `schedule_at` recovery.
- `schedule_at_recovery_runs_before_startup_readiness` was added to lock the TemperPaw startup ordering contract.
- `paw_agent_manifest_declares_hot_session_wasm_startup_policy` failed on missing `sandbox_provisioner` manifest registration.
- `paw_agent_specs_map_cron_spawn_session_config` failed with `spawn_initial_action_params_unmapped` before CronJob declared and copied the optional Session config fields.
- `api_key_resolution_skips_unresolved_secret_templates` failed by selecting `{secret:openai_api_key}` over a real fallback.

Green checks:

- `cargo test` in `openai-codex-wire`: 5 passed.
- `cargo test` in `wasm-helpers`: 35 passed.
- `cargo test` in `cron_compute_next`: 3 passed.
- `cargo test` in `provider_caller`: 27 passed.
- `cargo test -p temperpaw paw_agent_manifest_declares_hot_session_wasm_startup_policy`: passed.
- `cargo test -p temperpaw paw_agent_specs_map_cron_spawn_session_config`: passed.
- `cargo test -p temperpaw --test session_turn_architecture`: 22 passed.
- `cargo test -p temperpaw --test datadog_observability_contract`: 32 passed.
- `cargo test -p temperpaw --test paw_fs_hot_path`: 12 passed.
- `cargo test -p temperpaw schedule_at_recovery_runs_before_startup_readiness`: passed.
- Targeted paw-patrol schedule-boundary tests: passed.
- `cargo build -p temperpaw`: passed.
- `bash os-apps/paw-agent/wasm/build.sh`: all paw-agent WASM modules built.

Temper platform checks:

- `cargo test -p temper-server spawn_initial_params_keep_explicit_action_params_over_copied_fields`: passed.
- `cargo test -p temper-server test_spawn_with_copy_fields`: passed.
- `cargo test -p temper-server trajectory_request_body`: 2 passed.
- `cargo test -p temper-server schedule_at -- --nocapture`: passed, including startup `schedule_at` recovery.
- `cargo test -p temper-server --test schedule_at_hydration -- --nocapture`: 4 passed.
- `cargo test -p temper-jit test_spawn_with_copy_fields_passes_through`: passed.
- `cargo test -p temper-spec parse_spawn_effect`: passed.
- `cargo build -p temper-server`: passed.
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

## Production Deployment and Repair

Built and deployed GHCR image:

- GitHub Actions Docker run: `26891748201`
- Image: `ghcr.io/nerdsane/temperpaw:sha-a27f497`
- Digest: `sha256:5c3eb935c07956893d30f9ee0e0cf9298dc0bf98e05397e075c8805161ffa8e0`
- Railway deployment: `e2f4372d-e79e-4c0f-a61a-11444c7a7212`
- Railway `/readyz`: 200 after startup
- `/paw/version`: `{"version":"sha-a27f4977","sha":"a27f49775b645bed9be69acb727dd2b29dbb74ef"}`

GitHub Railway redeploy workflow dispatch failed before touching Railway because the GitHub environment did not have required Railway/TEMPER secrets. Deployment was completed via the linked Railway CLI with `IMAGE_TAG=sha-a27f497`.

Because production was Genesis-pinned to stale paw-agent app bytes, live app artifact repair was also required:

- Hot-uploaded fixed paw-agent WASM modules through `/api/wasm/modules/{module}`.
- Confirmed production accepted `cron_compute_next` hash `7831f6a70c7bdd6f7b97dd933eac62870d4f8de64beede71cb7ce222de2b0092`.
- Submitted the corrected paw-agent spec bundle through `/api/specs/load-inline`.
- Spec submission returned HTTP 200 and summary `all_passed=true` for Agent, App, CronJob, Memory, OpenaiCodexAuth, PlanReview, Project, Session, SessionEntry, SessionLink, Soul, Team, and ToolHook.

## Production E2E

Initial post-image proof `CronJob:cron-prod-token-context-proof-1780499143` failed with empty spawned Session fields. This proved the live app artifacts were still stale even though the container SHA changed.

After hot-uploading the casing-aware `cron_compute_next` module but before spec load, proof `CronJob:cron-prod-token-context-proof-1780499802` emitted `TriggerFailed` with `CronJob model is empty; configure model or tenant llm_model`. This proved the stale live spec lacked trigger defaults.

An explicit-model proof `CronJob:cron-prod-explicit-model-proof-1780500017` passed, proving declarative spawn itself still worked when model/provider existed in CronJob state:

- Spawned `Session:019e8e12-3e0e-7ba3-a369-312805b9a1d3`
- Session received matching `user_message`, `model`, and `provider`
- Proof Session was cancelled and proof CronJob paused

Patched the active broken Katagami CronJob in entity state:

- `CronJob:cj-019e3cf6-d6bd-7a32-a167-4254878eaf3a`
- Before: Active, empty `user_message`, empty `model`, empty `provider`
- Restored message from sibling `CronJob:cj-019e3cf4-1969-7203-9d64-fd649515eddf`
- After: Active, `user_message` length 897, `UserMessage` length 897, model/provider set, next run scheduled for `2026-06-03T15:33:41Z`

Final no-model/no-provider production proof passed after live spec load:

- `CronJob:cron-prod-token-context-proof-1780500519`
- CronJob trigger filled `model="gpt-5.5"`, `provider="openai_codex"`, `run_count=1`
- Spawned `Session:019e8e19-e593-7dd2-89d9-3e04f11ac6d7`
- Session reached `PreparingContext` with matching `user_message`, non-empty model/provider, and `tools_enabled="read"`
- Proof Session was cancelled and proof CronJob paused

After the second image deployment:

- GitHub Actions Docker run: `26895248992`
- Image: `ghcr.io/nerdsane/temperpaw:sha-4d64e80`
- Digest: `sha256:10438073ba17c7fd23d1bcc5dde29248562e690a2b8de5edbe999def2bd13389`
- Railway deployment: `376af58e-1696-41a1-8a87-e0e1f900340d`
- `/readyz`: 200
- `/healthz`: 200
- `/paw/version`: `{"version":"sha-4d64e800","sha":"4d64e800adba7c736dc5b65ebf4570e57dc401b9"}`

Production no-model/no-provider smoke on `sha-4d64e800` passed:

- `CronJob:cron-prod-token-context-proof-1780502469`
- Spawned `Session:019e8e37-a7a8-7520-9e17-5cbacc9c12aa`
- CronJob message lengths: `user_message=99`, `UserMessage=99`
- Session received matching `user_message`, `model="gpt-5.5"`, `provider="openai_codex"`, and `tools_enabled="read"`
- Proof Session was cancelled and proof CronJob paused

Active Katagami job after repair:

- `CronJob:cj-019e3cf6-d6bd-7a32-a167-4254878eaf3a`
- Status: `Active`
- `run_count=74`
- `user_message` length 897, `UserMessage` length 897
- `model="gpt-5.5"`, `provider="openai_codex"`
- Latest Session `019e8e30-bc5e-7591-b6e9-2d2c5f2e00ed` received the 897-byte message and model/provider, then failed only at the explicit graceful auth boundary:
  `OpenAI Codex sign-in is required; start the Codex device login again.`

Subsequent deploy verification found one remaining scheduler durability issue:

- Railway deployment `d1bb22e9-dad0-4152-92b0-d54e512da279` served `sha-cb3148da` / `cb3148dae6adcb955d1bea196c8bb39085a3f095`.
- `/readyz` and `/healthz` were 200.
- The active Katagami CronJob still showed `run_count=74` and stale `next_run_at="2026-06-03T16:03:39Z"` after deploy.
- Root cause: Temper's first scheduler recovery fix rearmed timers when an actor hydrated, but production startup only populated the query/index catalog. The scheduled CronJob actor stayed cold, so its in-memory `schedule_at` timer was never recreated.

## Final Production Deploy

Pinned Temper to `0418ddc30a6c3e362401a58c91c605e8d50b34c1`, which adds startup recovery for persisted `schedule_at` timers even when actors have not been hydrated by ordinary reads.

- TemperPaw commit: `7ccf62264c3c93bd3f5b09c0bab88cac2743d758`
- Image: `ghcr.io/nerdsane/temperpaw:sha-7ccf622`
- Image digest: `sha256:626a8dba549bfacea07201fd2c1c04394c56aa69ce5bfe81586f26c49d843753`
- GitHub Actions Docker run: `26902834433`
- GitHub Actions job: `79359578785`, success in `24m31s`
- Railway deployment: `d37ff10b-f5a0-470f-b843-1b9d1834c9e7`
- `/paw/version`: `{"version":"sha-7ccf6226","sha":"7ccf62264c3c93bd3f5b09c0bab88cac2743d758"}`
- `/readyz`: 200, Discord connected
- `/healthz`: 200

## Final Production E2E

The previously stale active Katagami CronJob recovered on startup:

- `CronJob:cj-019e3cf6-d6bd-7a32-a167-4254878eaf3a`
- Status: `Active`
- `run_count` advanced from `74` to `75`
- `next_run_at` advanced from stale `2026-06-03T16:03:39Z` to future `2026-06-03T18:28:36Z`
- New `last_session_id`: `019e8eb5-716f-7221-a1b8-94d3752f01c1`
- CronJob still carries `user_message` length `897`, `model="gpt-5.5"`, and `provider="openai_codex"`
- Spawned Session `019e8eb5-716f-7221-a1b8-94d3752f01c1` received `user_message` length `897`, `model="gpt-5.5"`, and `provider="openai_codex"`
- The Session failed only at the explicit graceful auth boundary:
  `OpenAI Codex auth failed. OpenAI Codex sign-in is required; start the Codex device login again.`

Disposable production CronJob smoke also passed:

- `CronJob:cron-prod-final-schedule-proof-1780510834`
- Seeded with `user_message` length `104`, empty `user_message_template`, empty `model`, and empty `provider`
- Manual `Trigger` filled `model="gpt-5.5"` and `provider="openai_codex"`
- Spawned `Session:019e8eb7-498a-73d3-afeb-724677eca03b`
- Session received matching `user_message` length `104`, `model="gpt-5.5"`, `provider="openai_codex"`, and `tools_enabled="read"`
- Proof Session was cancelled and proof CronJob paused.

Datadog post-deploy checks from `2026-06-03T18:17:57Z`:

- `service:temperpaw` logs for `sha-7ccf6226`: `4884` info, `244` warn, `0` error.
- Scheduler recovery evidence present at `2026-06-03T18:18:35Z-18:18:36Z`:
  `schedule_at timers re-armed from hydrated state`, `phase_9_schedule_at_recovery complete`, and `schedule_at timer recovery complete`.
- No post-deploy logs matched `failed to persist trajectory entry from outbox`, `token_revoked`, `OpenAI Codex auth failed`, or `OpenAI Codex sign-in is required`.
- Last seven days `token_revoked` count: `5`; first seen `2026-06-02T18:35:15.892Z`, last seen `2026-06-03T13:02:56.128Z`.
