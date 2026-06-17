# Incident: prod Genesis cannot serve app bundles (instance-wide)

**Status:** open · **Severity:** high (blocks all app install/update on prod) · **Found:** 2026-06-15 during the paw-foresight corridor deploy.

## One line
Prod Genesis (`https://genesis-production-164d.up.railway.app`) has the git **refs** for every TemperPaw app but its **blob/object store is missing the objects**, so the bundle-fetch API 404s for *every* app — including apps that are currently installed and running. No app can be installed or updated via the normal pinned-ref path; running apps survive only because openpaw recovers them durably from Postgres.

## Symptom (exact errors)
openpaw boot reconcile (`service:temperpaw env:prod`) logs, per app:
```
INFO  Reconciling changed Genesis bootstrap app   (previous_ref=…01ac826  next_ref=…1d2f6b8)
WARN  Genesis bootstrap install/reconcile failed; continuing startup with durable app recovery
      error: Genesis bundle fetch failed … returned 404 Not Found:
             {"error":"Genesis blob 013224e715c9342ffa6d31976aceed620c8539fb not found for rp-temperpaw-paw-foresight"}
             Git fallback is disabled; set TEMPER_GENESIS_INSTALL_GIT_FALLBACK=1 only for admin/debug recovery.
```
The same WARN fired for **14 apps** on the same boot. Two distinct sub-errors:
- New commit (paw-foresight@1d2f6b8): `Genesis blob <sha> not found for rp-temperpaw-paw-foresight`
- Long-published apps (paw-agent, paw-pm, paw-patrol, paw-channels, paw-ingest, paw-compute, paw-heal, paw-harness, paw-managed-agents, paw-wiki, paw-skills, paw-autoreason, paw-consilium, katagami-curation): `Genesis commit <sha> not found for rp-temperpaw-<app>`

## Reproduce (no auth needed)
```bash
GEN=https://genesis-production-164d.up.railway.app
# git refs EXIST (read works):
git ls-remote $GEN/temperpaw/paw-foresight.git main
#   -> 1d2f6b831632da50b0696011ecdc1d310b058005   refs/heads/main
# but the bundle API 404s, even for a running app:
curl -s "$GEN/api/genesis/apps/temperpaw/paw-agent/versions/69aaa6bc935ec6e11d074b4382abc5161d7727de/bundle"
#   -> {"error":"Genesis commit 69aaa6… not found for rp-temperpaw-paw-agent"}
curl -s "$GEN/api/genesis/apps/temperpaw/paw-foresight/versions/1d2f6b831632da50b0696011ecdc1d310b058005/bundle"
#   -> {"error":"Genesis blob 013224e7… not found for rp-temperpaw-paw-foresight"}
```
Note: a git **push** to Genesis still ingests fine — Datadog shows
`app usage: Repository.IngestPack Active -> Active on rp-temperpaw-paw-foresight succeeded`.
So **git ingest works; bundle/blob materialization + serving is what's broken.**

## Impact
- `App.Install` / pinned-ref `install_app` / boot bootstrap all fail for every app.
- The 18 bootstrap apps keep running only via openpaw's durable Postgres recovery; a cold boot (empty DB) would bring up **nothing**.
- Cannot complete the corridor deploy: specs install (via git-fallback) but **wasm, Cedar policies, and agent souls do not** (git-fallback is specs-only). Creating a `World` on prod is Cedar-denied because the foresight policies never installed.

## Prime suspect — "what changed"
The Genesis codebase checkout `~/Development/temper-git` is on branch
**`codex/genesis-install-performance-20260525`**. A blob/bundle-storage regression
from that *install-performance* work (e.g. a blob GC, a changed object-store write
path, or a pack/bundle materialization change) is the leading hypothesis. Diff that
branch against the last-known-good Genesis and look for changes to blob persistence
or bundle assembly.

## Where to look (in `~/Development/temper-git`)
- `wasm/app_registry/src/lib.rs` — the Genesis registry app (version/bundle surface).
- `temper/crates/temper-platform/src/genesis_install.rs` — install + bundle-fetch client side.
- `temper/crates/temper-store-turso/src/store/blobs.rs` — **blob storage** (the "blob not found" almost certainly originates here or in the bundle assembler that reads it).
- Search: `grep -rn "not found for" temper/ wasm/` to find the exact error site.

## Hypotheses to test
1. **Blobs never persisted / were GC'd**: refs point at trees/blobs absent from the blob table. Check whether Genesis's blob store has rows for `rp-temperpaw-paw-agent`'s objects.
2. **Repo-store migration**: refs were copied/migrated to `genesis-production-164d` but the object store wasn't — apps were originally published to a different/older Genesis.
3. **Bundle assembler regression**: the install-performance branch changed how bundles resolve objects (wrong repo key, wrong blob lookup), so lookups miss objects that are present.
4. **Bundle never materialized for new pushes**: a raw `git push` ingests the pack but a separate `App.PublishNewVersion`/materialize step (which builds the servable bundle) is required and wasn't run. (Explains the *new* commit's "blob not found"; does NOT explain old running apps 404ing — so #1/#2/#3 likely dominate.)

## Coordinates
- Genesis registry: `https://genesis-production-164d.up.railway.app` (Railway — confirm which project/service via `railway list`; likely the `temper` project).
- openpaw (consumer): Railway project `openpaw-seshendranalla` / env `production` / service `openpaw`; public `https://openpaw-production.up.railway.app` (custom domain `temperpaw.katagami.ai` currently misrouting — separate issue).
- openpaw bootstrap var `TEMPERPAW_GENESIS_BOOTSTRAP_REFS` pins 18 apps; `TEMPERPAW_GENESIS_REGISTRY_URL=https://genesis-production-164d.up.railway.app`, tenant `default`.
- Datadog: `service:temperpaw env:prod`, reconcile WARNs at 2026-06-15 ~18:05 and ~18:25 UTC.

## Temporary mitigation already applied (to remove once Genesis is fixed)
- Set `TEMPER_GENESIS_INSTALL_GIT_FALLBACK=1` on the **openpaw** service so installs fall back to git (specs only). This is the documented admin/debug recovery; it does **not** deliver wasm/policies/souls, so it is not a real fix. **Once Genesis bundle serving is repaired, remove this flag** and run a clean `install_app temperpaw/paw-foresight@1d2f6b8…`.

## Corridor-deploy state (context for why this matters now)
- paw-foresight corridor (app.toml 0.2.1) is **pushed to Genesis git** at `temperpaw/paw-foresight@1d2f6b831632da50b0696011ecdc1d310b058005`.
- On openpaw `default` tenant: corridor **specs** installed + **wasm** hand-uploaded (13 modules, `sample_endpoints=8360a1ef`), but **policies + souls missing** → engine can't run yet. A clean bundle install would finish it. See `DEPLOY-RUNBOOK.md` in this dir.
