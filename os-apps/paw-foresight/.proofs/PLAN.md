> PORTABLE COPY of the active plan (the live goal being executed). Original was
> Claude-Code plan-mode storage (~/.claude/plans/), not reachable by other agents.
> This is the canonical copy. Companion: DSF-2.0-HANDOFF.md (overview) and
> os-apps/paw-foresight/.proofs/vision-execution-log.md (run-by-run progress).
> The plan's C0–C8 / D0–D4 phases are the task breakdown being worked.

# Searched Corridor (paw-foresight 0.3) + Living Worlds (DSF 2.0 launch) — Plan v2

> Supersedes the v1 plan (corridor engine + DSF rewire), whose A0–A6/B1–B2 phases are complete and proven. This plan carries the unfinished v1 items (deploy, Genesis sync, archive, cutover) and adds the approach crystallized on 2026-06-12.

## What we are addressing

The v1 engine proved the spine: imagined futures (document bundles under stances) priced backward to today, forecasts frozen, reality grading them (hindcast mean Brier 0.1512 vs 0.25 coin-flip). But three gaps separate it from the actual goal:

1. **The verdict is a scalar on a single chain.** Each imagined world gets ONE linear bridge, ONE pass, ONE number. No decomposition into claims, no alternative routes, no iteration — the cost means "the toll on the first road tried," not a minimum.
2. **The fiction layer doesn't exist.** No module animates Dwellers; no `kind: story` artifact has ever been produced; the imagined-future bundles (the richest material) are invisible in the UI. DSF 2.0 today is a forecasting board with two documents.
3. **Resolution and calibration are stubs.** Authored claims have no resolution path (only market-URL nodes auto-resolve); cost constants are hand-set priors with no tuning loop.

Rita's decisions (2026-06-12): build v2 first, deploy once; all three tracks in scope; deep-sci-fi.world cuts over only when full v2 is live.

## Status (2026-06-13) — Track C built; live run surfaced four fidelity gaps

