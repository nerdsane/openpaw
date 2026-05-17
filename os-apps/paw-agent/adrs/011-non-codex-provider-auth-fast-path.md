# ADR-011: Non-Codex Provider Auth Fast Path

- Status: Proposed
- Date: 2026-05-17

## Context

PERF-007 removed the no-op `CheckSteering` stage for ordinary terminal Session
turns. The deployed proof on `480e5b41d899db1cd08f6701953449eeda766a70`
showed warm client Session p50 around 1.97 seconds and sampled Datadog workflow
roots around 3.92 seconds, with zero `CheckSteering` / `steering_checker`
spans.

The next residual heat is earlier staged workflow orchestration. For mock and
API-key providers, the current state path still goes:

`PreparingContext -> EnsuringProviderAuth -> CallingProvider`

The `provider_auth_gate` module then immediately returns
`ProviderAuthReady(provider_auth_status = "skipped")` when the normalized
provider is not `openai_codex`. That keeps the workflow explicit, but it adds
one Session action, one WASM integration, one callback dispatch, and one state
transition on the common path where there is no Codex OAuth to ensure.

The Codex OAuth provider is different. `openai_codex` and its aliases need the
auth gate because the provider call depends on fresh ChatGPT OAuth credentials
and account identity. That path must not be weakened.

## Decision

Add a Session action for the non-Codex fast path that transitions directly from
`PreparingContext` to `CallingProvider` and triggers `call_provider` while
recording the same skipped provider-auth fields.

`context_preparer` will select the action after it writes the prepared-context
artifact:

- If the normalized provider is `openai_codex`, dispatch the existing
  `ContextReady` action and continue through `provider_auth_gate`.
- For all other normalized providers, dispatch the new fast-path action with
  the prepared-context fields plus:
  - `provider_auth_status = "skipped"`
  - `provider_auth_checked_at_ms = <current timestamp>`
  - `provider_auth_error = ""`
  - current `provider_auth_retry_count`
  - current `compaction_auth_retry_count`

Provider normalization must match the existing `provider_auth_gate` and
`provider_caller` normalization for aliases such as `codex`, `openai-codex`,
and `open_router`.

## Semantics

This is not an auth bypass. It removes only the stage that already decided auth
was unnecessary for non-Codex providers. API-key provider correctness remains
owned by `provider_caller`, which still requires and uses the provider-specific
secret before external LLM calls.

`openai_codex` continues to route through `EnsuringProviderAuth`, including
fresh-token ensure, force refresh after provider-auth-expired errors, and the
existing retry budget.

Compaction auth remains unchanged in this slice. It can receive the same
treatment later if Datadog shows it is worth doing and the correctness contract
is equally narrow.

## Consequences

Positive:

- Mock, Anthropic, OpenRouter, and OpenAI Session turns avoid one no-op
  orchestration stage.
- The state machine still exposes the decision in entity fields via
  `provider_auth_status = "skipped"`.
- The provider call remains a Temper-native action-triggered WASM integration.

Tradeoffs:

- The Session spec grows one action to distinguish "context prepared, auth
  skipped" from "context prepared, auth must be ensured".
- Metrics that count `ProviderAuthReady` actions for all providers will need to
  treat the new fast-path action as the skipped-auth equivalent.

## Verification

- Add red unit coverage in `context_preparer` for provider route selection:
  - `mock`, `anthropic`, `openrouter`, and `openai` select the fast-path action.
  - `codex`, `openai-codex`, and `openai_codex` keep `ContextReady`.
- Add Session architecture tests proving:
  - the new action transitions from `PreparingContext` to `CallingProvider`;
  - the new action triggers `call_provider`;
  - Cedar permits the new system callback action.
- Run focused tests for `context_preparer` and Session architecture.
- Build the affected WASM module and the full paw-agent WASM bundle.
- Run CI-equivalent checks before PR.
- After merge and deploy, live-proof a mock Session and verify Datadog shows no
  `Session.ProviderAuthReady.integrations` / `provider_auth_gate` span on the
  non-Codex path while `openai_codex` remains gated.

## Rollback

Revert the new Session action and `context_preparer` route-selection change.
All providers then return to the existing `ContextReady -> provider_auth_gate ->
ProviderAuthReady` path.
