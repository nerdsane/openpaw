# ADR-012: Provider Caller Lazy Heartbeat

- Status: Proposed
- Date: 2026-05-17

## Context

PERF-008 deployed the non-Codex provider-auth fast path on
`20f647e051ebc5d714b2ae4cca8bf15ae266eb5e`. The live mock-provider proof
completed through:

`Provisioning -> PreparingContext -> CallingProvider -> ApplyingProviderResponse -> Completed`

The warm five-Session sample improved from PERF-007's 1.97 second p50 to
1.476 second p50. Datadog confirmed the `provider_auth_gate` stage was gone:
the retained proof trace included `ContextReadyAuthSkipped`, no
`ProviderAuthReady`, and zero `provider_auth_gate` logs.

The remaining proof trace shows that ordinary dispatch actions are now small
for this path, usually around 15-21 ms. The larger residual is staged
orchestration around the WASM modules. One avoidable part of the current
provider-caller prelude is an unconditional `Session.Heartbeat` dispatch before
the provider HTTP call:

`CallingProvider -> Heartbeat(self-loop) -> ProviderResponseReady`

The heartbeat is valuable for a module that is about to block for a long time:
it records liveness and drives user typing feedback without resetting
`CallingProvider`'s state timeout. But for the fast path it is a full
Temper action, authorization check, event write, projection update, SSE
broadcast, and reaction pass before the provider call even starts. The module
already posts the typing indicator directly, keeps the 600 second
`CallingProvider` timeout, and can emit explicit progress when configured.

## Decision

Remove the unconditional pre-provider `Heartbeat` from the normal
`provider_caller` hot path.

Keep the liveness semantics where they are actually needed:

- Preserve the explicit mock-hang path, which blocks intentionally to exercise
  timeout behavior.
- Preserve direct typing-indicator emission before provider I/O so user-facing
  transports still get immediate feedback.
- Preserve `ProviderResponseReady` / `ProviderAuthExpired` / failure callbacks.
- Preserve the `CallingProvider` state timeout and existing
  `ProgressMade`/`ResumeProvider` reset semantics.
- Keep provider-boundary `ProgressMade` dispatch gated by the existing
  `provider_progress_dispatch_enabled` configuration.

The provider call itself remains the Session action-triggered WASM integration.
No provider logic moves into Rust orchestration and no audit event is hidden.

## Semantics

`Heartbeat` is a liveness signal, not a correctness transition. It does not
reset `CallingProvider`'s timeout and does not change provider-call inputs or
outputs. Removing the eager heartbeat therefore does not weaken the provider
result contract.

Typing feedback remains available through `send_typing_indicator`, which does
not mutate Session state. Long-running providers can still use configured
progress dispatch, provider streaming progress, retries, and the 600 second
state timeout. If a future production path needs periodic heartbeat while a
blocking non-streaming provider call is in flight, that should be an explicit
budgeted progress/heartbeat policy rather than an unconditional pre-call
self-loop on every fast turn.

## Consequences

Positive:

- Fast provider calls avoid one Session self-loop action and the associated
  authz/event/projection/reaction work.
- Mock-provider proof traces should no longer contain a `Heartbeat` action
  between `ContextReadyAuthSkipped` and `ProviderResponseReady`.
- User-facing typing feedback is preserved without an entity write.

Tradeoffs:

- Operators lose one eager `last_heartbeat_at` update at the start of provider
  calls. This field was liveness-only and did not reset the timeout.
- If a non-streaming provider stalls inside a single host HTTP call, the next
  visible state movement may be the provider response, provider error, or the
  `CallingProvider` timeout rather than an initial heartbeat event.

## Verification

- Add red coverage proving `provider_caller` has an explicit hot-path heartbeat
  gate and no longer calls `send_heartbeat` unconditionally before provider I/O.
- Keep coverage that provider-boundary `ProgressMade` remains gated by
  `provider_progress_dispatch_enabled`.
- Run the affected provider-caller unit tests and Session architecture tests.
- Build the provider-caller WASM module and the full paw-agent WASM bundle.
- Run CI-equivalent checks before PR.
- After merge and deploy, live-proof a mock Session and verify:
  - state path remains
    `Provisioning -> PreparingContext -> CallingProvider -> ApplyingProviderResponse -> Completed`;
  - no `Heartbeat` action appears between `ContextReadyAuthSkipped` and
    `ProviderResponseReady`;
  - `provider_auth_status = "skipped"` still persists;
  - Datadog shows the retained trace on the deployed version with no
    provider-auth gate and no provider-caller prelude heartbeat.

## Rollback

Reintroduce the eager `send_heartbeat` call before provider I/O in
`provider_caller`. The provider path then returns to emitting a liveness
self-loop at the start of every non-hang provider call.
