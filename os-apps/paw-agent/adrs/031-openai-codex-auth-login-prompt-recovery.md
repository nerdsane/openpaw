# ADR-031: OpenAI Codex Auth Login Prompt Recovery

## Status

Accepted

## Context

Discord sessions using the OpenAI Codex provider could fail with:

`OpenAI Codex sign-in is required; start the Codex device login again.`

That message was a dead end in Discord. The `OpenAICodexAuth` entity and setup API already support device-code login, and `/paw/setup/openai-codex/device-login` returns a `verification_url` plus `user_code`, but the Session `provider_auth_gate` did not surface those fields.

Production also showed refresh failures with OpenAI error code `refresh_token_reused`. That is not a transient access-token expiry. It means the stored refresh token can no longer be used and a human must sign in again. The auth WASM only classified `invalid_grant`, missing refresh tokens, and expired-token text as sign-in-required, so refresh-token reuse was recorded as an opaque refresh failure.

Finally, `provider_auth_gate` treated any non-`Failed` auth status as provider-ready. `DeviceCodeReady` is a waiting-for-human state, not a usable provider-auth state.

## Decision

`provider_auth_gate` now considers only `OpenAICodexAuth` status `Ready` as provider-ready.

When the setup API reports `DeviceCodeReady`, the gate fails the Session with a Discord-safe message that includes the OpenAI Codex device URL, the user code, and instructions to retry the message after signing in.

When `EnsureFresh`, `Refresh`, or `ForceRefresh` fails with a sign-in-required refresh error, the gate dispatches the existing setup API device-login route. That route creates or reuses the singleton `OpenAICodexAuth` entity and dispatches `StartDeviceLogin`; the gate then includes the returned device-code prompt in the Session failure message.

If the auth entity is already `DeviceCodeReady`, `EnsureFresh` is not an allowed action. The gate recognizes that response, fetches the current auth status, and returns the existing device-code prompt instead of treating the action validation failure as an opaque HTTP 500.

The `openai_codex_auth` WASM now classifies `refresh_token_reused`, invalidated OAuth token text, and revoked-token text as sign-in-required refresh failures.

## Consequences

Discord users receive an actionable login link and code instead of a hard auth failure with no next step.

The recovery remains Temper-native: session auth still runs from the Session state machine, and device login still flows through the `OpenAICodexAuth` entity action and WASM integration.

Refresh-token reuse is not retried as if it were an ordinary expired access token. A new device login is required, which matches OpenAI refresh-token rotation semantics.

Stale `user_code` fields left on a `Failed` auth entity are not trusted. The gate only surfaces a code returned by a fresh `StartDeviceLogin` dispatch or by a current `DeviceCodeReady` status snapshot.
