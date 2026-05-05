# 064 - OpenAI Codex Auth Refresh Gate

Date: 2026-05-05

## Scope

Verify the Codex OAuth fix end to end:

- `OpenAICodexAuth.EnsureFresh` is a Temper entity transition that refreshes or no-ops inside WASM.
- Session provider calls pass through `EnsuringProviderAuth` / `ProviderAuthReady`.
- A transport-style channel ingress still reaches a completed Session through the auth gate.

## Build And Tests

- `cargo test -p temperpaw` - pass: 54 unit tests, 5 native_skill_installation tests, 2 paw_fs_versioning tests, 4 session_lifecycle_and_config tests, 13 session_turn_architecture tests.
- `cargo test --manifest-path os-apps/paw-agent/wasm/openai-codex-wire/Cargo.toml` - pass: 4 tests.
- `cargo test --manifest-path os-apps/paw-agent/wasm/openai_codex_auth/Cargo.toml` - pass: 3 tests.
- `cargo test --manifest-path os-apps/paw-agent/wasm/provider_auth_gate/Cargo.toml` - pass: 3 tests.
- `cargo test --manifest-path os-apps/paw-agent/wasm/provider_caller/Cargo.toml` - pass: 23 tests.
- `cargo test --manifest-path os-apps/paw-agent/wasm/context_compactor/Cargo.toml` - pass: 12 tests.
- `cargo build -p temperpaw` - pass.
- `os-apps/paw-agent/wasm/build.sh` - pass; built `provider_auth_gate` plus existing paw-agent modules.

## Runtime Proof

Booted a disposable local server:

```text
PORT=3487
TURSO_URL=file:/tmp/temperpaw-codex-auth-proof.../paw.db
TEMPER_API_KEY=test-local-key
LLM_PROVIDER=mock
LLM_MODEL=mock-model
```

Startup reached:

```text
Temper Paw is running.
API: http://localhost:3487/tdata
```

Discord and Slack transports were not started because the disposable vault had no transport tokens. For the user-facing flow, I used the Temper-native `Channel.ReceiveMessage` ingress path with a webhook proof channel.

## Codex Auth Entity

Seeded fresh fake Codex OAuth secrets with a JWT carrying:

```text
chatgpt_account_id=acct_e2e_codex_auth
expires_at_ms=1778072503000
```

Then:

```text
POST /paw/setup/openai-codex/ensure-fresh
```

returned:

```json
{
  "configured": true,
  "status": "Ready",
  "expires_at_ms": "1778072503000",
  "account_id": "acct_e2e_codex_auth",
  "last_error": null
}
```

OData state transition evidence:

```json
{
  "status": "Ready",
  "events": ["Created", "EnsureFresh", "LoginComplete"]
}
```

This confirms setup routes now await the auth WASM before reporting readiness.

## Session Gate

Created and configured a mock Session. Observed runtime states:

```text
Provisioning
PreparingContext
EnsuringProviderAuth
CallingProvider
ApplyingProviderResponse
Steering
Completed
```

Final Session evidence:

```json
{
  "status": "Completed",
  "provider_auth_status": "skipped",
  "events": [
    "Created",
    "Configure",
    "ProvisionWorkspace",
    "WorkspaceReady",
    "ContextReady",
    "ProviderAuthReady",
    "Heartbeat",
    "ProviderResponseReady",
    "CheckSteering",
    "FinalizeResult"
  ]
}
```

## Channel Ingress

Created a `webhook-proof` Channel and AgentRoute with `provider=mock`, then dispatched:

```text
Channels(...).Paw.Channel.ReceiveMessage
```

Webhook reply was delivered:

```json
{
  "reply_content": "hello from the proof channel"
}
```

The routed Session reached:

```json
{
  "session_status": "Completed",
  "provider_auth_status": "skipped",
  "session_events": [
    "Created",
    "Configure",
    "ProvisionWorkspace",
    "WorkspaceReady",
    "ContextReady",
    "ProviderAuthReady",
    "Heartbeat",
    "ProviderResponseReady",
    "CheckSteering",
    "FinalizeResult",
    "MarkTrajectoryEmitted"
  ]
}
```

## Result

The fix preserves the Temper-native audit trail: token freshness is visible on `OpenAICodexAuth`, provider calls are gated in `Session`, and ingress through the channel trigger still completes.
