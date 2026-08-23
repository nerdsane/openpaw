# ADR-0066: Typed Session Authentication Context

## Status

Accepted

## Context

Temper ADR-0157 removed `PreAuthenticatedRequest`, which combined an
in-process marker with raw principal headers. TemperPaw's verified local
session-cookie middleware still used that marker to pass dashboard authority
into the embedded Temper router. Pinning the scoped schema-routing fix together
with current Temper therefore exposes both a compile failure and an obsolete
authentication boundary.

Temper now supports composition through a tenant-bound
`AuthenticatedRequestContext` installed by trusted in-process outer
middleware. Internal WASM HTTP fallthrough uses Temper's single-use internal
bearer capability and no longer needs a header-only compatibility path.

## Decision

After verifying a TemperPaw local session cookie, the outer middleware creates
one typed authenticated context for the configured tenant and the verified
local administrator. It also keeps the existing principal headers for
TemperPaw-owned handlers, but those headers are compatibility metadata, not
Temper authority; Temper strips them at its own edge.

Header-only internal requests are no longer admitted. They must carry the
single-use internal invocation bearer minted by the pinned Temper runtime.
Normal tenant bearer credentials continue unchanged.

The development dependency pins exact reviewed Temper commit
`0190ce8995de1d62cefd1dfe9c39edd3d032aea4`, which contains both scoped
schema-routing merge `7e3c70dcc00f6e693a637b219d065e10ec862e87`
and Temper ADR-0176's typed outer-authentication primitive. This fork pin must
be replaced by an exact `nerdsane/temper` descendant with the same contracts
before production merge.

## Consequences

- Verified dashboard sessions preserve administrator behavior through a typed,
  tenant-bound authority value.
- Raw HTTP principal headers cannot bypass either TemperPaw or Temper auth.
- Legacy header-only internal callers fail closed instead of being promoted.
- Development images remain fork-pinned until the dependency is available in
  upstream Temper and the pin/regression suites pass again.
