# Proof Report: ARN-167 — Credential-bound ingress identity

## Date

2026-07-09

## Branch / Commit

`codex/pr452-review-fixes` based on PR #452 head
`f4149275b76174f2a29c521911dfe8690c74e593`.

## What Was Done

- Removed loopback-address authentication and the corresponding connection-info
  server plumbing.
- Stripped all client-assertable identity and tenant headers for every peer.
- Kept session identity injection server-side and bearer identity resolution in
  the kernel.
- Changed `PawApiClient` to use its configured bearer token on loopback and to
  synthesize no principal headers.
- Removed obsolete self-asserted admin headers from setup and startup clients.

## Verification Flow

1. Wrote rejection and credential-binding tests against the vulnerable PR.
2. Confirmed both loopback auth tests failed: self-assertion returned 200 and all
   nine forged identity-family headers survived.
3. Confirmed both pure transport request tests failed because the client emitted
   raw admin headers and omitted bearer auth.
4. Implemented the credential-only boundary.
5. Ran the complete auth test module, the transport header/trace suite, the full
   TemperPaw crate test suite, formatting, diff-check, and Clippy with warnings
   denied.

## Verification Results

| Step | Expected | Actual | Status |
|------|----------|--------|--------|
| Red: loopback self-assertion | New tests fail on old behavior | 200 vs 401; 9 forged headers survived | Pass |
| Red: transport identity | New pure tests fail on old behavior | Raw admin headers present; bearer absent | Pass |
| Auth module | Credential-bound flows pass | 11 passed, 0 failed | Pass |
| Combined auth chain | Global bearer replaces forged identity | `admin/api-key-holder/default` | Pass |
| Transport request construction | No principal headers; bearer on loopback | 3 passed, 0 failed | Pass |
| Full TemperPaw crate suite | All tests pass | 77 unit tests plus all integration contract tests passed | Pass |
| Formatting / diff hygiene | No drift | `cargo fmt --check` and `git diff --check` passed | Pass |
| Clippy | No warnings | `-D warnings` passed for both crates/all targets | Pass |

## What Worked

- The full in-process Axum chain exercises TemperPaw ingress middleware followed
  by the real kernel bearer middleware, without a mock authentication decision.
- Session-cookie identity overrides forged client headers after stripping.
- The remediation deletes more code than it adds and leaves one credential path.

## Limitations

No deployed Railway/Datadog verification was performed because the PR must remain
open for human review.

## What Still Doesn't Work

- Push/update PR #452 and request Greptile as `nerdsane`.
- Run deployed verification after human review/merge.
- Deployed verification is intentionally pending human merge.

## Artifacts

- `cargo test --locked -p temperpaw auth::tests -- --nocapture`
- `cargo test --locked -p paw-transport tests::paw_api_client_ -- --nocapture`
- `cargo test --locked -p temperpaw`
- `cargo fmt --check`
- `git diff --check`
- `cargo clippy --locked -p temperpaw -p paw-transport --all-targets -- -D warnings`

## Architecture Diagram

```text
network peer (remote or loopback)
        |
        v
strip client identity + tenant headers
        |
        +-- valid session --> inject authenticated dashboard principal
        |
        +-- bearer token --> kernel resolves registered agent or API-key admin
        |
        `-- neither ------> 401 on protected routes
```
