# ADR-0033: Untangle OpenPaw from the legacy default tenant

**Status:** Accepted
**Date:** 2026-04-15
**Related:** ADR-0032 (TemperFS Agent Operations), Temper ADR-0044 (Platform secrets layer and default-tenant untangling)

## Context

OpenPaw inherited Temper's old `"default"` tenant magic in three places:

- startup seeded every infra secret into both `"default"` and the configured tenant
- the dashboard auth middleware injected `"default"` whenever a request had no tenant header
- the setup API wrote and deleted secrets in both buckets

That meant a non-default `PAW_TENANT` still depended on `"default"` existing for bootstrap behavior. After Temper's vault grows an explicit platform secrets layer, OpenPaw should stop dual-writing and treat `"default"` like any other tenant name.

## Decision

### 1. Startup secrets use the platform cache

OpenPaw now caches shared infra secrets into Temper's platform secrets layer while still persisting them under the configured tenant in Turso. This keeps the current storage schema and removes the in-memory dependency on `"default"`.

### 2. Auth middleware injects the configured tenant

`ensure_tenant_header` now uses `PAW_TENANT` (from `Config`) instead of hardcoding `"default"`. Local auth account storage also writes to the configured tenant, with a read-only migration fallback to the legacy `"default"` bucket.

### 3. Setup API writes only to the configured tenant

The setup endpoints no longer dual-write or dual-delete secrets in both `"default"` and the configured tenant. Reads go through the vault's normal fallback path, which now includes platform secrets automatically.

### 4. Backward compatibility is handled at restore time

Startup restores shared secrets from the configured tenant bucket into the platform cache first. If the configured tenant is not `"default"`, it then restores from the legacy `"default"` bucket as a migration shim. The first restored value wins, so the configured tenant overrides old leftovers.

## Consequences

- New OpenPaw deployments never need a `"default"` tenant bucket just to boot.
- Existing deployments that still have infra secrets under `"default"` continue to work after startup restore.
- The dashboard and bearer/session auth now consistently target the configured tenant.
- The temporary local `[patch]` in the workspace `Cargo.toml` is required until the Temper changes merge upstream.
