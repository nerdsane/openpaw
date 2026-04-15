# 047: Untangle Default Tenant

Date: 2026-04-15

## Goal

Remove the hidden `"default"` tenant coupling between OpenPaw and Temper so that:

- platform/bootstrap secrets live in a platform-scoped cache instead of a fake tenant
- OpenPaw writes tenant-owned secrets only to its configured tenant
- OpenPaw can still read older `"default"`-bucket data during migration
- Temper no longer grants platform privileges or storage routing to `"default"`
- hardcoded `rita-agents` examples are removed from active docs and tests

## What Was Done

### Temper

Implemented the platform-secret split and removed default-tenant special casing:

- added a platform secret cache to [`VaultStore`](</Users/seshendranalla/Development/temper/.claude/worktrees/untangle-tenant/crates/temper-server/src/secrets/vault.rs>)
- changed secret fallback/merge behavior to use platform secrets instead of `"default"`
- updated CLI boot seeding in [`serve/mod.rs`](</Users/seshendranalla/Development/temper/.claude/worktrees/untangle-tenant/crates/temper-cli/src/serve/mod.rs>)
- removed `"default"` bypassing from [`tenant_access.rs`](</Users/seshendranalla/Development/temper/.claude/worktrees/untangle-tenant/crates/temper-platform/src/tenant_access.rs>) and Turso routing in [`router.rs`](</Users/seshendranalla/Development/temper/.claude/worktrees/untangle-tenant/crates/temper-store-turso/src/router.rs>)
- added ADR [`0044-platform-secrets-untangle-default-tenant.md`](</Users/seshendranalla/Development/temper/.claude/worktrees/untangle-tenant/docs/adrs/0044-platform-secrets-untangle-default-tenant.md>)
- removed active `rita-agents` examples from docs/tests that should stay tenant-generic

### OpenPaw

Reworked startup/auth/setup flows around the configured tenant:

- startup now seeds platform secrets into the Temper platform cache and persists tenant-owned values only to the configured tenant in [`startup.rs`](</Users/seshendranalla/Development/openpaw/.claude/worktrees/untangle-tenant/crates/openpaw/src/startup.rs>)
- auth middleware now injects the configured tenant instead of hardcoding `"default"` in [`auth.rs`](</Users/seshendranalla/Development/openpaw/.claude/worktrees/untangle-tenant/crates/openpaw/src/auth.rs>)
- setup APIs now rely on Temper’s tenant+platform secret resolution instead of dual-reading/writing `"default"` in [`setup_api.rs`](</Users/seshendranalla/Development/openpaw/.claude/worktrees/untangle-tenant/crates/openpaw/src/setup_api.rs>)
- added ADR [`0033-untangle-default-tenant.md`](</Users/seshendranalla/Development/openpaw/.claude/worktrees/untangle-tenant/docs/adrs/0033-untangle-default-tenant.md>)
- documented `PAW_TENANT` in [`.env.example`](</Users/seshendranalla/Development/openpaw/.claude/worktrees/untangle-tenant/.env.example>)
- removed active `rita-agents` test references from the Paw agent e2e shell test

### Integration Fix

OpenPaw’s local Temper patch exposed a missing public API: `temper_platform::os_apps::git_sources`.

To keep `TEMPER_APP_SOURCES` startup behavior working with the patched Temper worktree, I restored that compatibility module in:

- [`os_apps/mod.rs`](</Users/seshendranalla/Development/temper/.claude/worktrees/untangle-tenant/crates/temper-platform/src/os_apps/mod.rs>)
- [`os_apps/git_sources.rs`](</Users/seshendranalla/Development/temper/.claude/worktrees/untangle-tenant/crates/temper-platform/src/os_apps/git_sources.rs>)

## Verification Flow

1. Ran a broad `cargo test --workspace` sweep in the Temper worktree and confirmed the updated vault coverage and large DST suites were passing before the final integration shim surfaced.
2. Hit an OpenPaw compile failure caused by the missing Temper `git_sources` module.
3. Restored the Temper compatibility module and reformatted.
4. Re-ran focused Temper platform tests for the restored module.
5. Re-ran OpenPaw Rust tests.
6. Installed dashboard dependencies with `npm ci` and produced a production build with `npm run build`.
7. Booted `openpaw-server` locally against a file-backed Turso database using `PAW_TENANT=test-tenant` and a temporary Cargo patch config pointing at the Temper worktree.
8. Performed a runtime smoke:
   - `GET /healthz`
   - `GET /paw/setup/status`
   - `POST /paw/setup/secrets` for `slack_bot_token`
   - `GET /paw/setup/secrets`
   - `GET /paw/setup/secrets/slack_bot_token`
   - `POST /auth/register`
   - authenticated `GET /tdata/Agents?$top=1`
   - server restart
   - repeated `GET /paw/setup/status`, `GET /paw/setup/secrets`, and `GET /paw/setup/secrets/slack_bot_token`

