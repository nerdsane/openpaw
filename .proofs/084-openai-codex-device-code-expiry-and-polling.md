# Proof 084: OpenAI Codex Device Code Expiry And Polling

Date: 2026-06-03

## Problem

After Proof 083, Paw sent an OpenAI Codex device login code in Discord DMs, but a user reported that entering the code could not authorize.

The live auth status showed why the prompt was bad:

```json
{
  "configured": true,
  "status": "DeviceCodeReady",
  "verification_url_present": true,
  "user_code_present": true,
  "expires_at_ms_present": true,
  "expires_in_seconds": -3996,
  "account_id_present": true,
  "last_error": null
}
```

The code being repeated in Discord was already expired.

## Root Cause

`provider_auth_gate` had two remaining holes:

- It reused a `DeviceCodeReady` prompt without checking `expires_at_ms`.
- On retry while the auth entity was `DeviceCodeReady`, it returned/restarted prompts instead of polling `OpenAICodexAuth.PollDeviceLogin`. That meant retrying after a human authorized a code could not complete the login loop.

## Source Changes

Changed files:

- `os-apps/paw-agent/wasm/provider_auth_gate/src/lib.rs`
- `os-apps/paw-agent/adrs/031-openai-codex-auth-login-prompt-recovery.md`

Behavior added:

- Parse `expires_at_ms` from `DeviceCodeReady` status.
- Treat device codes with less than a 30-second safety window as unusable.
- Start a fresh `StartDeviceLogin` prompt when the current code is expired or nearly expired.
- When the setup API reports `DeviceCodeReady`, dispatch `/paw/setup/openai-codex/poll`.
- If polling returns `Ready`, continue the Session to the provider.
- If polling returns `DeviceCodeReady`, return the current prompt only if it is still fresh.

## Local Verification

Red/green tests:

- Added `device_login_prompt_expiry_is_honored`.
- Added `device_login_prompt_reads_expires_at_ms`.

Commands:

```text
CARGO_NET_GIT_FETCH_WITH_CLI=true cargo test --manifest-path os-apps/paw-agent/wasm/provider_auth_gate/Cargo.toml -- --nocapture
```

Result: 10 passed.

```text
CARGO_NET_GIT_FETCH_WITH_CLI=true bash os-apps/paw-agent/wasm/build.sh
```

Result: all paw-agent WASM modules built. Only unrelated existing warnings appeared in `sandbox_provisioner` and `monty_repl`.

```text
CARGO_NET_GIT_FETCH_WITH_CLI=true cargo build -p temperpaw
```

Result: passed.

## Production Hotload

Hot-uploaded fixed module:

```text
POST /api/wasm/modules/provider_auth_gate
sha256_hash=1546a9f33722ac2e169b099119c34667fc3096209b059d1629c52a1fc97eb95a
size_bytes=231890
HTTP 200
```

## Production E2E

Smoke Session:

- Session: `ss-019e9012-948a-7bf2-9b88-0cdd896a4478`
- Result: `Failed` at the expected auth boundary while sign-in is still required.
- `message_has_device_url`: true
- `message_has_enter_code`: true
- `message_has_old_dead_end`: false

Auth status after smoke:

```json
{
  "auth_status": "DeviceCodeReady",
  "auth_user_code_present": true,
  "auth_expires_in_seconds": 897
}
```

The smoke proved that production no longer reuses the expired code and instead starts a fresh OpenAI Codex device code.

## Outcome

The current Discord prompt should now contain a fresh code. After the human enters the code, sending the Discord message again should cause the auth gate to poll `OpenAICodexAuth.PollDeviceLogin`; if OpenAI has authorized the code, tokens will be stored and the Session can continue.
