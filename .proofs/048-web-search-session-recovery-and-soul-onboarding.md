# Proof: web search/session recovery fixes, `paw-research` core install, and deployed soul onboarding

Date: 2026-04-15

## Scope

This proof covers:

- web search/fetch recovery behavior in the Paw agent tool layer
- backward-compatible session and memory tool argument handling
- making `paw-research` a core startup app so `WebQueries` exists on fresh boots
- deployed soul onboarding in the dashboard
- personalized Paw soul persistence across restart
- local Datadog env defaulting to `local`
- Modal bridge configuration requiring an explicit bridge URL

## Published dependency note

The supporting Temper fix for stable remote consumption was pushed to:

- repo: `https://github.com/nerdsane/temper.git`
- branch: `codex/fix-odata-entity-filtering`
- commit: `9f8696f4577e63f19db22f4682cdba6a8165d44c`

Direct push to Temper `main` was blocked by GitHub protected-branch rules, so OpenPaw is pinned to that exact remote commit by `rev`.

## Rust verification

Ran against `/tmp/openpaw-fix` after removing the local Temper patch override:

```bash
cargo update --manifest-path /tmp/openpaw-fix/Cargo.toml
cargo test -p openpaw --manifest-path /tmp/openpaw-fix/Cargo.toml
cargo test --manifest-path /tmp/openpaw-fix/os-apps/paw-agent/wasm/monty_repl/Cargo.toml
cargo test --manifest-path /tmp/openpaw-fix/os-apps/paw-agent/wasm/llm_caller/Cargo.toml
cargo test --manifest-path /tmp/openpaw-fix/os-apps/paw-agent/wasm/wasm-helpers/Cargo.toml
cargo test --manifest-path /tmp/openpaw-fix/os-apps/paw-research/wasm/web_fetch/Cargo.toml
```

Observed results:

- `openpaw`: 25 passed, 0 failed
- `monty_repl`: 15 passed, 0 failed
- `llm_caller`: 15 passed, 0 failed
- `wasm-helpers`: 12 passed, 0 failed
- `web_fetch`: 13 passed, 0 failed

The `openpaw` suite included:

- `startup_os_apps_only_include_core_apps`
- `bootstrap_soul_preserves_existing_personalized_paw_content`
- `paw_soul_content_personalization_detection_matches_non_default_content`
- `safe_setup_metadata_routes_are_public_only_before_first_account`
- `setup_secret_routes_require_auth_even_before_first_account`
- `ensure_dd_env_defaults_to_local_without_override`

## Dashboard verification

Ran:

```bash
cd /tmp/openpaw-fix/dashboard
npm run check
npm run build
```

Observed results:

- `svelte-check found 0 errors and 0 warnings`
- production build completed successfully

## End-to-end runtime proof

Fresh HOME:

```bash
/tmp/openpaw-e2e-remote.B3rzxK
```

Started server:

```bash
HOME=/tmp/openpaw-e2e-remote.B3rzxK \
PORT=3417 \
RUSTUP_HOME=/Users/seshendranalla/.rustup \
CARGO_HOME=/Users/seshendranalla/.cargo \
/tmp/openpaw-fix/target/debug/openpaw-server
```

### Fresh boot status

Anonymous status before auth:

```bash
curl -s http://127.0.0.1:3417/paw/setup/status
```

Observed:

```json
{"has_anthropic_key":false,"llm_provider":null,"has_discord":false,"has_slack":false,"has_agents":true,"agent_count":4,"has_personalized_soul":false,"discord_connected":false,"slack_connected":false,"discord_interaction_url":null}
```

### `paw-research` is installed on fresh boot

Registered first user:

```bash
curl -s -c /tmp/openpaw-e2e-remote.cookie \
  -H 'content-type: application/json' \
  -d '{"email":"remoteproof@example.com","password":"pass123456"}' \
  http://127.0.0.1:3417/auth/register
```

Queried installed apps:

```bash
curl -s -b /tmp/openpaw-e2e-remote.cookie \
  'http://127.0.0.1:3417/tdata/Apps?$top=20'
```

Observed app names included:

- `paw-agent`
- `paw-fs`
- `paw-channels`
- `paw-research`

Server startup logs also showed:

```text
Startup OS app surface resolved from manifests apps=["paw-agent", "paw-channels", "paw-fs", "paw-research"]
```

### `WebQueries` metadata is present

Queried metadata:

```bash
curl -s -b /tmp/openpaw-e2e-remote.cookie \
  'http://127.0.0.1:3417/tdata/$metadata' | rg -n 'WebQueries|WebQuery'
```

Observed:

```text
<EntityType Name="WebQuery">
<EntitySet Name="WebQueries" EntityType="OpenPaw.Research.WebQuery"/>
```

### Personalized soul save and restart persistence

Saved personalized soul:

```bash
curl -s -b /tmp/openpaw-e2e-remote.cookie \
  -H 'content-type: application/json' \
  --data @/tmp/openpaw-save-soul-final.json \
  http://127.0.0.1:3417/paw/setup/soul/save
```

Observed:

```json
{"saved":true}
```

After save:

```bash
curl -s -b /tmp/openpaw-e2e-remote.cookie http://127.0.0.1:3417/paw/setup/status
curl -s -b /tmp/openpaw-e2e-remote.cookie http://127.0.0.1:3417/paw/setup/soul
```

Observed:

- `has_personalized_soul: true`
- soul summary: `I am tailored for Arni and keep the focus on shipping.`

Restarted the same server with the same HOME, then re-queried:

```bash
curl -s -b /tmp/openpaw-e2e-remote.cookie http://127.0.0.1:3417/paw/setup/status
curl -s -b /tmp/openpaw-e2e-remote.cookie http://127.0.0.1:3417/paw/setup/soul
```

Observed after restart:

- `has_personalized_soul: true`
- the personalized summary was still `I am tailored for Arni and keep the focus on shipping.`

## Behavioral fixes covered by tests

The targeted tests above prove:

- vague follow-up web search queries are rewritten once using recent context
- zero-result web searches are no longer reported as generic global web outage
- blank/empty fetches become explicit errors
- `recall_memory`, `save_memory`, `list_sessions`, `spawn_session`, `steer_session`, and `abort_session` accept the legacy positional call style the agent prompt had been teaching
- Modal sandbox provisioning no longer fabricates a bogus default bridge URL
- local runs default `DD_ENV=local` when unset

## Conclusion

The fixes are verified against the real fetched Temper dependency, and the deployed-style runtime path now shows:

- `paw-research` auto-installs on fresh boot
- `WebQueries` exists in metadata
- personalized soul onboarding is reflected in setup status
- the personalized Paw soul survives restart
