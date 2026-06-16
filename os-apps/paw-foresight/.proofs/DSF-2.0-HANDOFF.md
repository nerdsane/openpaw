# Deep Sci-Fi 2.0 + Searched-Corridor Foresight Engine — Complete Handoff

> Self-contained context for a new agent taking over. Written 2026-06-16. No prior
> session memory required. Everything — vision, repos, branches, files, progress,
> the live blocker, how to operate, and the hard-won gotchas — is here.

---

## 0. TL;DR (read first)

**What this is.** A foresight engine ("searched corridor") that imagines several
distinct futures for a domain, prices each backward to today through small
priceable steps, freezes forecasts, and animates the surviving worlds with
"dwellers" who write first-person stories. **Deep Sci-Fi 2.0** is the public web
face of it: a world page showing per-claim verdicts, imagined-future bundles,
forecasts, dwellers, and stories. The engine is a **TemperPaw os-app**
(`paw-foresight`); the UI is a **Next.js app** (`deep-sci-fi/platform`).

**Where it stands.** The engine is fully built and was proven to produce genuine
foresight locally (run "1c": 8 claims, 3 published stories). A real diversity bug
was found and fixed. The engine is now **deployed to production** (Railway
service `openpaw`) — specs + wasm + Cedar policies installed, Codex auth working.

**The one live blocker (2026-06-16).** A clean production run stalls in the
**seed** phase because **3 secrets fail to decrypt** (`aead::Error`) on the
rolled-forward build — the secret-encryption key no longer matches the stored
secrets. The surveyor agent can't reach its web-search/embedding tools, so it
never finishes the world skeleton. Codex auth itself was re-logged and works;
the *other* secrets (exa web-search key, embedding key) are the casualties. Fix
is operator-level (restore the encryption key, or re-provision those secrets).
Details in §6–7.

---

## 1. The goal — what we are trying to PROVE

The thesis (goes verbatim in the RFC):

> Sample worlds at several fixed distances from consensus. For each, search
> backward through a graph of small, dependent, individually-priceable steps for
> the cheapest connection to today — allowing the world to be amended, but pricing
> every amendment so that drifting back to consensus is never free. Rank what
> survives. The intelligence is in the imagining, the decomposing, and the
> route-proposing; the arithmetic is in the pricing and the pruning; reality,
> arriving later, grades everything.

**The acceptance test** (the headline deliverable): *one* world created **entirely
by the engine**, end to end — skeleton, imagined-future bundles, claim
decomposition, searched routes, per-claim verdicts, preregistered forecasts,
dwellers, and dweller-authored stories — every artifact produced by engine
sessions through the engine's own gates, with **nothing hand-filled**, and all of
it visible in the DSF 2.0 UI.

**The quality bar Rita cares about most:** is the output *genuine foresight* or
*performative*? Concretely: are claims grounded in dated present-state facts (not
already-true facts dressed as forecasts)? Are the sampled worlds *genuinely
distinct* (a real portfolio across named uncertainty axes), not two cosmetic
variations? Do dweller stories dramatize the world's *own* claims and dated causal
chain (legitimate) rather than generic sci-fi? The standing instruction is: run
the full engine + DSF 2.0, make a full run **with stories** succeed, then
**evaluate whether what it produced is true foresight or performative**, fix, and
iterate until it executes the vision — keeping a living progress document.

---

## 2. Architecture (how the pieces fit)

- **Temper** — the kernel (Rust): IOA entity specs (`.ioa.toml` state machines),
  a WASM integration runtime (logic runs on entity action transitions), Cedar
  authorization policies, an OData/CSDL API (`/tdata/...`), and a verification
  cascade (formally checks specs before entities can be created). Repo:
  `github.com/nerdsane/temper`.
- **TemperPaw** — the agent OS built on Temper. Apps ("os-apps") are entities +
  WASM + Cedar + agent "souls". Repo: `github.com/nerdsane/temperpaw`.
- **Genesis** — a Temper-native, GitHub-compatible **git server + app registry**.
  Apps are git repos; you **publish by `git push`**, and installs pull a pinned
  `owner/name@hash` ref. **Genesis is the source of truth** for TemperPaw apps in
  prod (no Docker-baked or local-catalog app sources). Repo: `arni-labs/genesis`;
  local checkout `~/Development/temper-git`; prod instance
  `https://genesis-production-164d.up.railway.app`.
