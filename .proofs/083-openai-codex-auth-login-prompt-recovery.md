# Proof 083: OpenAI Codex Auth Login Prompt Recovery

Date: 2026-06-03

## Problem

Discord DMs were failing with the dead-end message:

`OpenAI Codex auth failed. OpenAI Codex sign-in is required; start the Codex device login again.`

That was not actionable in Discord. If the stored OpenAI Codex OAuth refresh token is revoked or reused, the system must start the existing Temper-native device-code flow and surface the `verification_url` plus `user_code` in the Session failure message.

## Root Cause

- OpenAI returned real auth failures earlier in the day, including HTTP 401 `token_revoked` / invalidated-token responses.
- `refresh_token_reused` and invalidated-token responses mean the stored refresh token cannot be recovered by retrying refresh; a human sign-in is required.
- `provider_auth_gate` did not turn every sign-in-required auth state into an actionable Discord prompt.
- `DeviceCodeReady` was treated too close to provider-ready behavior in the old gate path; it is a waiting-for-human state.
- Production also used Genesis-pinned/stale installed paw-agent WASM bytes. The container image changed, but the live Session path continued to run the old auth gate until the fixed modules were hot-uploaded.

## Source Changes

Commits on branch `codex/openai-token-context-rca-20260603`:

- `951b18332e64097f453fdcd9814c1e76e85b4a00` - `Fix Codex auth login prompt recovery`
- `a51dcf7f2a59701f6afd876883b1691630a61f75` - `Prompt on generic Codex auth failure`

Changed files:

- `os-apps/paw-agent/wasm/provider_auth_gate/src/lib.rs`
- `os-apps/paw-agent/wasm/openai_codex_auth/src/lib.rs`
- `os-apps/paw-agent/adrs/031-openai-codex-auth-login-prompt-recovery.md`

ADR:

- `os-apps/paw-agent/adrs/031-openai-codex-auth-login-prompt-recovery.md`

## Local Verification

Red/green TDD:

- Added provider-gate tests for device-code prompt behavior, `DeviceCodeReady` not being provider-ready, empty `last_error` fallback, and generic `Failed` auth status starting device login.
- Added OpenAI auth tests for `refresh_token_reused`, invalidated-token, and revoked-token classification.

Commands run:

```text
CARGO_NET_GIT_FETCH_WITH_CLI=true cargo test --manifest-path os-apps/paw-agent/wasm/provider_auth_gate/Cargo.toml -- --nocapture
```

Result: 8 passed.

```text
CARGO_NET_GIT_FETCH_WITH_CLI=true cargo test --manifest-path os-apps/paw-agent/wasm/openai_codex_auth/Cargo.toml -- --nocapture
```

Result: 4 passed.

```text
CARGO_NET_GIT_FETCH_WITH_CLI=true bash os-apps/paw-agent/wasm/build.sh
```

Result: all paw-agent WASM modules built. Only unrelated existing warnings appeared in `sandbox_provisioner` and `monty_repl`.

```text
CARGO_NET_GIT_FETCH_WITH_CLI=true cargo build -p temperpaw
```

Result: passed.

## Image And Railway Deployment

Docker workflow:

- GitHub Actions run: `26917753269`
- Job: `79411295958`
- Result: success
- Image: `ghcr.io/nerdsane/temperpaw:sha-a51dcf7`
- Image digest: `sha256:da3089f45baa8c335a28527953cee09a5daba2c5edd76e5519d947f32bf942bb`
- Build version: `sha-a51dcf7f`
- Build SHA: `a51dcf7f2a59701f6afd876883b1691630a61f75`

Railway target:

- Project: `openpaw-seshendranalla`
- Environment: `production`
- Service: `openpaw`
- Base URL: `https://openpaw-production.up.railway.app`

Deployment:

- Railway deployment: `9103e7fd-9373-4825-998e-28577701a193`
- Railway status: `SUCCESS`
- `/readyz`: ready
- Discord transport: connected
- `/paw/version`: `{"version":"sha-a51dcf7f","sha":"a51dcf7f2a59701f6afd876883b1691630a61f75"}`

The GitHub Railway redeploy path was not used because the GitHub environment still lacks the required Railway/TEMPER secrets for this branch/environment. The app-managed Railway redeploy path also failed earlier with `Railway Runtime Agent variableUpsert failed: Not Authorized`, which points to the app vault's Railway token being stale or under-scoped. Deployment was completed with the locally authenticated Railway CLI.

## Runtime Artifact Repair

Post-image smoke still produced the old dead-end string, proving the live Session path was still using stale installed WASM bytes.

Hot-uploaded fixed modules through Temper:

```text
POST /api/wasm/modules/provider_auth_gate
sha256_hash=b6bed76eeb61d7449c3c7e4b7848e5a238490fc1205244a10f4b3637c88520dc
size_bytes=227424
HTTP 200
```

```text
POST /api/wasm/modules/openai_codex_auth
sha256_hash=4f450a490d029166af8f3cdade4dc23b560f36bc914a81c3b2cea431af39e227
size_bytes=273685
HTTP 200
```

## Production E2E

Pre-hotload smoke:

- Session: `ss-019e8fc4-2fc4-7de3-bcf3-30e4fdec549b`
- Version: `sha-a51dcf7f`
- Result: failed with old dead-end message.
- Interpretation: container was new, but installed paw-agent WASM behavior was stale.

Post-hotload smoke:

- Session: `ss-019e8fc5-18f0-7632-b98f-fdb19ac76614`
- Version: `sha-a51dcf7f`
- Result: `Failed` at the auth boundary, as expected while human sign-in is required.
- `error_message_has_device_url`: true
- `error_message_has_enter_code`: true
- `error_message_has_old_dead_end`: false
- Full user code intentionally not recorded in this proof.

Auth setup status after smoke:

```json
{
  "configured": true,
  "status": "DeviceCodeReady",
  "verification_url_present": true,
  "user_code_present": true,
  "expires_at_ms_present": true,
  "last_error_present": false
}
```

Datadog evidence:

- Earlier production logs showed real OpenAI HTTP 401 `token_revoked` / invalidated-token provider errors.
- Recent logs for proof session `ss-019e8fc5-18f0-7632-b98f-fdb19ac76614` show `version=sha-a51dcf7f`.
- Datadog also recorded `provider_auth_gate: dispatching OpenAICodexAuth.EnsureFresh` and `provider_auth_gate: dispatching OpenAICodexAuth.ForceRefresh` for the proof session.

## Outcome

The app is deployed, Discord is connected, and the OpenAI Codex auth recovery is now actionable in Discord: a user receives the OpenAI device-login URL and a code instead of the old generic instruction.

The system is not silently logged in yet. It is intentionally in `DeviceCodeReady` until a human completes OpenAI Codex device login, then repeats the Discord request.
