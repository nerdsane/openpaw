# Proof 059: OpenAI Codex Subscription Auth Parity

Date: 2026-04-25

## Scope

Implemented OpenAI Codex subscription auth as a Temper-native workflow:

- New `OpenAICodexAuth` entity spec and CSDL surface.
- New `openai_codex_auth` WASM integration for ChatGPT/Codex device-code OAuth.
- Shared `openai-codex-wire` crate for Codex endpoint/header/JWT account-id behavior.
- Provider caller and compactor now route `openai_codex` to `https://chatgpt.com/backend-api/codex/responses`, not `https://api.openai.com/v1/responses`.
- Setup API and dashboard expose managed device-code login/check/disconnect.
- CLI/deploy/setup docs no longer import `~/.codex/auth.json` as canonical auth.
- Cedar policy grants `openai_codex_auth` only the boundaries it needs: `http_call` and `access_secret`.

OpenClaw parity checked against `openclaw/openclaw`:

- `docs/providers/openai.md` documents `openai-codex/*` as a distinct Codex OAuth route and says onboarding no longer imports OAuth material from `~/.codex`.
- `extensions/openai/openai-codex-device-code.ts` uses the same OpenAI auth base URL, client id, device-code usercode endpoint, polling endpoint, token exchange endpoint, callback URL, and 15-minute timeout shape.

## Verification

All root Rust checks were run from `/tmp/openpaw-codex-auth-verify.HiSmtW` to avoid the parent worktree's local Temper patch overriding the locked git Temper dependency.

### Red

`cargo test -p temperpaw session_policy_authorizes_openai_codex_auth_wasm_boundaries --test session_turn_architecture --locked`

Failed before the Cedar policy change:

```text
openai_codex_auth must be authorized for http_call and access_secret
```

### Green

`cargo test -p temperpaw session_policy_authorizes_openai_codex_auth_wasm_boundaries --test session_turn_architecture --locked`

Result: passed.

`cargo test -p temperpaw openai_codex --tests --locked`

Result: passed setup API, OpenAICodexAuth spec/CSDL, and policy coverage tests.

`cargo check -p temperpaw-cli --locked`

Result: passed.

`cargo test --manifest-path os-apps/paw-agent/wasm/openai-codex-wire/Cargo.toml --lib`

Result: passed.

`cargo test --manifest-path os-apps/paw-agent/wasm/provider_caller/Cargo.toml --lib --locked`

Result: passed 16 tests.

`cargo test --manifest-path os-apps/paw-agent/wasm/context_compactor/Cargo.toml --lib`

Result: passed 2 tests.

`cargo test --manifest-path os-apps/paw-agent/wasm/openai_codex_auth/Cargo.toml --lib`

Result: passed 3 tests.

`npm run check` in `dashboard/`

Result: `svelte-check found 0 errors and 0 warnings`.

`npm run build` in `dashboard/`

Result: passed.

## Local E2E

Started a clean local server:

```text
HOME=/tmp/openpaw-e2e-home.codex-auth-policy
PORT=46321
OTEL_ENABLED=false
TEMPERPAW_WASM_STARTUP_POLICY=build
cargo run -p temperpaw -- run
```

Observed boot evidence:

- `/healthz` returned `200`.
- `OpenAICodexAuth` spec verified during bootstrap.
- `openai_codex_auth` WASM registered from the `paw-agent` OS app.

Exercised the setup API and OData surface:

- `GET /paw/setup/openai-codex/status` returned `configured=false`, `status=null`.
- `GET /tdata/OpenAICodexAuths` returned an empty entity set.
- `POST /paw/setup/openai-codex/device-login` returned `status="Starting"`.
- After the WASM integration completed, status returned:

```json
{
  "configured": false,
  "status": "DeviceCodeReady",
  "verification_url": "https://auth.openai.com/codex/device",
  "user_code": "1TJA-N1W81",
  "last_error": null
}
```

OData entity history showed:

- `Created` -> `Idle`
- `StartDeviceLogin`: `Idle` -> `Starting`
- `DeviceCodeReady`: `Starting` -> `DeviceCodeReady`

Polling before human approval:

- `POST /paw/setup/openai-codex/poll` returned `status="Polling"`.
- After the one-shot poll completed without approval, status returned to `DeviceCodeReady`.
- OData event showed `DeviceCodeReady`: `Polling` -> `DeviceCodeReady`.

Disconnect:

- `POST /paw/setup/openai-codex/disconnect` returned `status="Disconnected"`.
- `/observe/agents/system/history?entity_type=OpenAICodexAuth&limit=20` showed `Disconnect`: `DeviceCodeReady` -> `Disconnected`, `authz_denied=false`.

The first E2E attempt intentionally caught a missing Cedar boundary:

```text
authorization denied for http_call: no matching permit policy
```

That failure is now covered by `session_policy_authorizes_openai_codex_auth_wasm_boundaries`.
