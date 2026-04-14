# 046: Restart Healing And Full Capability Smoke

Date: 2026-04-14

## Goal

Verify that the startup-surface and lazy-WASM optimizations did not cut off real OpenPaw or Temper functionality.

This proof specifically checks:

- persisted app specs still survive restart
- startup no longer leaves app specs stuck in `pending`
- system skills still bootstrap and remain readable
- core file APIs still work end to end
- manual app install still works after restart
- installed-app entity creation and action dispatch still work after restart
- persisted WASM still lazy-compiles on first use
- actors, entities, and memory stay bounded in an isolated run

## Regression Found

During the full smoke matrix, restart exposed a real regression:

- after reinstall/restart, creating a new `WebQuery` returned `423 VerificationRequired`
- live app specs were present, but some durable rows had been re-verified and left `committed = 0`
- restart recovery only restores committed specs, so app entity sets could disappear even though the app looked installed

Root cause:

- `temper-platform` install flow persisted spec rows and committed them
- later, `persist_bootstrap_verification(...)` called `upsert_spec(...)`
- `upsert_spec(...)` resets the row to uncommitted while rewriting content
- bootstrap verification persisted status, but did not recommit the tenant's spec set

Result: the platform could report a stable verification status in memory while the durable row was still uncommitted and therefore not restart-visible.

## Code Changes

### Durable recommit after bootstrap verification

`persist_bootstrap_verification(...)` now calls `commit_specs(tenant)` after verification persistence, so bootstrap-healed rows are durably visible on restart.

Relevant file:

- `/Users/seshendranalla/Development/temper/crates/temper-platform/src/bootstrap.rs`

### Recovery now heals pending app specs

App restore now skips reinstall only when the app's specs are both present and in a stable verification state (`Completed` or `Restored`).

Pending specs are explicitly healed through the normal install/bootstrap path.

Relevant file:

- `/Users/seshendranalla/Development/temper/crates/temper-platform/src/recovery.rs`

### Install flow now rehydrates the full app schema surface

App install/bootstrap now merges the full bundle schema back into memory and persists bootstrap verification for the full app spec set, even when individual spec files are byte-for-byte unchanged.

This preserves entity-set mappings and allows restart recovery to heal partially-restored apps.

Relevant file:

- `/Users/seshendranalla/Development/temper/crates/temper-platform/src/os_apps/mod.rs`

### Regression tests added

Added or strengthened tests to cover:

- stable skill/app install across restart
- healing app specs left in `pending`

Relevant file:

- `/Users/seshendranalla/Development/temper/crates/temper-platform/src/os_apps/mod_test.rs`

## Red-Green Verification

### Focused regression tests

Passed:

```bash
cargo test -p temper-platform test_skill_install_survives_restart -- --nocapture
cargo test -p temper-platform test_restore_installed_app_heals_pending_specs_on_restart -- --nocapture
```

### Full affected crate verification

Passed:

```bash
cargo test -p temper-platform -- --nocapture
cargo build -p openpaw --release
```

The `temper-platform` crate run includes the heavy DST and integration coverage for install, restart, and platform recovery paths.

## End-To-End Isolated Smoke Matrix

### Server setup

Started a fresh isolated release OpenPaw instance with:

- dedicated temp `HOME`
- `OPENPAW_WASM_STARTUP_POLICY=load-only`
- `DD_ENV=isolated-capability-check`
- one process only

The process rebound to port `59309` because the default port was occupied.

### Health

Passed:

```http
GET /healthz
```

Response: `200`

### Core entity APIs still work

Passed:

```http
GET /tdata/Agents?$top=1
GET /tdata/Files?$top=1
```

Both returned `200`.

### System skills still load

Passed:

```http
GET /tdata/Files?$filter=startswith(path,'/system/skills/') and name eq 'SKILL.md' and Status ne 'Archived'&$top=5
```

Returned multiple system skill files.

Then fetched one skill body directly:

```http
GET /tdata/Files('os-sys-skill-file-platform-awareness')/$value
```

Returned `200` with the expected skill markdown content.

### File round-trip still works

Passed:

1. Created a file entity via `POST /tdata/Files`
2. Uploaded content via `PUT /tdata/Files('<id>')/$value`
3. Read content back via `GET /tdata/Files('<id>')/$value`

Observed exact content round-trip with `200/204` responses.

### Manual app catalog still works

Passed:

```http
GET /api/os-apps
```

Returned `200` and included manual-install apps such as `paw-research`.

### Manual app install still works after restart

Passed:

```http
POST /api/os-apps/paw-research/install
```

Response: `200`

Observed install result:

```json
{
  "app": "paw-research",
  "tenant": "default",
  "added": [],
  "updated": [],
  "skipped": ["WebQuery"],
  "status": "installed"
}
```

This was the healthy path: the app already existed durably, restart recovery left it in a stable state, and reinstall became a no-op instead of breaking the entity set.

### Installed app entity set still exists after restart

Passed:

```http
GET /tdata/WebQueries?$top=1
```

Returned `200`.

This is the exact surface that previously broke with `423 VerificationRequired`.

### Installed app action dispatch still works after restart

Passed:

1. Created a new `WebQuery` entity after restart
2. Dispatched `Temper.ExecuteSearch?await_integration=true`
3. Polled the entity state

Observed:

- create returned `201`
- dispatch returned `200`
- the entity transitioned through `Created -> Executing -> Failed`
- failure reason was the expected missing external secret (`exa_api_key`), not a platform/spec verification problem

This proves:

- entity creation still works
- bound actions still resolve
- persisted app specs still load after restart
- lazy persisted WASM dispatch still works

### Lazy persisted WASM compile confirmed live

Server log showed:

```text
lazy-compiled persisted WASM module on first use tenant=default module=web_search
```

This confirms the optimized startup path is active and the module still compiles correctly on first invocation.

## Datadog Verification

Queried isolated metrics for `service:openpaw env:isolated-capability-check`.

Observed maxima during the run:

- `temper_active_actors`: `12`
- `temper_indexed_entities`: `223`
- `temper_projected_entities`: `223`
- `process_resident_memory_bytes`: `284327936`

Interpretation:

- actors stayed bounded
- indexed and projected entities matched
- isolated process RSS remained under roughly `271 MB`

Also queried logs for:

```text
service:openpaw env:isolated-capability-check (status:error OR status:critical OR status:alert OR status:emergency OR @http.status_code:423)
```

There was noise from normal actor stop/rebuild logs, but there was no recurrence of the old `423` restart failure in the fixed run.

## Local Process Snapshot

For the isolated release server after the successful smoke matrix:

- RSS from `ps`: `277664 KB`
- macOS physical footprint from `vmmap -summary`: `96.3M`
- peak physical footprint: `112.7M`

Persisted module corpus in the isolated DB:

- `28` modules total
- largest module: `monty_repl` at about `6.7 MB`

This confirms the remaining baseline-memory story is about installed module/app surface, not actor explosion.

## Conclusion

The startup and lazy-WASM optimizations did not silently cut off the product surface.

After fixing the restart-healing bug:

- restart recovery works
- system skills still load
- file APIs still work
- manual app install still works
- installed-app entity creation still works after restart
- lazy persisted WASM compile still works
- actors and memory stay bounded in an isolated run

The real regression found during this verification was durable-spec recommit after bootstrap verification. That is now fixed and covered by regression tests.
