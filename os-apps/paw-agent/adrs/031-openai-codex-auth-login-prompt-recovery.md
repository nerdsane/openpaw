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

If the auth entity is already `Failed`, the gate also dispatches the existing device-login route even when the stored failure has no useful `last_error`. A failed OpenAI Codex auth setup is not a provider-ready state, and Discord recovery needs an actionable sign-in prompt rather than a generic retry instruction.

If the auth entity is `DeviceCodeReady` when a Session retries, the gate now dispatches `PollDeviceLogin` before re-prompting. If the user has authorized the code, polling stores tokens and the Session continues to the provider. If authorization is still pending, the gate returns the current prompt only when it has more than a short safety window remaining; expired or nearly expired codes are replaced by a fresh `StartDeviceLogin` prompt.

`OpenAICodexAuth.DeviceCodeReady` now also schedules bounded self-polling. The entity increments `poll_attempt_count` and schedules `PollDeviceLogin` every five seconds, guarded by `poll_attempt_count < 180`. This keeps the login completion loop Temper-native: once the user enters the device code, the auth entity can observe completion and store tokens without waiting for another Discord message. The polling window is bounded to roughly the 15-minute device-code lifetime.

The `openai_codex_auth` WASM now classifies `refresh_token_reused`, invalidated OAuth token text, and revoked-token text as sign-in-required refresh failures.

## Consequences

Discord users receive an actionable login link and code instead of a hard auth failure with no next step.

The recovery remains Temper-native: session auth still runs from the Session state machine, and device login still flows through the `OpenAICodexAuth` entity action and WASM integration.

Refresh-token reuse is not retried as if it were an ordinary expired access token. A new device login is required, which matches OpenAI refresh-token rotation semantics.

Stale `user_code` fields left on a `Failed` auth entity are not trusted. The gate only surfaces a code returned by a fresh `StartDeviceLogin` dispatch or by a current `DeviceCodeReady` status snapshot.

Expired `DeviceCodeReady` prompts are not reused. The retry path also becomes the completion path: after a human enters the code in OpenAI, sending the Discord message again causes the gate to poll and complete login instead of repeating the same code forever.

The auth entity also self-polls while a code is pending, reducing the chance that Discord users see repeated login prompts after successful browser authorization. If the device code expires before OpenAI authorizes it, polling fails the auth entity and the next Session attempt starts a fresh code.
