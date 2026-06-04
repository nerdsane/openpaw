# Proof 085: OpenAI Codex Device Login Autopoll

Date: 2026-06-04

## Problem

Discord DMs sent a valid OpenAI Codex device-login code, but after the user completed browser login Paw repeated the same sign-in prompt instead of clearly picking up the authorization.

The prompt also duplicated the lead sentence:

`OpenAI Codex sign-in is required. OpenAI Codex sign-in is required. ...`

## Observed Production Timeline

Datadog logs around the user's Discord messages showed:

- `2026-06-04T17:52:00Z`: `provider_auth_gate` ran on `version=sha-fb02704d`.
- The previous device code was expired, and the gate started a fresh code.
- `2026-06-04T17:52:01Z`: Paw replied in Discord with the fresh sign-in prompt.
- `2026-06-04T17:52:29Z`: a second Discord message ran `provider_auth_gate` again and replied again.
- `2026-06-04T17:53:24Z`: `openai_codex_auth: mode=poll` observed the completed browser authorization.
- `2026-06-04T17:53:48Z`: `provider_auth_gate` ran again with auth ready.
- `2026-06-04T17:53:49Z`: Codex provider call started.
- `2026-06-04T17:53:51Z`: Codex provider call returned successfully.

Direct setup check after polling:

```json
{
  "configured": true,
  "status": "Ready",
  "account_id_present": true,
  "last_error": null
}
```

## Root Cause

The device login flow still depended on a later trigger to poll OpenAI. If the user authorized the browser code after Paw had already replied, nothing automatically observed completion until a new Session or an explicit setup poll ran.

`provider_auth_gate` did poll on Session retry after Proof 084, but `OpenAICodexAuth` itself did not self-poll while in `DeviceCodeReady`.

## Source Changes

Commit:

- `25d94a94e4b7a3115d579913f5ddaa51ac400ccb` - `Auto-poll Codex device login`

Changed files:

- `os-apps/paw-agent/specs/openai_codex_auth.ioa.toml`
- `os-apps/paw-agent/specs/model.csdl.xml`
- `os-apps/paw-agent/wasm/openai_codex_auth/src/lib.rs`
- `os-apps/paw-agent/wasm/provider_auth_gate/src/lib.rs`
- `os-apps/paw-agent/adrs/031-openai-codex-auth-login-prompt-recovery.md`
- `crates/temperpaw/tests/session_lifecycle_and_config.rs`

Behavior added:

- `OpenAICodexAuth.DeviceCodeReady` schedules `PollDeviceLogin` after 5 seconds.
- Each pending `DeviceCodeReady` increments `poll_attempt_count`.
- `PollDeviceLogin` is guarded by `poll_attempt_count < 180`, giving a bounded roughly 15-minute polling window.
- Fresh device-code start resets `poll_attempt_count` to `0`.
- Polling fails expired device codes instead of keeping a dead prompt alive.
- `provider_auth_gate` no longer duplicates `OpenAI Codex sign-in is required` in Discord prompts.

ADR:

- `os-apps/paw-agent/adrs/031-openai-codex-auth-login-prompt-recovery.md`

## Local Verification

Red/green tests:

- `sign_in_message_does_not_duplicate_required_phrase`
- `paw_agent_defines_temper_native_openai_codex_auth_entity`
- `device_code_without_expiry_is_not_expired`

Commands:

```text
CARGO_NET_GIT_FETCH_WITH_CLI=true cargo test --manifest-path os-apps/paw-agent/wasm/provider_auth_gate/Cargo.toml -- --nocapture
```

Result: 11 passed.

```text
CARGO_NET_GIT_FETCH_WITH_CLI=true cargo test --manifest-path os-apps/paw-agent/wasm/openai_codex_auth/Cargo.toml -- --nocapture
```

Result: 5 passed.

```text
CARGO_NET_GIT_FETCH_WITH_CLI=true cargo test -p temperpaw --test session_lifecycle_and_config -- --nocapture
```

Result: 7 passed.

```text
CARGO_NET_GIT_FETCH_WITH_CLI=true bash os-apps/paw-agent/wasm/build.sh
```

Result: all paw-agent WASM modules built. Existing unrelated warnings remained in `sandbox_provisioner` and `monty_repl`.

```text
CARGO_NET_GIT_FETCH_WITH_CLI=true cargo build -p temperpaw
```

Result: passed.

## Production Hotload

Submitted the updated paw-agent spec bundle through:

```text
POST /api/specs/load-inline
HTTP 200
spec_count=14
```

Hot-uploaded changed modules:

```text
POST /api/wasm/modules/provider_auth_gate
sha256_hash=601be54eb47cd125646257720e41bf1b0b2703a52409bf020c3b10aacbd09f19
size_bytes=232393
HTTP 200
```

```text
POST /api/wasm/modules/openai_codex_auth
sha256_hash=cf46b790569f380ce8d67b9d63c689d970f245bb858325520dd45aa3b2e10d3a
size_bytes=274742
HTTP 200
```

Production status after hotload:

```json
{
  "readyz": "ready",
  "discord": "connected",
  "openai_codex_status": "Ready"
}
```

## Production Smoke

Direct no-reply Codex Session:

- Session: `ss-019e93cd-36fd-7c22-ab7f-194af85f79fa`
- Prompt: `Codex auth post-login smoke. Reply exactly: Codex login works.`
- Result status: `Completed`
- `provider_auth_status`: `Ready`
- Error: false
- Result prefix: `Codex login works.`

## Baked Image Deployment

Baked image:

- GitHub Actions Docker run: `26970132432`
- Result: success
- Image: `ghcr.io/nerdsane/temperpaw:sha-25d94a9`
- Image digest: `sha256:20b46beab351c314f932aa3cbb3df518d227f439e233a9f2a78c7b29bcc62b88`
- Build version: `sha-25d94a94`
- Build SHA: `25d94a94e4b7a3115d579913f5ddaa51ac400ccb`

Railway:

- Deployment: `bee0762d-1d01-4dac-b420-cba5d9016ab3`
- Service: `openpaw`
- Environment: `production`
- Status: `SUCCESS`
- `/readyz`: ready
- Discord transport: connected
- `/paw/version`: `{"version":"sha-25d94a94","sha":"25d94a94e4b7a3115d579913f5ddaa51ac400ccb"}`

Baked-image smoke:

- Session: `ss-019e93e8-ac37-7d91-a7ab-8a875056e89a`
- Result status: `Completed`
- `provider_auth_status`: `Ready`
- Error: false
- Result prefix: `Codex autopoll baked.`
- Post-smoke auth status: `Ready`

Datadog evidence:

- `version=sha-25d94a94`
- `provider_auth_gate: dispatching OpenAICodexAuth.EnsureFresh`
- `session_turn: OpenAI Codex response: blocks=1, stop=end_turn, in=199, out=10`
- `emit_ots_trajectory ... status=Completed`

## Outcome

The already-entered browser login was valid and was picked up once polling ran. The live auth entity is now `Ready`, and a direct Codex provider Session completed successfully. The follow-up change makes the auth entity poll itself while a code is pending, so completion no longer depends on a perfectly timed extra Discord message or manual setup poll.
