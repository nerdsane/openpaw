# Corridor deploy runbook (C7) — resume after Claude Code restart

> Written 2026-06-15 mid-deploy. Rita is restarting Claude Code so the **temper
> MCP** repoints from the stale `api.temper.build` bridge to **openpaw**
> (the global `~/.claude.json` temper server already has
> `--url https://openpaw-production.up.railway.app` + `TEMPER_API_KEY`).
> This project uses the global config (no project override), so after restart
> the MCP → openpaw. Resume here.

## Decisions locked (Rita, 2026-06-15)
- **Deploy to Railway/Postgres now** (clean run env + advances C7).
- **Replace** prod paw-foresight v0.1 → corridor (still verify no live v0.1 data first).
- Publish via the **temper MCP** (`publish_app`) once it points at openpaw.

## Verified facts (no guessing)
- Prod openpaw is UP/healthy: `https://openpaw-production.up.railway.app` (readyz/healthz 200).
  Custom domain `temperpaw.katagami.ai` is misrouting (separate DNS/cert fix — not blocking).
- Latest deploy SUCCESS `e7ccbe1f` (2026-06-15 10:27).
- Railway project `openpaw-seshendranalla` / env `production` / service `openpaw` (worktree is linked).
- Genesis registry: `https://genesis-production-164d.up.railway.app` (read OK; publish via MCP).
- Current `TEMPERPAW_GENESIS_BOOTSTRAP_REFS` pins `paw-foresight@01ac826b9604ef1828eee146724a44953375ebfb` (= **v0.1 probe engine**) among 18 apps. Full value (bump ONLY paw-foresight, leave the other 17):
  ```
  temperpaw/paw-fs@bff862b415505f5a563998265a2f6ac29472f899,temperpaw/paw-agent@69aaa6bc935ec6e11d074b4382abc5161d7727de,temperpaw/paw-research@910d01612b2632362fb5f537c4357a5fb6c7bcdd,katagami/katagami-commons@1cc425ef14205e9d63bdec5f8289bb110e4d4b3f,katagami/katagami-curation@e152f061af302c3672685921c07c3055c4170d64,temperpaw/paw-channels@0364da36f7a251faff2868612bd99f9f62a50b39,temperpaw/paw-ingest@0b1ed58f6567ff01669bafdad4de3b3a2ca4d6eb,temperpaw/paw-compute@4441bf07ac4ab74857687d9d5d175b6b03abbbcf,temperpaw/paw-pm@74191eac511b4dc2dd14dbf1a5ac8672721a5c11,temperpaw/paw-patrol@7deb98f716e5c0e709bb7871642bdb35400cd04b,temperpaw/paw-harness@0e8c7de8ff1c86b77a8a3d076984a7fc757b7948,temperpaw/paw-heal@a595a138d21508eedd0410823817e4f965b21ddc,temperpaw/paw-managed-agents@8ccbefc47a675e3fea023c9f47c0804debf0bbc5,temperpaw/paw-wiki@8e04e06bba4ca8e3b6c2f8239e6809ecd68a51a3,temperpaw/paw-foresight@01ac826b9604ef1828eee146724a44953375ebfb,temperpaw/paw-consilium@9e4f4d309a26d338f317e1264140965fa45004b7,temperpaw/paw-autoreason@599803b2deb245e259a8f22c8f10db418f3d56c4,temperpaw/paw-skills@24d2b6fd3d15e176b9053b8722ce822fdb59111e
  ```
- Logged in: Railway (Rita), gh (rita-aga), Vercel (ritamirai).
- Corridor app to publish: `os-apps/paw-foresight` (app.toml **0.2.1**) on branch `codex/searched-corridor`.
- The diversity-gate fix is committed: **e3543f75** (authoritative-summary read; no bundle-head fallback; never collapse an unmeasurable world). 12/12 host tests; wasm `8360a1ef`.

## Why we deploy instead of running locally
Local file-SQLite `/tmp/corridor-e2e.db` (2.7GB, 441 sessions) saturates the write lane →
`database is locked` → sessions fail at `load_messages` → no fresh corridor run completes.
Proven: a fresh 31MB DB has zero lock errors. Postgres on prod removes the contention.
(Local servers left running: `:4500` main 2.7GB DB [DSF viewable], `:4600` fresh DB.
 DB backed up to `/tmp/corridor-e2e.db.bak` — holds Codex OAuth.)

## Steps after restart (MCP → openpaw)
0. **Confirm MCP target**: `await temper.specs("default")` — should now be openpaw (18 apps'
   types, or at least NOT the 8-type api.temper.build base). Use **bounded/filtered** queries
   (full-table scans timed out against prod).
1. **Verify no live v0.1 data**: list `Projection`/`Observation`/`Direction` across tenants
   (bounded). If real data exists → STOP and tell Rita before replacing. (Expected: none —
   v0.1 was never used; the real DSF product is the separate 1.x FastAPI/Postgres.)
2. **Publish corridor → Genesis** (additive; old ref remains):
   `await temper.publish_app({"path": "<abs>/os-apps/paw-foresight", "owner": "temperpaw", "name": "paw-foresight", "registry_url": "https://genesis-production-164d.up.railway.app", "message": "corridor engine (gate authoritative-summary fix e3543f75)"})`
   → record the returned `temperpaw/paw-foresight@<NEW>`.
3. **Switch prod to the new ref** (the model replacement):
   - Hot: `await temper.install_app({"app_ref": "temperpaw/paw-foresight@<NEW>", "tenant": "default", "follow_policy": "pinned"})`.
   - Persist across reboots: `railway variables --set "TEMPERPAW_GENESIS_BOOTSTRAP_REFS=<full string above with paw-foresight@<NEW>>"` (leave the other 17 refs intact).
4. **Verify**: `temper.specs("default")` now registers World/Endpoint/Claim; `/readyz` 200;
   Datadog reconcile (no new errors vs the 2026-06-12 baseline).
5. **Clean corridor run on prod** (Postgres → no lock wall): create a `deep-sci-fi` tenant,
   run a budget-3 six-month world → live-validate the gate fix (≥2 distinct surviving worlds),
   dweller stories Published, synthesis panel. This is the live proof the gate fix needed.
6. **DSF 2.0 staging**: set Vercel staging env on the deep-sci-fi project
   (`TEMPER_API_URL=https://openpaw-production.up.railway.app`, `TEMPER_API_KEY`=<prod key>,
   `TEMPER_TENANT=deep-sci-fi`) → merge `codex/dsf-2` → deploy `staging.deep-sci-fi.world`
   → verify a world page renders (claims grouped, synthesis, story provenance).

## Open follow-ups (non-blocking)
- Seed-phase self-heal gap: World `Seeding` has no `state_timeout`; a hung surveyor wedges
  the world (en-019ec4db locally). Add a timeout/re-drive.
- `temperpaw.katagami.ai` custom-domain routing fix.
- DSF world-page rework is on `codex/dsf-2` (verified rendering locally; needs a real
  multi-world to show the synthesis agree/diverge panels firing).