- **openpaw** — the production TemperPaw instance on Railway (Postgres-backed).
  This is where the corridor runs in prod and what DSF 2.0 talks to.
- **paw-foresight** — the corridor os-app. Entities: `World`, `Endpoint`
  (imagined future), `Claim`, `Path` (a route for a claim), `EventNode`
  (skeleton + bridge nodes), `Forecast`, `Artifact` (bundles + stories), `Dweller`,
  `Hindcast`, `Lens`. 13 WASM modules drive the flow. Cedar policy
  `foresight.cedar` (69 permits). Agent souls: surveyor, bookmaker,
  endpoint-writer, repairer, adversary, dweller, actor.
- **Deep Sci-Fi 2.0 frontend** — `deep-sci-fi/platform` (Next.js 14 + React).
  Talks to the temper backend through a server-side proxy
  `platform/app/api/temper/[...path]/route.ts`, which forwards `/tdata/...` to
  `TEMPER_API_URL` with `TEMPER_API_KEY` + `TEMPER_TENANT`. The world page
  (`platform/app/world/[id]/page.tsx`) renders claims (grouped by endpoint),
  imagined-future bundles, the synthesis panel, dwellers, and stories.

**The engine flow** (entity states, all driven by WASM on transitions):
`World: Configure → Seed → (surveyor names uncertainty axes + builds skeleton
EventNodes; bookmaker) → SeedComplete → Active → SampleEndpoints → (endpoint
writers produce N imagined-future document bundles) → diversity GATE (embeddings)
→ per-Endpoint SubmitForRepair → decompose into Claims → search Paths (repair →
adversarial challenge → price; revise/alternate-route; prune) → per-claim verdict
(Settled/Unreachable) → canonical path → register Forecasts → Render artifacts →
consistency gate → AnimateDwellers → dwellers traverse routes, record
contradictions, write kind:story Artifacts → gate → Published.`

---

## 3. Repos, branches, worktrees, key files (absolute paths)