Track C (C0–C5) is **implemented, reviewed, committed, pushed** (PR temperpaw#398): claim decomposition, route search with revision/pruning/deformation, conditional-edge invalidation, the dweller/story machinery, judged resolution, the hindcast library + calibrator. DSF 2.0 UI (claims panel, imagined-future bundles, story bylines, nav cleanup) is built and pushed (PR deep-sci-fi#98). The first live run (C6, six-month world) **proved the corridor works** — 15 claims decomposed, multi-route search, live revisions, a live AmendText+deformation, honest Unreachable verdicts — and surfaced four gaps that Track D now fixes:

- **G1 — Grounding hygiene.** Agents have web tools but **no `exa_api_key`** is configured, so they fall back to a stale training prior and mint already-true facts as uncertain future forecasts (e.g. "first-party agents grow as rivals" dated 2026-09-30 at p=0.55). No reconcile pass collapses an authored node that restates a determined fact; the lag table isn't wired into date assignment. This poisons calibration (already-true "forecasts" resolve yes for free).
- **G2 — Sampling fidelity.** Stances are unverified prompt instructions; nothing measures whether an "anti-modal" world is actually off-consensus. Modal and anti-modal worlds converged. The model is also too small: worlds should be a *diverse portfolio across named uncertainty axes*, not two arbitrary stances, and each (expensive) world must be guaranteed meaningfully different before the corridor spends sessions on it.
- **G3 — Readability.** The world page dumps raw bundles; it never synthesizes "what the futures agree on / where they diverge / what to watch." A reader cannot derive "what happens to coding tools in six months" from four documents.
- **G4 — Crash resilience.** The debug server aborted twice under v2 session concurrency (SIGABRT/SIGSEGV), orphaning in-flight sessions and forcing manual salvage. Runs must finish smoothly: run on the release build, and make interrupted corridors self-heal rather than need hand-driven re-dispatch.

Embeddings are the shared tool for several of these (assessed 2026-06-13): **build** for world-diversity sampling (G2), cross-world claim clustering → synthesis (G3), reconcile-authored-vs-determined (G1), and hindcast actuals→forecast matching (retires the brittle substring needles). **Do not** use embeddings for logical contradiction or as the anti-consensus test — distance-between-samples ≠ distance-from-consensus.

### Session update (2026-06-13, late) — dweller-spine blocker root-caused + fixed; run-1 retired

Run-1 (`en-019ebd69-…`) settled (canonical set, 9 Settled / 6 Unreachable claims, 85 forecasts, 2 render artifacts published) but then **terminally Failed at the dweller phase**, which is why no `kind:story` artifact had ever been produced. Root cause: `animate_dwellers`' `DwellersCast` handler persisted the dweller roster with a raw `PATCH /tdata/Worlds(...)`, which **Cedar denies (403** — only Admin may PATCH a World; system flows drive it through actions). The handler returned that error, the World hit `Failed` (terminal), and the 3 cast dwellers (Marta Voss, Devon Iqbal, Lena Cho) were orphaned with no traversal/story sessions. Run-1 is now retired (Failed is final); it served its purpose as the gap-finder.

Five fixes landed on `codex/searched-corridor` (PR temperpaw#398), each red-green TDD'd + wasm rebuilt:
- **D1 core** (`1b79c8d5`): `corridor_embed` deterministic embedding library (cosine/cluster/farthest-point), 7 golden tests. HTTP fetch into consumers still pending Ollama.
- **D0 liveness** (`021ea51a`): `claim_decision` settles on the cheapest acceptable route once the route budget is spent rather than waiting forever on a wedged straggler — makes ADR-007's "budget bounds the self-heal" real. ADR-007 consequences corrected to describe the claim-level bound.
- **D0 straggler termination** (`ed77156b`): when a claim settles, `claim_phase` fails its still-in-flight sibling routes ("superseded"), stopping the one unbounded self-heal left after settlement. Completes the liveness fix.
- **D2 lag table** (`ba2d222c`): the repairer prompt now inlines the world's lag table + a DATE DISCIPLINE contract (dates derived from historical lags anchored to TODAY → horizon; under-lag compression raises a `lag` cost). Fixes eyeballed round-date authoring. (`frontier_date` is the scoreable horizon, NOT "today" — confirmed against `register_forecasts`; "today" stays rhetorical as in the surveyor.)
- **C4 dweller spine** (`e5cadbd8`): `dweller_ids` now rides a `SpawnNextDweller` action param (spec + CSDL + handler) instead of the denied World PATCH. The fatal blocker above. `corridor_engine_contract` 12/12 + `corridor_cedar_matrix` 10/10 confirm the dweller/story authorization path (create → SubmitForCheck → gate → Publish) is otherwise Cedar-clean.

**Known follow-ups (non-blocking, documented so they're not lost):**
- *PATCH-class Cedar gap.* Several modules mutate sibling-entity fields via raw PATCH; Cedar denies system/agent PATCH. The dweller-roster instance was fatal (fixed via action param). The rest are **non-fatal** (warn + proceed): `spawn_repairers`/`spawn_adversaries` setting `repairer_agent_id`/`adversary_agent_id` (loosens the assigned-repairer/adversary binding — the session still spawns), and `animate_dwellers` `phase_contradiction` appending `challenge_flags` (the `Reprice` action still carries the pricing feedback, so a dweller contradiction still re-prices the claim; only the flag record is dropped). The generic fix — convert these to explicit actions, or add scoped system-update permits — is a dedicated effort, not rushed onto the end of this one.
- *Live e2e of the four engine fixes* (claim_decision, straggler termination, lag table, dweller spine) lands in the next fresh run — run-1's World is terminally Failed and cannot host the re-drive. The full dweller → story → gate → publish path is therefore not yet *seen* working; the gate is the C6 flagship run.

**Flagship readiness / decision point:** the engine is now correct for a run that reaches dwellers + stories.

Progress since (2026-06-13, later): **Ollama is installed + running** (brew service, `mxbai-embed-large` on `localhost:11434`, verified). **D1 (embedding fetch)** and **D2 (reconcile + lag table)** are implemented, tested, committed, pushed, and live-validated against the embedder (reconcile threshold 0.10 calibrated: restatements ~0.06, distinct ~0.29–0.35). ADR-005 (grounding) + ADR-006 (diversity) written.

Remaining D-work, by character:
- **D3 diversity gate** — an *architecture change* (a barrier so the gate sees all written bundles before re-steering collapsed worlds; ADR-006 option A vs B left open for review). Best designed-then-verified in a run, not rushed.
- **D4 hindcast matching** — needs the fixture actuals rewritten from keyword needles (`'gpt-5'`, `'price'`) to embeddable full-text descriptions; only affects the calibration suite, not the flagship world.
- **D4 synthesis panel** — DSF-repo UI work (cluster claims via `cluster_by_threshold` → agree/diverge/leading-indicators).

So the engine is now **flagship-capable** (grounded skeleton via exa, reconcile backstop, lag-disciplined dates, dweller-story spine fixed, liveness fixed) but **not yet diversity-gated or synthesis-rendered**. Decision: (a) run the flagship now to get a real grounded world *with stories* (endpoints sampled with distinct stances but mutual distinctness not yet enforced; raw bundles, no synthesis panel), then layer D3/D4; or (b) build D3 (decide ADR-006 A/B) + D4 first for a fully-clean flagship. A full world is ~30–50 gpt-5.5 sessions; not started autonomously.

## Expected end state (Definition of Done)

**The headline deliverable — the flagship world.** At least one world that was *created entirely by the engine*, end to end: skeleton, imagined-future bundles, claim decomposition, searched routes, per-claim verdicts, preregistered forecasts, dwellers, and dweller-authored stories — every artifact produced by engine sessions through the engine's own gates. Nothing hand-filled: no admin-seeded content, no operator-written text standing in for engine output (operator involvement is limited to dispatching actions and openly-flagged salvage, as with the v1 honesty flags). All of it implemented as a TemperPaw os-app (entities + WASM + Cedar — engine *and* the DSF-specific dweller/story machinery alike), and all of it visible in the DSF 2.0 UI: the world page shows the claims, routes, receipts, bundles, dwellers and stories of that engine-made world. This is the single acceptance test the rest of the plan serves.

One production deploy and one public cutover, after which:
- A world page shows **per-claim verdicts**: each imagined world decomposed into 3–8 load-bearing claims, each with its cheapest explored route, its price, and its amendment history. The cost number means "cheapest of several explored routes."
- Worlds contain **dweller-authored stories** through the consistency gate, surfaced bundles ("the imagined future" as a first-class section), and at least one **live retcon** produced by the evidence loop.
- Authored claims resolve through a **judged adjudicator** (rubric + snapshotted evidence + human escalation), never by string-match.
- Cost constants carry **calibration receipts** from a hindcast library (≥4 fixtures), not just intuition.
- The drift constraint is enforced: **amendment is priced** (deformation flags); drifting an anti-modal world back to consensus is never free.
- deep-sci-fi.world serves all of it live; FastAPI 1.x is tombstoned; engine deployed on Railway with Datadog proof; Genesis synced.

## The crystallized formulation (goes verbatim into the RFC)

> Sample worlds at several fixed distances from consensus. For each, search backward through a graph of small, dependent, individually-priceable steps for the cheapest connection to today — allowing the world to be amended, but pricing every amendment so that drifting back to consensus is never free. Rank what survives. The intelligence is in the imagining, the decomposing, and the route-proposing; the arithmetic is in the pricing and the pruning; reality, arriving later, grades everything.

Design commitments accumulated in conversation (codify in ADRs before code):
1. Cost constants are tunable priors pending hindcast calibration; the ordering (miracle > contradiction > incentive > lag) is the design claim, the values are not.
2. Authored-claim resolution must stay a judged step (LLM adjudicator w/ rubric + evidence snapshots + human escalation). Never a regex/threshold over text.
3. Evidence adapters are pluggable; Polymarket/Kalshi are v1 adapters, not architecture.
4. **Drift constraint**: repair may amend a claim only via an explicit action carrying the diff, priced as a `deformation` flag. Objective = repair cost + deformation cost, never repair cost alone.
5. **Grounding** (ADR-005): every imagined-future and bridge is grounded in a dated present-state brief; an authored node that restates a determined/present fact is collapsed to determined, never registered as a forecast. Web search (`exa_api_key`) feeds the brief in live mode; the brief itself is the deterministic fallback the reconcile pass checks against.
6. **Diversity** (ADR-006): worlds are sampled as a portfolio across named uncertainty axes; embeddings guarantee mutual distinctness *before* the corridor spends sessions; "different from other samples" (embeddings) and "surprising vs consensus" (a consensus reference) are separate guarantees with separate machinery.

---

## Track C — Searched corridor (temperpaw, paw-foresight 0.2→0.3)

Worktree `codex/searched-corridor` off updated main; draft PR on first commit; one PR for the track.

### C0 — ADRs, RFC update, collision gate
- ADR-004 "Searched corridor": claim decomposition, route search, drift constraint, conditional edges. ADR-002 amended with the four commitments above.
- RFC gets the crystallized formulation as its opening.
- Entity-name gate: `Claim` verified unique as an automaton name across `os-apps/*/specs` (paw-patrol's `Claim` hits are *actions* on Run entities — actions don't collide in CSDL; confirmed 2026-06-12). Extend the existing cross-app uniqueness CI test (crates/temperpaw/tests/corridor_engine_contract.rs) to include `Claim`.

### C1 — Claim entity + decomposition
- **New spec `claim.ioa.toml`**: fields `world_id`, `endpoint_id`, `original_text` (frozen at creation), `current_text`, `amendment_log_json`, `route_count`, `best_route_cost`, `classification`; states `Proposed → Bridging → Settled | Unreachable` (ADR-0050: `state_timeout` on Bridging). Actions: `SubmitForBridge`, `AmendText(old, new, justification)` (records diff, system-relay; the amending repairer must also flag `deformation`), `RouteSettled` (self-loop as routes finish; guards `route_count`), `Settle`, `MarkUnreachable`.
- **New WASM `decompose_endpoint`**: refit `Endpoint.SubmitForRepair` trigger — spawns one decomposer session (bundle inlined, 30KB pattern) → self-reports `Endpoint.DecompositionComplete(claims_json)` → WASM creates Claim entities (3–8) and dispatches `SubmitForBridge` per claim, **chunked via entity self-loop** (`Endpoint.SpawnNextBridge`, `check_count`/`max_checks`) to respect session admission caps. Decomposition failure → `Endpoint.Fail` with reason, never silent.
- `spawn_repairers` refit: fires per-Claim; prompt = claim text + relevant bundle excerpt (inlined) + skeleton list + **edges contract**: bridge nodes now declare `depends_on` (stored via existing `EventNode.UpdateEdges(edges)`, system relay).
- Prompt-contract tests for decomposer + refit repairer (exact action/param names).

### C2 — Route search + iteration + deformation
- **Path spec v2**: gains `claim_id`, `route_index`, `round_count`, `revision_brief` (the prior route's objections, inlined to the next repairer); states extended `Solving → Repaired → Challenged → {Settled | RevisionRequested | Pruned}`.
- Deterministic search policy lives in **aggregate_costs v2** (no LLM in the loop logic):
  - On `ChallengeComplete`: compute route cost. If `round_count < 2` AND any single flag ≥ 20 points → dispatch `Path.RevisionRequested` (same route revised; repairer gets the adversary's flags as a brief). Else `Path.Settled`.
  - On `Claim.RouteSettled`: if `best_route_cost > acceptability bound` (default: tail ceiling formula applied per-claim) AND `route_count < 3` → create alternate Path with `revision_brief` = "beat these specific objections via a different mechanism" → `SubmitForBridge`. Else settle the claim with the cheapest route.
  - **Pruning**: a route whose repairer-phase cost already exceeds `best_route_cost × 2 + 20` is marked `Pruned` without spending an adversary session.
  - Single-writer guards per claim (extend the v1 `canonical_path_id` pattern to `settled_route_id` on Claim); authoritative re-reads for projection lag (v1 lesson, pinned).
- **Deformation flag kind**: weight 25 (tunable prior, = contradiction), severity by distance moved (low = clarified, medium = narrowed scope, high = changed meaning). Adversary prompt updated to audit amendment honesty (compare `original_text` vs `current_text`).
- **Session budget math (hard bound, enforced by counters)**: per world ≤ endpoints(3) × claims(5) × routes(3) × sessions-per-route(3: repair+challenge+revision) = 135 theoretical max; defaults (2 endpoints, ~4 claims, mostly 1 route) ≈ **25–40 sessions/world typical**. World gains `session_budget_guard` counter; spawning WASM refuses past it. *Honest cost note: a full v2 e2e world run is ~30–50 gpt-5.5 sessions; the flagship phase (C6) will run 3+ worlds.*
- Golden-fixture tests for the search policy (same flags → same route tree → same verdicts, order-independent).

### C3 — Conditional edges in the evidence loop
- `evidence_ingest` v2: on a node resolving "no", walk `depends_on` dependents transitively (bounded depth, cycle-safe), mark them `Invalidated`, flag affected Claims, dispatch scoped `Claim.Reprice` (re-runs costing from recorded flags; spawns nothing) and `World.BeginUpdate` only when a settled claim flips. Unit tests on the graph walk (pure function over fixture edges).

### C4 — Living worlds
- **New WASM `animate_dwellers`** on new `World.AnimateDwellers` (dispatched by render_artifacts completion or operator): one casting session proposes 2–3 personas grounded in the canonical claims → WASM creates Dweller entities → one session per dweller: traverse the settled routes (nodes + claims inlined), self-report `RecordTraversal` / `RecordContradiction` (existing actions; **contradiction reports append to the claim's challenge flags** — dweller stress-testing feeds pricing), author a first-person `kind: story` Artifact → `SubmitForCheck` → existing gate → Publish. Cedar: relay permits exist for Record*; add story-author relay coverage if gaps appear in the matrix test.
- **DSF UI** (deep-sci-fi worktree `codex/dsf-2`): world page gains **Claims panel** (per-claim verdict: text, amendment badge, cheapest route cost, route count) and **"The imagined future" section** (per-endpoint stance + bundle rendered via the existing Files/$value proxy path); stories feed lights up with dweller stories; nav cleanup (drop 1.x tombstone links from header/footer); standings gains per-claim counts. Playwright specs updated (mapping gate enforces this).
- **Evidence cadence**: cron entity (paw-agent pattern) dispatching `IngestEvidence` weekly on live worlds. **Retcon drill** in C6: admin-resolve a required node "no" on a copy world → verify BeginUpdate → re-route → artifact Retconned → "What changed" renders with cause + successor.
- Dials: default `endpoint_budget` 3 (5 for flagship), stances extended to 5 distinct positions.

### C5 — Judged resolution + calibration
- **New WASM `adjudicate_nodes`** + adjudicator soul: for overdue non-market nodes (the v1 "needs judgment, leaving open" branch): spawn adjudicator session — web evidence (live mode), snapshots to paw-fs, rubric in soul — self-reports `Resolve(outcome, evidence_refs, confidence)`. Confidence below threshold → node stays open with `needs_human_resolution=true` surfaced in DSF standings (NOT the Cedar pause path — `request_approval` module gap is a known platform bug, chip filed). Hindcast mode: adjudicator never runs (actuals file remains gold standard).
- **Hindcast library**: 3 new fixtures beside `hindcast-2025h2/` (candidates: 2024-H2 AI coding tools; 2025-H1 LLM API market; 2023-H2 developer tools — each corpus.md + actuals.json, human-authored). Harness runs the suite.
- **Calibration**: offline grid search over (kind weights incl. deformation, severities, decay 25, ceiling 2×+20, resolve thresholds .95/.05) **recomputed deterministically from recorded flags + graded outcomes** of the library runs — no LLM re-runs per grid point. Output: chosen constants + receipts committed as a calibration report; `ENGINE_VERSION` bump stamps the regime change on future forecasts.

---

## Track D — Fidelity (the four gaps the first live run surfaced)

Same worktree/PR as Track C. Each phase is red-green TDD with a live local exercise; ADR-005 (grounding) and ADR-006 (diversity) written first. A shared **embedding capability** lands first because three phases consume it.

### D0 — Crash resilience (unblocks everything; do first)
- All local runs use the **release build** (`./target/release/temperpaw-server`); the debug build cannot take v2 concurrency. Record this in the proof runbook.
- **Self-healing corridors** so a restart never needs hand-salvage: a `Path.state_timeout` on Solving/Repaired/Challenged (ADR-0050) that re-spawns the dead session's work; a World/Claim reconcile sweep on boot that re-dispatches `SubmitForBridge`/`RequestChallenge` for routes whose sessions died (recovery already inspects sessions — extend it to re-drive corridor entities). Verify by killing the server mid-run and confirming the world settles without manual dispatch.
- Fold the in-flight run-1 world to a clean terminal state (it has served its purpose as the gap-finding run; the clean flagship comes from D-improved code).

### D1 — Embedding capability (shared)
- One integration: `embed` WASM module + `embedding_model` secret (pinned model id stamped on every vector for reproducibility); pure cosine-distance/clustering helpers in Rust, unit-tested with golden vectors. Distances and thresholds are deterministic; only the vector fetch is external (same shape as web search). Cedar egress permit + secret access, mirroring the corridor-module allowlist.

### D2 — Grounding (G1)
- **Present-state brief**: the surveyor (web search in live mode via `exa_api_key`; training-knowledge fallback otherwise) writes a dated "Present state as of <today>" file of determined facts; it is inlined into every repairer/endpoint-writer prompt (the wall-9 inline pattern) and seeds determined EventNodes.
- **Reconcile/dedup pass** (deterministic + embeddings from D1): an authored node whose claim embeds within ε of a determined/present fact is collapsed to `determined` (p≈1.0), not registered; `register_forecasts` refuses any node already covered by a determined fact. Pin with a test: "first-party agents already compete" → determined, never a 0.55 forecast.
- **Wire the lag table** into the repairer's date contract so dates derive from historical durations, not round-number intuition.

### D3 — Diverse-world sampling (G2)
- Surveyor names the **top-K load-bearing uncertainty axes** + the consensus pole of each (the consensus reference).
- `sample_endpoints` becomes a **portfolio sampler**: one consensus anchor + worlds placed to *cover* the named axes (not two arbitrary stances), each anti-consensus world inverting a *named* axis.
- **Diversity gate** (embeddings, D1): embed each world's claim-set; enforce minimum pairwise distance via farthest-point selection; re-steer/regenerate a collapsed world *before* the corridor spends sessions — so budget only ever buys distinct, on-axis worlds. World budget configurable (default 3, flagship 4–5).

### D4 — Synthesis + embedding-matched grading (G3 + needles)
- **World-page synthesis panel** (DSF): cluster claims across a world's endpoints (D1 embeddings) → "what the futures agree on" (cross-endpoint clusters = near-inevitable), "where they diverge" (singletons, by axis), each future as a one-line headline + stance/axis badge, and the leading-indicator nodes the cheap shared claims depend on. The raw bundles stay one click deeper.
- **Hindcast matching**: `grade_hindcast` matches actuals to forecasts by nearest embedding, retiring the brittle ordering-sensitive substring needles (and the fixture README caveat).

---

### C6 — Flagship local e2e (the v2 proof, and the headline deliverable)
Runs on the **release build**, on the Track-D-improved engine, all surfaces verified in the local DSF UI:
1. Six-month world (AI coding tools, budget 3): grounded skeleton (no already-true forecasts) → diverse on-axis worlds (diversity gate passed) → decomposition → routes (≥1 revision or alternate-route) → per-claim verdicts → forecasts → dweller stories Published → claims + bundles + **synthesis panel** in UI.
2. **The flagship world** (2045 fiction path): the world named in the Definition of Done — fully engine-created, zero hand-filled content, ≥2 dwellers each with a Published first-person story through the gate, traversals/contradictions on their track records, surfaced bundles, per-claim verdicts, synthesis panel, and a retcon drill on a copy. Acceptance = a recorded walkthrough where every visible artifact traces to an engine session (session IDs in the proof) and the run completed **without manual salvage**.
3. Hindcast suite: all 4 fixtures green via embedding-matched grading; calibration report produced; constants updated with receipts.
Proofs per TEMPLATE.md with session counts and costs.

### C7 — One deploy (carried A7+A8)
Merge order: temperpaw PR (searched corridor) → DSF PR. Then: Railway Postgres snapshot **[Rita]** → `gh workflow run railway-redeploy.yml -f image_tag=edge` **[Rita or with Rita present]** → /readyz + Datadog reconcile proof (zero-error baseline recorded 2026-06-12) → prod Cedar prune script → Codex device login on prod **[Rita]** → prod smoke world (small budget) → full live world. Genesis sync + pinned-ref verification. Old-entity readability check (A2 rehearsal pattern).

### C8 — Cutover (carried B3/B5/B6)
B3 archive export (1.x Postgres → published paw-fs + /archive) → staging flip → full walkthrough (worlds/claims/stories/standings/retcon/interview + no-token network assertion) → production flip → FastAPI tombstone (410 + skill.md pointer) → 2–4wk monitor → decommission.

## Risk register (delta from v1)

| Risk | Mitigation |
|---|---|
| Session-count blowup (search multiplies spawns) | Hard counters on World/Claim/Path; pruning before adversary spend; defaults small; budget guard refuses past cap; costs reported per proof |
| Drift-to-consensus via free amendment | Deformation flags priced (commitment #4); adversary audits original vs current text; amendment only via explicit diff-carrying action |
| Search loops stall (event-driven, no orchestrator) | Every wait state has `state_timeout` (ADR-0050); self-loop actions carry `max_checks`; matrix test asserts no unreachable states |
| `Claim` CSDL collision | Gate in C0 + CI uniqueness test (action-name hits in paw-patrol are not collisions) |
| Adjudicator hallucinating resolutions | Confidence threshold + evidence_refs required + human-flag fallback; hindcasts never use it |
| Calibration overfits 4 fixtures | Report states n; constants remain "priors v2"; ordering constraint (miracle>contradiction>incentive≥lag) enforced in the grid |
| In-place 0.2→0.3 evolution | Same A2 rehearsal: seed 0.2 entities locally, install 0.3, verify coexistence; never reuse retired names |
| Server abort under v2 concurrency (G4) | Release build only for runs; self-healing corridor (state_timeout re-spawn + boot reconcile re-drive); kill-test in D0 |
| Embeddings mis-used as a truth/anti-consensus test | Embeddings gate diversity-between-samples only; consensus reference (named axes + modal anchor) is a separate mechanism; logical contradiction stays with the adversary LLM |
| Already-true facts registered as forecasts (G1) | Present-state brief + reconcile/dedup collapse to determined; register_forecasts refuses determined-covered nodes; pinned test |
| External deps (exa_api_key, embedding_model) absent | Both degrade to deterministic fallbacks (training-knowledge brief; if no embedder, fall back to exact/substring dedup + skip the diversity gate with a loud log) — never a silent pass |

## Carried-over side items (not on critical path)
Task chips: `request_approval` WASM gap, WorkCycle collision, mid-run WASM rebind, DSF git-hook codex args. The DSF git-hook codex-args breakage is now worth fixing (it nags every commit). Standing goal: *ship the searched corridor (paw-foresight 0.3) + living-worlds DSF 2.0 — grounded, diverse, readable, calibrated worlds with dweller stories — and produce one full engine-made flagship world, then one deploy and public cutover, verified live.*

## Sequence (adjusted trajectory)
D0 (crash resilience — unblocks) → D1 (embeddings) → D2 (grounding) → D3 (diverse sampling) → D4 (synthesis + grading) → **C6 flagship run on the improved engine** (the headline deliverable: a full engine-made world with stories + synthesis) → C7 deploy → C8 cutover. Track D phases are largely parallelizable across worker agents once D0+D1 land; C6 is the barrier that gates C7.

## Critical files
- Track C (built): `os-apps/paw-foresight/specs/{claim,path,endpoint,event_node,world}.ioa.toml`, `wasm/{decompose_endpoint,animate_dwellers,adjudicate_nodes,spawn_repairers,spawn_adversaries,aggregate_costs,evidence_ingest}`, `policies/foresight.cedar`, `crates/temperpaw/tests/corridor_*.rs`, DSF `platform/app/world/[id]/page.tsx`, `platform/lib/{api,odata,standings,claims}.ts`, `platform/e2e/*`.
- Track D (new): `os-apps/paw-foresight/wasm/embed(new)`; `seed_world` (present-state brief + named axes), `sample_endpoints` (portfolio sampler + diversity gate), `spawn_repairers`+`register_forecasts` (reconcile/dedup, lag table), `grade_hindcast` (embedding match); `crates/temperpaw/src/startup.rs` (boot reconcile re-drive); `path.ioa.toml` (state_timeout); DSF synthesis panel in `platform/app/world/[id]/page.tsx` + `platform/lib/synthesis.ts(new)`; ADR-005/006; `prove_corridor_e2e.py` (release build, self-heal assert).
- Secrets (tenant, [Rita]): `exa_api_key` (web search), `embedding_model` provider key.

## Verification
Per phase: red-green unit + contract tests; live local dispatch on the **release build** with OData state checks. D0's self-heal is proven by a kill-test (server killed mid-run; world settles unaided). C6 is the gate for C7: all three flagship runs green with proofs, **completed without manual salvage**, the flagship world showing grounded skeleton + diverse on-axis worlds + dweller stories + synthesis panel. C7 verified via /readyz, Datadog (reconcile + zero new errors vs the 2026-06-12 baseline), prod OData spot-checks. C8 verified by the recorded public walkthrough. Nothing reported done without being run and seen.