## Verification Results

| Step | Expected | Actual | Status |
|------|----------|--------|--------|
| `cargo test -p temper-platform git_sources` | Restored Temper API compiles and git-source parser tests pass | 12 tests passed | PASS |
| `cargo test -p openpaw` | OpenPaw compiles against patched Temper and tenant-aware startup/auth tests stay green | 13 tests passed | PASS |
| `npm ci` in `dashboard/` | Frontend deps install from lockfile | Install completed successfully | PASS |
| `npm run build` in `dashboard/` | Dashboard still produces a production build | SvelteKit/Vite build completed successfully | PASS |
| Local `openpaw-server` runtime smoke | Tenant-aware setup/auth flow works across restart on the publishable tree shape | Health check passed, `slack_bot_token` persisted/restored, authenticated `tdata` call returned `200` | PASS |

## What Worked

- Platform bootstrap secrets now resolve through Temper’s platform cache instead of a hidden `"default"` tenant.
- Tenant-scoped secret persistence in OpenPaw no longer dual-writes or dual-deletes `"default"`.
- Legacy `"default"` data still has a migration read path during startup/account restore.
- Temper platform authorization and Turso routing no longer treat `"default"` as privileged.
- OpenPaw compiles and tests cleanly against the patched Temper worktree after restoring the missing git-source compatibility surface.
- The dashboard still builds successfully after the tenant untangling changes.
- A real local runtime smoke passed with `PAW_TENANT=test-tenant`, including secret persistence across restart and an authenticated `tdata` request.

## What Didn't Work

- The first dashboard build attempt failed because the worktree did not have frontend dependencies installed yet, so `vite` was not on `PATH`.
- The first OpenPaw Rust test run failed because the patched Temper worktree no longer exported `temper_platform::os_apps::git_sources`.

## Limitations

- OpenPaw still carries a local Cargo `[patch."https://github.com/nerdsane/temper.git"]` entry in [`Cargo.toml`](</Users/seshendranalla/Development/openpaw/.claude/worktrees/untangle-tenant/Cargo.toml>) so the worktree can compile against the Temper changes before they are merged upstream.
- After restoring the Temper `git_sources` module, I re-ran focused Temper platform coverage for that restored surface rather than repeating the entire long workspace sweep from scratch.
- Historical artifacts that intentionally reference `rita-agents` were left alone where they serve as archived evidence rather than active docs/runtime guidance.

## What Still Doesn't Work

- OpenPaw is not yet independent of the local Temper patch; removing that patch before the Temper branch lands upstream would break local compilation again.

## Artifacts

- OpenPaw branch: `feat/untangle-default-tenant` at `bf96e4d2`
- Temper branch: `feat/platform-secrets-untangle-default-tenant` at `6c73cd3`
- ADRs:
  - [`docs/adrs/0033-untangle-default-tenant.md`](</Users/seshendranalla/Development/openpaw/.claude/worktrees/untangle-tenant/docs/adrs/0033-untangle-default-tenant.md>)
  - [`docs/adrs/0044-platform-secrets-untangle-default-tenant.md`](</Users/seshendranalla/Development/temper/.claude/worktrees/untangle-tenant/docs/adrs/0044-platform-secrets-untangle-default-tenant.md>)

## Architecture Diagram

```text
                 startup env / setup API / restored tenant rows
                                   |
                                   v
                        +------------------------+
                        |   OpenPaw startup      |
                        |  configured PAW_TENANT |
                        +-----------+------------+
                                    |
                     platform cache | tenant persistence
                                    |
               +--------------------+--------------------+
               v                                         v
    +----------------------+                 +----------------------+
    | Temper VaultStore    |                 | Turso secret rows    |
    | platform secrets     |                 | tenant-owned secrets |
    +----------+-----------+                 +----------+-----------+
               |                                         |
               +--------------------+--------------------+
                                    v
                        secret resolution at runtime
                  tenant override -> platform fallback
```
