# ADR-0066: Reject self-asserted principal headers at the ingress edge

## Status

Accepted.

## Context

The embedded dashboard/API auth middleware treated any request carrying both
`x-temper-principal-kind` and `x-temper-principal-id` as a
`PreAuthenticatedRequest`. No cookie or bearer token was required, so an
Internet caller could self-assert an admin principal and supply the tenant and
Cedar attributes that the kernel later trusted.

The first proposed repair stripped those headers only for non-loopback TCP
peers and retained header-only pre-authentication for loopback callers. That
does not establish identity. TemperPaw supports separate same-host processes,
including `paw-codex-worker`, and those processes run lower-trust tasks. Local
reverse proxies and server-side request paths can also terminate on loopback.
Any such caller could still select `admin` and bypass the bearer layer.

TemperPaw already creates a platform API key during startup. Internal startup
helpers and transports can use that existing bearer credential; agent workers
use registry-issued credentials. A second internal secret or a network-location
identity mechanism is unnecessary.

This is the TemperPaw instance of the systemic Class A self-asserted-identity
bypass tracked by ARN-167 under ARN-165. The kernel-side credential binding is
tracked separately by ARN-170.

## Decision

Identity at the TemperPaw HTTP edge is credential-derived for every network
peer, including loopback:

1. The outer middleware removes the complete client-assertable identity family
   from every request before public-path handling or authentication:
   `x-temper-*`, `x-agent-id`, and `x-tenant-id`.
2. A valid TemperPaw session cookie may inject the authenticated dashboard
   principal server-side and mark the request pre-authenticated.
3. A bearer credential passes to the kernel bearer middleware, which resolves
   either a registered agent identity or the platform API-key administrator.
   TemperPaw injects only its configured deployment tenant.
4. Requests with neither credential are rejected on protected paths regardless
   of their source address.
5. Internal `PawApiClient` traffic uses its configured bearer token on loopback
   exactly as it does remotely. It never synthesizes principal headers.

Connection metadata may still support logging or rate controls, but it grants no
authentication or authorization capability.

## Alternatives considered

- **Trust loopback principal headers.** Rejected because host/network placement
  is not an identity boundary and same-host lower-privilege executors are a
  supported topology.
- **Add a separate internal shared secret.** This would duplicate the existing
  bearer credential path, add distribution and rotation state, and create a
  second authentication implementation.
- **Use a Unix-domain socket for internal calls.** This can reduce exposure but
  still requires peer/credential authorization and would add platform-specific
  transport complexity. It remains optional defense in depth.

## Consequences

- Raw identity, delegation, scope, context, and tenant headers are untrusted at
  the edge even when the TCP peer is loopback.
- Internal callers must possess the already-generated API key or a registered
  scoped credential. No-key header-only compatibility is intentionally removed.
- Session and bearer flows remain the only authentication implementations.
- The runtime server no longer needs `ConnectInfo<SocketAddr>` solely for auth.
- A compromised local worker cannot become admin by changing request headers;
  its effective identity is the one bound to its credential.
