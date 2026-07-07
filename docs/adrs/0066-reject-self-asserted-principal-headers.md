# ADR-0066: Reject self-asserted principal headers at the ingress edge

## Status

Accepted.

## Context

The embedded dashboard/API auth middleware (`crates/temperpaw/src/auth.rs`)
treated **any** request carrying both `x-temper-principal-kind` and
`x-temper-principal-id` as a `PreAuthenticatedRequest` and passed the
client-supplied principal downstream — with no cookie, no bearer token, and no
check that the caller was actually internal. The code comment claimed the path
was for "internal WASM agent calls," but nothing verified the call was internal.

The production server binds `0.0.0.0` and Railway deploys it as the network
edge with no header-stripping proxy in front of it. Client-supplied
`x-temper-*` headers therefore reached the app untouched. Any external caller
could self-assert an admin principal:

```
curl https://<host>/tdata/<EntitySet> \
  -H 'x-temper-principal-kind: admin' \
  -H 'x-temper-principal-id: attacker' \
  -H 'x-tenant-id: default'
```

Cedar was the only remaining gate, and the client also controlled `x-tenant-id`.
This is the TemperPaw instance of the systemic **Class A** self-asserted-identity
bypass (epic ARN-165); the kernel counterpart was ARN-170 (temper PR #343).
Tracked as ARN-167.

Legitimate internal callers already model "internal" as **loopback**: the
transport client (`crates/paw-transport`), setup, startup, and observer callers
all target `http://127.0.0.1:{port}` and only attach admin principal headers on
loopback URLs (`PawApiClient::uses_internal_loopback`). Remote agents/workers
authenticate with a Bearer token. So the property that distinguishes an internal
caller from an external one is the real TCP peer being loopback — something a
remote client cannot forge.

## Decision

Identity for external requests is derived **only** from a resolved credential;
self-asserted identity headers are never trusted from a remote peer. Two changes
implement this at the ingress edge:

1. **Strip client-asserted identity headers from every non-loopback request.**
   Before any downstream logic (Cedar, the kernel `bearer_auth_check`) can read
   them, the middleware removes the **entire `x-temper-*` header family** plus
   `x-agent-id` and `x-tenant-id` from any request whose TCP peer is not
   loopback. The whole prefix is stripped — not a hardcoded subset — because the
   kernel's Cedar principal builder (`SecurityContext::from_headers` in
   temper-authz) trusts far more than principal id/kind: it derives
   `principal.role` from `x-temper-agent-role`, delegation from
   `x-temper-acting-for`, scopes from `x-temper-principal-scopes`, arbitrary
   principal attributes from `x-temper-attr-*`, and Cedar context attributes
   (including `agentTypeVerified`) from `x-temper-ctx-*`. TemperPaw is the sole
   network edge in front of the kernel and the kernel does not itself strip these,
   so anything less than a full-family strip would leave a privilege-escalation
   surface for any caller holding a valid low-privilege credential. The tenant and
   principal are then re-derived server-side from the resolved credential (session
   cookie injects the admin principal; bearer is resolved by the kernel), and the
   deployment tenant is forced via `ensure_tenant_header`.

2. **Honor the header-only "internal" path only for genuinely loopback peers.**
   The branch that marks a request `PreAuthenticated` purely from the presence of
   principal headers now additionally requires the request to originate from a
   loopback peer. The peer is read from the real connection address
   (`ConnectInfo<SocketAddr>`), never from a client-supplied forwarding header.
   To make that address available, the runtime server is now served with
   `into_make_service_with_connect_info::<SocketAddr>()`. When connection info is
   absent (e.g. in-process test transports) the request is treated as **not**
   loopback — the safe default.

External callers therefore have no way to reach the pre-authenticated branch with
forged identity: their headers are stripped, and they are not loopback. Internal
loopback callers and cookie/bearer-authenticated callers are unaffected.

## Alternatives considered

- **Internal shared secret / in-process marker instead of a loopback check.**
  Topology-independent and forge-proof regardless of network layout, but it
  requires threading a startup-generated secret through every internal caller,
  including the separate `paw-codex-worker` process, for a larger change surface.
  The loopback check reuses the convention internal callers already follow
  (`uses_internal_loopback`) and needs no secret distribution. A shared secret
  remains a viable hardening follow-up if the internal-call topology ever stops
  being loopback.

## Consequences

- Remote requests must present a session cookie or a Bearer token; self-asserted
  `x-temper-*`/`x-tenant-id` headers from remote peers are ignored (stripped).
- In-container loopback callers (transport, setup, startup, observer) continue to
  work unchanged because their real peer is `127.0.0.1`/`::1`.
- The runtime server now propagates connection info; this is additive and does
  not change routing.
- **Residual risk:** the loopback check assumes external traffic never reaches the
  app over a loopback peer. This holds on the current Railway topology (the app is
  the edge; no same-container proxy forwards over loopback). If that topology
  changes, or a `paw-codex-worker` runs remotely without a Bearer token, the
  shared-secret hardening above should be adopted.
