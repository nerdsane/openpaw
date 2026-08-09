# ARN-167: Credential-bound TemperPaw ingress identity

## Objective

Reject every client-asserted identity at the TemperPaw HTTP edge, including
loopback callers, while preserving internal transport and startup behavior
through credential-derived identity.

## Plan

1. Revise ADR-0066 so network location is defense in depth, never proof of
   identity.
2. Add red tests proving loopback self-assertion is rejected and loopback API
   clients use bearer credentials without principal headers.
3. Remove the loopback pre-authentication branch and its connection-info
   plumbing; strip the entire client-assertable identity family on every
   request before resolving a session or bearer credential.
4. Remove obsolete client-side loopback identity synthesis.
5. Run focused tests, full affected-crate tests, a live local HTTP flow, Clippy,
   formatting, and independent review.

## Acceptance criteria

- Loopback plus raw admin headers and no credential returns 401.
- Remote and loopback requests cannot preserve forged identity attributes.
- A valid session is injected server-side after stripping.
- Internal `PawApiClient` calls use the configured bearer token on loopback.
- No IP/hostname allowlist or duplicate authentication implementation remains.