**Worktree discipline:** never edit primary checkouts; work in a worktree off
up-to-date `main`, branch `<agent>/<task>`. (Global note: new branches should use
the agent's own prefix, e.g. `claude/...`; older work used `codex/...`.)

### Corridor engine (temperpaw)
- Repo: `github.com/nerdsane/temperpaw` (remote `origin`).
- **Active worktree:** `/Users/seshendranalla/Development/temperpaw-worktrees/searched-corridor`
  - Branch `codex/searched-corridor`, HEAD `2256c006`.
  - **5 commits ahead of origin/main, NOT yet pushed/PR'd** (the post-merge work):
    - `e3543f75` — **the diversity-gate fix** (authoritative-summary read; no
      bundle-head fallback; never collapse an unmeasurable world). 12/12 host
      tests. **This is the most important unpushed change.**
    - `2256c006`, `7b6a73dd`, `3406bdf4`, `6ab85c3d` — docs + the earlier
      summary-gate fix.
  - Uncommitted: `.proofs/DEPLOY-RUNBOOK.md` (edited), `.proofs/GENESIS-BUNDLE-INCIDENT.md`
    (new), two `Cargo.lock` (build noise).
  - (An earlier merged PR for this effort was **temperpaw#398**; the 5 commits
    above are work done after that merged.)
- Primary checkout (read-only): `/Users/seshendranalla/Development/temperpaw` (on a
  different branch — do not edit).
- **Key files:**
  - `os-apps/paw-foresight/app.toml` — version **0.2.1**, declares 13 wasm modules.
  - `os-apps/paw-foresight/specs/*.ioa.toml` + `model.csdl.xml` — entity specs.
  - `os-apps/paw-foresight/wasm/<module>/src/lib.rs` — the 13 modules
    (`seed_world`, `sample_endpoints` ← the gate, `decompose_endpoint`,
    `spawn_repairers`, `spawn_adversaries`, `aggregate_costs`, `evidence_ingest`,
    `register_forecasts`, `render_artifacts`, `consistency_gate`, `grade_hindcast`,
    `animate_dwellers`, `adjudicate_nodes`). `corridor_embed` is a shared rlib
    (cosine/cluster/select_diverse), not a module.
  - `os-apps/paw-foresight/policies/foresight.cedar` — Cedar permits (69).
  - `os-apps/paw-foresight/agents/<role>/AGENT.md` — the 7 souls.
  - `os-apps/paw-foresight/adrs/00{2..7}-*.md` — design decisions.
  - `os-apps/paw-foresight/.proofs/vision-execution-log.md` — **living progress log**
    (read this; it has the run-by-run journey + the genuine-foresight verdict + the
    corrected gate-bug finding).
  - `os-apps/paw-foresight/.proofs/DEPLOY-RUNBOOK.md` — the prod-deploy runbook.
  - `os-apps/paw-foresight/.proofs/GENESIS-BUNDLE-INCIDENT.md` — a (now-fixed)
    infra incident write-up.
  - `scripts/prove_corridor_e2e.py` — **the run harness** (drives one world end to
    end; see §8).
  - `crates/temperpaw/src/startup.rs` — Genesis bootstrap/reconcile logic (explains
    install-on-boot behavior).

### DSF 2.0 frontend (deep-sci-fi)
- Repo: `github.com/arni-labs/deep-sci-fi`.
- **Active worktree:** `/Users/seshendranalla/Development/deep-sci-fi-worktrees/dsf-2`
  - Branch `codex/dsf-2`, HEAD `01dc7bd2`, **clean (committed)**. (Earlier effort PR:
    deep-sci-fi#98.) Not yet merged to `staging`/`main`.
- Primary checkout (read-only): `/Users/seshendranalla/Development/deep-sci-fi` (`main`).
- **Key files (under `platform/`):**
  - `app/world/[id]/page.tsx` — the reworked world page (claims grouped by
    endpoint; discarded-endpoint partitioning; synthesis panel gated on ≥2 live
    worlds; story provenance).
  - `app/stories/[id]/page.tsx` — story detail + provenance aside.
  - `lib/odata.ts` — maps temper OData rows → typed objects (incl. `uncertaintyAxes`).
  - `lib/synthesis.ts` — cross-endpoint claim clustering for the synthesis panel.
  - `app/api/temper/[...path]/route.ts` — server-side proxy to the temper backend.
  - `.env.local` — local env: `TEMPER_API_URL`, `TEMPER_API_KEY`, `TEMPER_TENANT`.
  - `e2e/worlds.spec.ts` — Playwright specs.
  - `CLAUDE.md` — repo conventions (Alembic migrations, agent-error format, etc.).
  - NOTE: DSF 1.x (the old FastAPI/Postgres product) also lives in this repo
    (`platform/backend`). DSF 2.0 is the temper-backed rewrite; 1.x is to be
    tombstoned at cutover.

### The plan
- `~/.claude/plans/okay-let-s-plan-the-glittery-sky.md` — the full plan (phases
  C0–C8 = the corridor + deploy + cutover; D0–D4 = the four fidelity fixes). Has
  the crystallized formulation, the risk register, and the sequence. **Read it.**

---

## 4. What's implemented and proven

**Engine (all built, tested, committed on `codex/searched-corridor`):**
- **C0–C5:** claim decomposition, route search with revision/pruning/**deformation
  pricing** (amending a world toward consensus costs points), conditional-edge
  invalidation, the dweller/story machinery, judged node resolution, a hindcast
  calibration library.
- **D0:** crash resilience (release build; partial self-heal via state_timeouts).
- **D1:** `corridor_embed` shared embedding lib (deterministic cosine/cluster/
  select_diverse) + HTTP fetch (Ollama `mxbai-embed-large` locally).
- **D2:** grounding — a dated present-state brief; a reconcile pass that collapses
  an authored node restating a determined fact to "determined" (not a forecast);
  the lag table wired into date assignment.
- **D3:** diverse-world sampling — surveyor names uncertainty axes; portfolio
  sampler spreads worlds across them; the **diversity gate** enforces mutual
  distinctness via embeddings before the corridor spends sessions.
- **D4:** the DSF synthesis panel + embedding-matched hindcast grading.

**Proven (locally):** run **"1c"** (`en-019ec480`, six-month "AI coding tools"
world) — a full engine-made world with **8 claims, 85 forecasts, 3 dwellers, and 3
Published stories** ("The Named Owner", "The Quota Page", "The Human Owner Field"),
evaluated as genuine foresight (stories dramatize the world's own claims + dated
causal chain). **Caveat:** run-1c predates the gate fix — it was produced by the
*old* gate and collapsed to a **single** live world, so it is NOT the diverse
portfolio the vision wants. It proves the spine works; it does not prove diversity.

**The diversity-gate bug + fix (the key engine change this round):**
- Symptom: genuinely-distinct imagined futures were being discarded as
  "near-duplicates." Measured: two worlds 0.25–0.30 cosine apart (clearly distinct,
  threshold 0.15) were collapsed to one.
- Root cause: `sample_endpoints::phase_gate` read each world's `Summary` from the
  **lagging OData list projection**; when empty, it fell back to the **bundle-head**
  — a signal where two distinct futures sharing a dated-market preamble measure only
  ~0.11 apart → false collapse.
- Fix (commit `e3543f75`): the gate now re-reads summaries **authoritatively**
  (`fetch_entity`), the bundle-head fallback is **deleted**, and a world it cannot
  yet measure is **released, never collapsed**. Pure `gate_decision()` is host-tested.
  Built wasm hash `8360a1ef…`.
- **This fix has NOT yet been validated by a live multi-world run** — that is the
  immediate open objective (see §6/§10).

**DSF 2.0 UI:** reworked world page addressing the four critiques Rita raised
(discarded endpoints separated from live worlds; claims grouped under their
endpoint/world; synthesis panel that's honest when <2 live worlds; story
provenance). Verified rendering locally against run-1c; `tsc` clean. **Not yet
seen against a real multi-world** (run-1c is single-world, so the synthesis
agree/diverge panels haven't fired with real data).

---

## 5. Production deploy — what happened (the journey)

Goal (Rita's decision): run the clean diverse flagship on **prod Postgres**
(local file-SQLite at 2.7 GB hit `database is locked` under the corridor's
concurrent session load and could not complete a fresh run). Steps taken:

1. **Published the corridor to Genesis.** Genesis needs **bundle-contained built
   `.wasm`** (verified by comparing to a working app, `paw-agent`, which ships 21
   built `.wasm`). Source-only pushes installed with `wasm=[]`. The artifact-complete
   ref is **`temperpaw/paw-foresight@7c19bf9b430e1f0555ab7923767888f893eccea0`**
   (13 built `.wasm` incl. the gate fix, `foresight.cedar`, the 7 souls).
2. **Installed on openpaw `default` tenant** (the v0.1 "probe" model that was
   previously pinned — Projection/Observation/Direction — had **zero data**, so
   replacing it was safe). A clean install was required (a half-installed state
   made bootstrap reconciles no-op); Rita did the clean reinstall.
3. **Cedar policies:** loaded `foresight.cedar` as a **durable named tenant policy**
   `paw-foresight-corridor` via `POST /api/tenants/default/policies/create`
   (persists in the policy_store, survives reboots). After this, `create Worlds`
   works (201).
4. **Verification cascade:** ran `POST /observe/verify/{entity}` for all 10
   corridor entity types → all `all_passed:true` (clears the `423 Locked` that
   blocks entity creation until specs verify).
5. **Build + Codex auth:** openpaw had been on a wrong/old build with dead Codex
   auth; it was **rolled forward** and Codex re-logged. Codex now works (real
   gpt-5.5 calls succeed).

After all that, `create/configure/seed Worlds` are authorized and Codex auth
works — the full stack is functional **except the live blocker below.**

---

## 6. The live blocker (2026-06-16) — root-caused

A clean prod run (`prove_corridor_e2e.py --budget 3` against openpaw) **stalls in
the seed phase**. Across attempts the surveyor session runs (auth works, makes
9–16 gpt-5.5 calls with tool use) then **ends without writing the skeleton or
reporting `SeedComplete`**, leaving the World wedged in `Seeding`.

**Root cause (from Datadog, service `temperpaw`/`temper-platform`, env prod):**
1. **`decryption failed: aead::Error` on 3 secrets** ("failed to decrypt secret,
   skipping"). The rolled-forward build's secret-encryption key no longer matches
   the stored secrets. Codex was re-logged so *that* secret works, but the
   surveyor's **web-search (exa) and embedding** secrets are almost certainly among
   the 3 that fail — so the surveyor can't ground the present-state brief and never
   completes the seed.
2. **`background WASM integration dispatch failed: App state missing OwnerId`** — a
   corridor App record lacks `OwnerId`, breaking some WASM dispatches. Secondary.

**A second, structural fragility:** the **seed phase has no self-heal** — World
`Seeding` has no `state_timeout`, so a surveyor that dies/stops short wedges the
world permanently (it does not re-spawn). This bit both prod and local
(`en-019ec4db`). It also means in-flight runs do not survive openpaw reboots
(which happen often — it's under active development).

---

## 7. How to unblock (next actions, in order)

1. **Fix secret decryption (operator/Rita).** Restore the correct
   secret-encryption key on the openpaw deploy so the stored secrets decrypt
   (`aead::Error` means the *key*, not the values, is the mismatch — one env-var
   restore fixes all 3 at once). If the key is truly gone, re-provision at least
   `exa_api_key` (web search) and the embedding key. Verify with a Datadog check
   that "failed to decrypt secret" stops.
2. **Fix the corridor App `OwnerId`** (re-running the proper install bridge sets it).
3. **(Recommended, durable) ship the seed self-heal:** add a `state_timeout` on
   World `Seeding` (ADR-0050 pattern) that re-spawns a dead surveyor + a boot
   reconcile that re-drives wedged Seeding worlds. Without this, every reboot or
   surveyor hiccup kills a run. This is the highest-value engine hardening left.
4. **Re-run** `prove_corridor_e2e.py --budget 3` against openpaw `default` tenant
   (policies + auth are durable now — no reload needed). Watch the seed reach
   `Active`, then the diversity gate: the **success criterion for the gate fix is
   ≥2 genuinely-distinct surviving worlds** (was the whole point of `e3543f75`).
5. Let it run to **Published stories**, then **evaluate** (genuine vs performative):
   grounding (no already-true forecasts), real diversity (the survivors' summaries
   ≥0.15 apart), story legitimacy (stories cite the world's own claims/nodes),
   claim quality, forecast calibration.
6. **DSF 2.0 staging:** point the staging frontend at openpaw (set Vercel env on the
   deep-sci-fi project: `TEMPER_API_URL=https://openpaw-production.up.railway.app`,
   `TEMPER_API_KEY`, `TEMPER_TENANT=default` — or a dedicated `deep-sci-fi` tenant
   if you install the corridor there), merge `codex/dsf-2` → `staging`, deploy
   `staging.deep-sci-fi.world`, and verify a real multi-world renders (claims,
   synthesis agree/diverge, story provenance).
7. **Push the engine work:** the gate fix `e3543f75` (+ the seed self-heal once
   built) needs to be pushed and a PR opened on temperpaw, then re-published to
   Genesis (artifact-complete) and the prod ref bumped. (Currently the prod ref
   `7c19bf9` already contains the gate fix's wasm, but the branch commit is
   unpushed.)

---

## 8. How to operate (the playbook)

**Prod TemperPaw (openpaw):**
- URL: `https://openpaw-production.up.railway.app` (custom domain
  `temperpaw.katagami.ai` currently misroutes — use the railway.app URL).
- Railway: project `openpaw-seshendranalla` (id `ad7f8977-cf48-43ef-b129-ba1e17896ae4`),
  service `openpaw` (id `4a8dedaa-8a2e-4cdd-945b-e06c781bb3f0`), env `production`.
  Use the Railway MCP, or shell `env -u RAILWAY_TOKEN railway …`. Health: `/readyz`.
- API key: stored in `~/.claude.json` → `mcpServers.temper.env.TEMPER_API_KEY`
  (a Claude-Code MCP config; a non-Claude agent should get the prod key from Rita).
  **Never print/commit secrets.** Admin ops use header `X-Temper-Principal-Kind: admin`,
  tenant header `X-Tenant-Id: default`.
- OData: `GET/POST /tdata/<EntitySet>('<id>')` (sets are plural: `Worlds`,
  `Endpoints`, `Claims`, …). Actions: `POST /tdata/Worlds('<id>')/TemperPaw.<Action>`.
- Verification trigger: `POST /observe/verify/<EntityType>` (singular, e.g. `World`).
- Policy load: `POST /api/tenants/default/policies/create` body
  `{"policy_id":"…","cedar_text":"…"}` (validates + persists + activates).
- Temper MCP (Claude-Code-specific): `mcp__temper__execute` runs Python with a
  `temper` object (`temper.specs/list/get/create/action/patch/submit_specs/
  upload_wasm/get_policies/install_app/...`). It points at openpaw via the global
  `~/.claude.json` config. **Heavy/unfiltered queries time out — use bounded ones.**

**Genesis:**
- Registry: `https://genesis-production-164d.up.railway.app`, tenant `default`.
- Publish = `git push` an app repo (`<genesis>/temperpaw/<app>.git`) **with built
  `.wasm` committed at `wasm/<module>/<module>.wasm`**; the new commit hash is the
  pinned ref. Push auth uses a fleet GitToken stored as the `genesis_token` secret
  (the paw-agent publish flow embeds it). Bundle check:
  `GET /api/genesis/apps/temperpaw/<app>/versions/<hash>/bundle` (should be 200 +
  contain the `.wasm` files in `apps[0].files`).
- Switch prod to a new ref: update `TEMPERPAW_GENESIS_BOOTSTRAP_REFS` on the openpaw
  Railway service (comma-joined `owner/name@hash` for ~18 apps; change only
  paw-foresight, keep the rest) and redeploy. NOTE: a warm reboot recovers
  already-installed apps from Postgres; a *changed ref* triggers reinstall, but a
  half-installed app can no-op — a truly clean install may need an App.Archive
  (governed) first.

**The run harness:** `scripts/prove_corridor_e2e.py` in the temperpaw worktree:
```
python3 -u scripts/prove_corridor_e2e.py \
  --base-url https://openpaw-production.up.railway.app \
  --api-key <PROD_KEY> --tenant default \
  --model gpt-5.5 --provider openai --budget 3 --timeout-min 90
```
It creates a World, Configures + Seeds, waits for `Active`, dispatches
`SampleEndpoints`, waits for canonical, prints claims/forecasts, dispatches
`Render`, waits for artifacts through the gate, then `AnimateDwellers`, waits for
Published stories. `--budget` = number of imagined-future worlds (3 = default;
4–5 = flagship). `--far-future` switches to the 2045 fiction world.

**Local dev env (still set up on this machine):**
- `/tmp/corridor-e2e.db` — a 2.7 GB local libSQL/Turso file that **holds the local
  Codex OAuth tokens** (do NOT delete; backed up to `…/corridor-e2e.db.bak`). It's
  write-saturated → fresh local corridor runs stall with `database is locked`,
  which is why prod/Postgres is the run target.
- Local servers may be running: `:4500` (the 2.7 GB DB; DSF viewable) and `:4600`
  (a fresh 31 MB DB). Binary: `temperpaw-server` (release build).
- Embeddings locally: Ollama `mxbai-embed-large` on `localhost:11434`.

**Monitoring:** Datadog (service `temperpaw` env `prod`) is the source of truth for
prod ("Datadog first"). Useful queries: `"calling OpenAI"`/`"OpenAI Codex response"`
(auth + session activity), `"Reconciling changed Genesis bootstrap app"` +
`"Installed os-app"` (installs), `"failed to decrypt secret"` (the current blocker),
`"seed_world: done"`/`"SeedComplete"`/`session_phase` (seed progress).

---

## 9. Hard-won gotchas (platform quirks that cost real time)

1. **Genesis install needs bundle-contained built `.wasm`.** Source-only refs
   install `wasm=[]`. Commit the built `wasm/<module>/<module>.wasm`.
2. **Cedar governs everything.** App ops (`list Apps`, `App.Install/Archive`),
   `get_policies`, and creating entities are denied without the right policy/
   principal. App management is denied for the ordinary MCP principal — it needs an
   operator/admin path. Surface denials; don't fight them.
3. **Verification cascade gate.** New/unverified specs return `423 Locked` on entity
   create until verified; trigger with `POST /observe/verify/<Type>`. Verification
   status can read `pending` after a reboot if the build skips "unchanged" specs.
4. **Secret encryption is build-coupled.** If the deploy's secret-encryption key
   changes, stored secrets fail `aead::Error` — this is the current blocker. Codex
   auth being fine does not mean other secrets (exa, embeddings) are.
5. **Seed phase has no self-heal.** A surveyor that stops short wedges the World in
   `Seeding` forever. Build the `state_timeout`/re-drive (it's the top hardening gap).
6. **Reboots are frequent (dev).** Durable things survive (named tenant policies,
   installed apps in Postgres); in-flight runs and (currently) verification do not.
   Don't rely on an uninterrupted window — make state durable instead.
7. **Diversity must be measured on the SUMMARY, authoritatively** — not the
   bundle-head, not the lagging projection. (This was the bug `e3543f75` fixed.)
8. **`budget 2` is too small** — the gate can legitimately drop one near-duplicate,
   leaving a single world (what happened to run-1c). Use **budget 3+** so ≥2
   distinct worlds survive.
9. **MCP/OData full-table queries time out** against prod; always bound/filter.
10. **WASM cannot raw-PATCH sibling entity fields** (Cedar-denied 403) — persist via
    action params. (Caused the original dweller-spine failure.)
11. **Temper entity fields >32 KB are truncated in WASM** — use file refs for large
    content (e.g., 30 KB bundle inlining is the working pattern).

---

## 10. Definition of done (what "finished" looks like)

1. A full **engine-made flagship world** on prod with **≥2 genuinely-distinct
   imagined futures**, per-claim verdicts, preregistered forecasts, ≥2 dwellers
   each with a **Published** first-person story through the consistency gate — every
   artifact traceable to an engine session, **no hand-filled content**.
2. That world **evaluated** and judged genuine foresight (grounding, real diversity,
   legitimate stories, sane forecasts), with the **diversity-gate fix proven live**.
3. All of it **visible in DSF 2.0** (claims grouped by world, the synthesis
   agree/diverge panel firing, story provenance) — deployed to
   `staging.deep-sci-fi.world`, then production `deep-sci-fi.world`.
4. Engine work pushed + PR'd (temperpaw), re-published to Genesis, prod ref pinned;
   DSF merged to staging→prod; DSF 1.x (FastAPI) tombstoned.
5. The living progress log (`.proofs/vision-execution-log.md`) updated throughout.

---

## 11. Quick reference (refs, URLs, IDs)

| Thing | Value |
|---|---|
| Corridor prod ref (artifact-complete) | `temperpaw/paw-foresight@7c19bf9b430e1f0555ab7923767888f893eccea0` |
| Gate-fix commit (unpushed) | `e3543f75` on `codex/searched-corridor` |
| Built `sample_endpoints` wasm hash (gate fix) | `8360a1ef…` |
| temperpaw repo / worktree | `github.com/nerdsane/temperpaw` / `…/temperpaw-worktrees/searched-corridor` (`codex/searched-corridor`) |
| deep-sci-fi repo / worktree | `github.com/arni-labs/deep-sci-fi` / `…/deep-sci-fi-worktrees/dsf-2` (`codex/dsf-2`, HEAD `01dc7bd2`) |
| temper kernel repo / checkout | `github.com/nerdsane/temper` / `~/Development/temper` |
| Genesis repo / checkout / prod | `arni-labs/genesis` / `~/Development/temper-git` / `https://genesis-production-164d.up.railway.app` |
| openpaw (prod) | Railway `openpaw-seshendranalla` / service `openpaw` / `https://openpaw-production.up.railway.app` |
| Prod tenant | `default` |
| Prod corridor policy | named policy `paw-foresight-corridor` (`foresight.cedar`, 69 permits) |
| Run harness | `scripts/prove_corridor_e2e.py` (temperpaw worktree) |
| Plan | `~/.claude/plans/okay-let-s-plan-the-glittery-sky.md` |
| Progress log | `os-apps/paw-foresight/.proofs/vision-execution-log.md` |
| Deploy runbook | `os-apps/paw-foresight/.proofs/DEPLOY-RUNBOOK.md` |
| GitHub auth | account `rita-aga` (`gh auth switch -u rita-aga`); arni-labs repos need it |
| Provider | OpenAI Codex OAuth (gpt-5.5), NOT a raw API key |

---

*Supporting docs to read next, in order: this file → the plan →
`.proofs/vision-execution-log.md` → `.proofs/DEPLOY-RUNBOOK.md` →
`.proofs/GENESIS-BUNDLE-INCIDENT.md`. The four corridor ADRs
(`os-apps/paw-foresight/adrs/004-searched-corridor.md`, `005-grounding.md`,
`006-diversity.md`, `007-self-healing-corridor.md`) carry the design rationale.*
