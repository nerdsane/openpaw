# ADR-001: Webhook HMAC Verification

**Status:** Accepted
**Scope:** wasm-integration (validate_webhook)
**Author:** TemperPaw maintainers
**Date:** 2026-07-07
**Tracking:** ARN-168 (Class B, epic ARN-165); mirrors the kernel fix ARN-171 (temper PR #340 / ADR-0156)

## Context

`/triggers/webhook/{route_key}` is a public endpoint — it is exempted from bearer
auth (`crates/temperpaw/src/auth.rs`, `is_public_path`) because external senders
(GitHub, Datadog, …) cannot present a Temper credential. The signature over the
request body is therefore the *only* thing that authenticates an inbound webhook.

The trigger creates one `WebhookEvent` and dispatches `Received`, which fires the
`validate_webhook` WASM integration. That module was supposed to verify the HMAC
signature, but the implementation only checked that the signature *header was
present* — it never computed or compared `HMAC-SHA256(secret, body)`. It returned
`hmac_verified: "true"` whenever the header existed and always transitioned to
`Validated`, so the pipeline dispatched the payload regardless.

Exploit: `POST /triggers/webhook/github` with header
`X-Hub-Signature-256: sha256=anything` was recorded as `hmac_verified: true` and
routed into the scout/SRE/heal/patrol agents — allowing spoofed GitHub/Datadog
events, injected agent instructions, and resource abuse. The module's own doc
comment admitted it: "Full cryptographic verification can be added later." This
is an instance of systemic **Class B** (unauthenticated ingress).

## Decision

`validate_webhook` now performs real signature verification and fails closed.

- **Compute and compare.** When a route declares a `webhook_secret`, compute
  `HMAC-SHA256(secret, raw_payload)` and compare it against the signature header
  using a **constant-time** comparison (`subtle::ConstantTimeEq`), not `==`, to
  avoid a timing side channel on the digest.
- **Signature format.** The provided value is trimmed and lower-cased, an
  optional `sha256=` prefix (as GitHub sends) is stripped, then the remaining hex
  digest is compared. Bare-hex signatures (no prefix) are also accepted.
- **Secret resolution.** `webhook_secret` may be a literal value or a
  `{secret:KEY}` template resolved from the host secret store via
  `ctx.get_secret(KEY)`. An empty or unresolvable secret is treated as *not
  resolvable*.
- **Fail closed.** If a route declares a secret and the request has no signature
  header, an unresolvable secret, or a mismatched signature, the module
  transitions the `WebhookEvent` to `ValidationFailed` (→ `Rejected`) with a
  `validation_error`. The payload is never routed or processed.
- **No secret configured → skipped.** A route with an empty `webhook_secret` has
  opted out of signature verification; behaviour is unchanged (`hmac_verified:
  "skipped"`, transitions to `Validated`). This matches the kernel counterpart,
  which leaves authenticity for such routes to the Cedar gate rather than
  rejecting outright, and avoids breaking routes that intentionally carry no
  secret.

The header a route's signature is read from is selected by `source_type`
(`github` → `x-hub-signature-256`, `datadog` → `x-datadog-signature`, default
`x-hub-signature-256`). Header lookup is case-insensitive.

## Consequences

### Positive
- Spoofed webhooks with forged or absent signatures are rejected before any agent
  is invoked, closing the Class B ingress on the TemperPaw side.
- The signing secret is resolved from the secret store, not trusted from a header.
- Constant-time comparison removes a timing side channel on the digest.
- Verification logic is factored into pure, host-free functions
  (`classify_secret`, `extract_header`, `verify_signature`, `signature_matches`)
  that are unit-tested (red exploit test → green), keeping `run()` a thin shell.

### Negative
- Routes that had a `webhook_secret` set but relied on the old no-op now require
  the sender to produce a correct `HMAC-SHA256` signature; a misconfigured secret
  will start rejecting traffic (the intended fail-closed behaviour). Operators
  must ensure the stored secret matches the sender's signing key.
- Adds `hmac`, `sha2`, `hex`, and `subtle` as build dependencies of the module
  (all pure-Rust, `no_std`-friendly, and confirmed to compile for
  `wasm32-unknown-unknown`).
