# Proof Report: 040 — Dashboard Auth and Paw Soul E2E

## Date
2026-04-13

## Branch / Commit
- **openpaw**: `main` (`a8fdc585`)
- **temper**: `feat/governance-decision-callbacks` (`e74dd6f`)

## What Was Done
Closed the remaining gaps in the dashboard-authenticated setup flow and verified them end-to-end:

1. Fixed Paw soul resolution to use the active Paw agent's attached `soul_id` first, with legacy name-based fallback for older data.
2. Fixed Paw soul save to update the existing soul content file instead of relying on a fragile `name eq 'Paw'` lookup.
3. Fixed dashboard-authenticated `/tdata/*` access by extending Temper with an internal `PreAuthenticatedRequest` marker so OpenPaw's cookie auth can safely satisfy Temper's bearer boundary without spoofable headers.
4. Updated OpenPaw auth middleware to set the trusted pre-auth marker only after a valid dashboard session cookie is decoded.
5. Rebuilt the release binary and re-verified the live dashboard + setup flow on a clean-room HOME directory.

## Root Cause
There were two separate failures:

1. **Soul lookup mismatch**
   - Live data used mixed field conventions across Soul entities (`Name` vs `name`).
   - The active Paw agent already carried the correct `soul_id`, but the setup API was querying by soul name only.
   - Result: `/paw/setup/soul` returned `404 {"error":"Paw soul not found"}` even though the soul existed.

2. **Platform auth boundary mismatch**
   - OpenPaw dashboard cookie auth injected trusted principal headers.
   - Temper's `bearer_auth_check` still required `Authorization: Bearer ...` even for already-authenticated internal requests.
   - Result: cookie-authenticated `/tdata/*` requests returned `401`, which broke the setup API's internal OData calls.

## Red-Green TDD
### Red
- Added a new Temper test:
  - `bearer_auth::tests::pre_authenticated_request_bypasses_bearer_requirement`
- This codified the missing platform behavior: a request pre-authenticated by an outer middleware layer should be allowed through Temper when it carries a trusted in-process marker plus principal headers.

### Green
- Added `temper_platform::bearer_auth::PreAuthenticatedRequest`.
- Updated Temper's bearer auth middleware to allow requests carrying that marker and trusted principal headers.
- Updated OpenPaw auth middleware to insert the marker only after successful cookie-session decoding.
- Re-ran the targeted Temper and OpenPaw auth tests successfully.

## Files Changed
### OpenPaw
- `crates/openpaw/src/auth.rs`
- `crates/openpaw/src/setup.rs`
- `crates/openpaw/src/setup_api.rs`
- `crates/openpaw/src/startup.rs`

### Temper
- `../temper/crates/temper-platform/src/bearer_auth.rs`

## Verification Flow
1. Run focused Temper auth regression test.
2. Run focused OpenPaw auth regression test.
3. Run full OpenPaw package tests.
4. Build the dashboard production bundle.
5. Run `svelte-check`.
6. Build the `openpaw` release binary.
7. Restart `openpaw` on a clean-room HOME dir using the rebuilt binary.
8. Verify health and SPA login route.
9. Verify cookie-authenticated `/tdata/*` access.
10. Verify `/paw/setup/soul` returns the live Paw soul.
11. Save a sentinel soul payload through `/paw/setup/soul/save`.
12. Read the soul back and confirm the sentinel content persisted.
13. Confirm the Soul entity count remains stable, proving we updated the existing soul instead of creating a duplicate.

## Verification Results
| Step | Expected | Actual | Status |
|------|----------|--------|--------|
| `cargo test --manifest-path ../temper/Cargo.toml -p temper-platform pre_authenticated_request_bypasses_bearer_requirement -- --nocapture` | New platform auth regression passes | 1 passed | PASS |
| `cargo test -p openpaw tdata_subpaths_accept_cookie_sessions -- --nocapture` | OpenPaw cookie-auth regression passes | 1 passed | PASS |
| `cargo test -p openpaw --quiet` | OpenPaw package tests pass | 14 passed | PASS |
| `cd dashboard && npm run build` | Dashboard production build succeeds | PASS, with pre-existing unused CSS warnings in `src/routes/+page.svelte` | PASS |
| `cd dashboard && npm run check` | Svelte diagnostics succeed | 0 errors, 9 warnings (same unused CSS selectors) | PASS |
| `cargo build -p openpaw --release` | Release binary builds successfully | PASS | PASS |
| `GET /healthz` | Release server is healthy | `200` | PASS |
| `GET /dashboard/login` | SPA route resolves after static serving changes | `200` | PASS |
| Cookie-auth `GET /tdata/Agents?$filter=name eq 'Paw' and Status eq 'Active'` | Dashboard session can reach `/tdata/*` | `200`; returned active Paw agent and attached `soul_id` | PASS |
| Cookie-auth `GET /paw/setup/soul` | Paw soul loads through setup API | `200`; returned live summary/content | PASS |
| Cookie-auth `POST /paw/setup/soul/save` | Personalized soul save updates existing soul | `200 {"saved":true}` | PASS |
| Cookie-auth `GET /paw/setup/soul` after save | Saved content is visible immediately | Returned sentinel summary `Verification sentinel 2026-04-13T21:47Z.` | PASS |
| `GET /tdata/Souls` after save | No duplicate soul created | `souls_count=100` (unchanged) | PASS |

## Key Live Evidence
- Pre-fix live failure:
  - cookie-authenticated `GET /tdata/Agents?...` returned `401`
  - cookie-authenticated `GET /paw/setup/soul` returned `404 {"error":"Paw soul not found"}`
- Post-fix live success:
  - cookie-authenticated `GET /tdata/Agents?...` returned `200`
  - active Paw agent: `aj-019d88ab-6bc0-71e1-ba8d-818d3b04a86d`
  - attached Paw soul: `sl-019d88b5-b211-7360-9c0b-0db34f9ad310`
  - soul count after restart and after save remained `100`

## Notes
- The end-to-end save verification intentionally wrote a sentinel soul payload into the isolated clean-room HOME directory:
  - `/tmp/openpaw-release-e2e.d3mhIN`
- This verification did not touch the developer's normal OpenPaw data directory.
- Cargo continued to emit pre-existing patch warnings about local Temper crates not used in the current crate graph; these warnings did not block test or release verification.

## Artifacts
- Clean-room HOME: `/tmp/openpaw-release-e2e.d3mhIN`
- Clean-room cookie jar: `/tmp/openpaw-cookie.jar`
- Release binary: `/Users/seshendranalla/Development/openpaw-codex/target/release/openpaw`
