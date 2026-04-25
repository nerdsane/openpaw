# ADR-0044: OpenAI Codex Subscription Auth Boundary

Date: 2026-04-25

## Status

Accepted

## Context

OpenAI ChatGPT/Codex subscription OAuth tokens are not OpenAI Platform API keys.
They are valid for the ChatGPT Codex backend, not for
`https://api.openai.com/v1/responses`; using them against the public Responses
API now fails with missing `api.responses.write`.

OpenClaw handles this by making `openai-codex` a distinct provider route with
its own OAuth lifecycle and backend URL. OpenPaw should follow the same boundary
instead of treating a Codex OAuth token as an API key variant.

## Decision

`openai` and `openai_codex` are separate provider contracts:

- `openai` uses `openai_api_key` and `https://api.openai.com/v1/responses`.
- `openai_codex` uses OpenPaw-managed ChatGPT/Codex OAuth credentials and
  `https://chatgpt.com/backend-api/codex/responses`.

Codex OAuth is modeled as a Temper entity:

- `OpenAICodexAuth.StartDeviceLogin` requests a device code.
- `OpenAICodexAuth.PollDeviceLogin` polls once and exchanges the code when
  authorized.
- `OpenAICodexAuth.Refresh` refreshes the stored access token.
- `OpenAICodexAuth.Disconnect` clears stored Codex credentials.

The setup API only creates/reads the singleton auth entity and dispatches these
actions. The OpenAI auth HTTP calls and vault writes live in the
`openai_codex_auth` WASM integration. Cedar explicitly authorizes that module
for `http_call` and `access_secret`; auth cannot rely on ambient network or
vault access.

Canonical runtime secrets are:

- `openai_codex_access_token`
- `openai_codex_refresh_token`
- `openai_codex_expires_at_ms`
- `openai_codex_account_id`

`openai_codex_token` remains a temporary legacy read fallback for existing
deployments, but it is not the canonical setup path.

## Consequences

Provider callers must never send Codex subscription tokens to
`api.openai.com/v1/responses`. Codex requests include the ChatGPT account id and
Codex SSE beta headers. Public OpenAI requests do not include these headers.

The setup dashboard uses device-code login because it works for local,
headless, and hosted deployments without requiring a callback URL.
