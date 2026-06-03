# ADR-030: Session Auth, Context, and Cron Recovery

## Status

Accepted

## Context

Production sessions showed three related failure modes:

- OpenAI Codex returned HTTP 401 with `code: "token_revoked"` and message text saying the OAuth token was invalidated. The provider caller only routed `token_expired` through `ProviderAuthExpired`, so revoked access tokens failed as hard provider errors instead of forcing one refresh.
- Entity-backed SessionEntry context could contain later entries while missing the virtual first user entry. When the current `session_leaf_id` pointed into that chain, context preparation saw a non-empty tree but built zero refs and failed to avoid wiping the conversation.
- Some CronJobs had an existing `user_message` but an empty `user_message_template`, and some had empty `model` / `provider`. The trigger WASM rendered from only the template and the declarative spawn copied empty model/provider fields, creating Sessions that failed before useful work.
- Several paw-agent WASM modules referenced by entity specs were built but not declared in `app.toml`, so the installer skipped them and triggers such as `cron_compute_next` could fail with "WASM module not found in registry."
- Provider credential selection treated literal unresolved secret templates such as `{secret:openai_api_key}` as configured credentials, which hid fallback credentials and produced noisy provider failures.
- Production Railway was bootstrapping paw-agent from a Genesis-pinned app ref, so replacing the container image alone did not replace the live CronJob spec or WASM bytes. The stale pinned spec had no cron trigger defaults, and the stale module did not read PascalCase OData `UserMessage`.

## Decision

OpenAI Codex 401 responses that indicate expired, invalidated, or revoked tokens are all treated as provider-auth-expired outcomes. The existing `ProviderAuthExpired` action and `provider_auth_gate` force-refresh path remain the only retry mechanism.

Entity-backed SessionEntry reads synthesize the missing virtual initial header and user entry only when later entries reference `u-<session_id>-0` and the original `Session.user_message` is available. Other broken graphs still fail rather than silently feeding an empty or invented context to the LLM.

Cron trigger computation now renders from `user_message_template` when present and falls back to existing `CronJob.user_message` / `UserMessage`. It also resolves missing `model` and `provider` from tenant defaults exposed to the WASM trigger. `TriggerComplete` records the resolved values and passes them as explicit callback params; Temper's spawn merge preserves those params over stale copied parent fields.

CronJob now carries explicit default state for the optional `Session.Configure` surface and includes those fields in the declarative `copy_fields` spawn mapping. This keeps the entire cron-to-session handoff visible in entity state and satisfies the production spec linter before hot-loading the app spec.

The paw-agent manifest now declares every spec-triggered WASM module as an app-required artifact. Hot Session modules remain eager; lower-frequency modules such as cron, OpenAI Codex auth, approval handlers, and workspace restoration are lazy but registered.

Provider credential selection now skips unresolved `{secret:...}` templates before choosing a provider API key or OAuth token. Missing provider credentials still fail explicitly, but a placeholder can no longer mask a later valid fallback.

When production is using Genesis-pinned app refs, operational repair must update the live app spec/WASM registry or publish a new Genesis app ref. Container deployment is necessary for Rust/runtime changes but is not sufficient to replace pinned app artifacts.

## Consequences

Revoked Codex access tokens now get one entity-visible refresh attempt instead of repeated hard 401 failures. If the refresh token itself is invalid, the auth entity still fails and the operator must run device login again.

Session context recovery stays auditable from state transitions and SessionEntry state. The repair is limited to the known virtual-first-turn shape so unrelated data loss remains visible.

CronJobs with legacy or partially populated configuration stop spawning empty Sessions when tenant defaults exist. CronJobs with no usable message or no resolvable model/provider fail at `TriggerFailed`, which is safer than creating broken work. New cron-spawned Sessions receive the same optional configuration defaults as ordinary Sessions unless the CronJob state deliberately overrides them.

Manifest drift now fails a startup contract test before it can reach production as a missing WASM registry entry.

Unconfigured optional provider secrets no longer create misleading "unresolved secret template" provider calls. If no real credential is available, the Session fails with a direct missing-key error.
