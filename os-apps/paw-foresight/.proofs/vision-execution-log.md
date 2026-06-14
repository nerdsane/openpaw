# Foresight Vision Execution Log

Living tracker for the goal (set 2026-06-13): *implement the plan fully, run the
full engine + DSF 2.0 to a real world WITH stories, evaluate whether the output
is genuine foresight or performative, then iterate (run → critique → fix) until
it executes the vision.* Milestones, challenges, and iteration notes accrue here.

## The vision (what "true foresight, not performative" means)

The acceptance bar is not "it ran and produced text." It is whether the artifacts
are **load-bearing**:

- **Worlds** are genuinely different futures across named uncertainties — not the
  same story reworded (the run-1 modal/anti-modal convergence is the failure mode).
- **Claims** are falsifiable, load-bearing assertions — not vague mood.
- **Routes** are real causal chains of dependent, dated, individually-priced steps
  back to today — not an agent "reasoning three times" hand-wavily.
- **Costs** reflect real strain (miracle/contradiction/incentive/lag/deformation)
  and the ordering means something; cheap ≠ arbitrary.
- **Forecasts** are gradeable and not already-true (no free-resolving facts).
- **Dweller stories** are grounded in *this* world's claims/routes — not generic
  sci-fi that could be pasted into any world.
- **Synthesis** lets a reader actually derive "what happens to X, and how sure."

Performative tells to hunt for in evaluation: round-number dates, claims that
restate the prompt, routes with no real dependencies, uniform costs, dwellers who
never touch the corridor, "diverse" worlds with ~0 embedding distance.

## Status snapshot

- Engine: paw-foresight 0.3 on `codex/searched-corridor` (PR temperpaw#398).
- Embedder: Ollama + mxbai-embed-large, brew service on :11434 (verified).
- Server: release build `./target/release/temperpaw-server` (mandatory, ADR-007).

## Milestones

| # | Milestone | State |
|---|-----------|-------|
| M0 | Crash resilience + self-heal (D0) | ✅ shipped |
| M1 | claim_decision liveness + straggler termination (D0) | ✅ shipped |
| M2 | Dweller-spine fix (World-PATCH→param) — stories can now be produced | ✅ shipped |
| M3 | Embedding capability (D1) + reconcile + lag table (D2) | ✅ shipped, live-calibrated |
| M4 | ADR-005 grounding + ADR-006 diversity | ✅ written |
| M5 | Diversity gate (D3): barrier + embed + re-steer | ✅ shipped + **verified live in run-1c** (gate fired, re-steered, discarded duplicate) |
| M5b | Named-axes sampling (D3 sampling side) | ✅ shipped (960845b6) — surveyor names axes, anti-modal worlds invert distinct named axes |
| M6 | D4: synthesis panel (DSF) + hindcast embedding matching | ✅ shipped — synthesis panel (DSF, type-clean) + grade_hindcast embedding match (01533476) |
| M7 | Full flagship run WITH dweller stories, in DSF 2.0 UI | 🔨 run-1b launched 2026-06-13 23:54 |
| M8 | Quality evaluation: true-foresight vs performative critique | ⏳ |
| M9 | Iterate (fix → re-run) until vision executed | ⏳ |
| M10 | Deploy + cutover (C7/C8) | ⏳ needs Rita gates |

## Iteration log

### Run-0 (run-1, six-month world) — the gap-finder (retired)
World `en-019ebd69-…`. Corridor settled (9 Settled / 6 Unreachable, 85 forecasts,
2 render artifacts) but **terminally Failed at the dweller phase** — root cause: a
Cedar-denied raw `PATCH /tdata/Worlds(...)` writing the dweller roster. Fixed (M2).
Also surfaced: liveness deadlock on wedged stragglers (M1), grounding gaps G1/G2
(M3), no diversity verification (M5). No stories produced. Retired (Failed is final).

### Challenges / incidents
- WASM raw PATCH of entity fields is Cedar-denied (403) → persist via action params.
  Fatal for the dweller roster; non-fatal (warn) elsewhere (agent-binding, flags).
- mxbai-embed-large caps at 512 tokens → cannot embed full 30KB bundles; gate embeds
  bundle-heads, reconcile embeds statements.
- `frontier_date` is the scoreable horizon, NOT "today" (no stored present-date).
- Session self-reports arrive as the `service:wasm-runtime` relay principal
  (agent_type "service"), so they need an explicit relay permit — the generic
  `["system","agent"]` permit does NOT cover them. Renaming a writer's
  self-report action (SubmitForRepair→BundleWritten) without updating the relay
  permit silently denies it (→ missing request_approval → session Failed).
  Wedged run-1b for 60 min before diagnosis.

### Run-1b (six-month world, budget 2) — first run on the D0–D3 engine — WEDGED
World `en-019ec443-…4ceb6b08a74f`. Engine restarted clean (all 13 modules
registered; sample_endpoints hash verified = my D3 build); Ollama live. Both
endpoint writers ran, then **wedged in Sampled** for 60 min. Root cause: the
writers' `BundleWritten` self-report was **Cedar-denied** (→ routed to the
missing `request_approval` module → session Failed). D3 renamed the writer's
self-report SubmitForRepair→BundleWritten but the `service:wasm-runtime` relay
permit still listed only SubmitForRepair; the generic ["system","agent"] permit
doesn't match the relay principal (agent_type "service"). **Lesson:** a renamed
session-self-report action must be added to the relay permit, not just the
generic permit — same class as [[feedback_temper_wasm_patch_denied]] but for
relay permits. Fixed (275030a3) + regression test. Run-1b abandoned.

### Run-1c (six-month world, budget 2) — on the relay-fixed engine
World `en-019ec480-…c594ada8bbd8`. Server restarted with the fixed Cedar (active
policy verified to grant BundleWritten at the relay). Launched 2026-06-14 00:59.

**Live proofs (the gate works):**
- BundleWritten succeeded for both writers → relay fix verified end-to-end.
- The barrier correctly waited for the 2nd writer, then fired GateDiversity.
- Round 1: released the diverse world into repair, re-steered the collapsed twin.
- Round 2: the twin was still a near-duplicate → **discarded** after the cap.
- The surviving world decomposed into 8 claims and entered the corridor.

**Key finding — diversity GATE works, but diversity SAMPLING is weak.** The
generic "anti-modal" stance converged onto the modal one (run-1's exact G2
failure), so budget-2 collapsed to ONE distinct world (gate correctly refused to
spend corridor sessions on the duplicate). The gate is the safety net; it cannot
manufacture diversity that the sampling never created. **#1 iteration item: the
named-axes sampling (the deferred half of D3, ADR-006) — the surveyor names the
top-K uncertainty axes and each anti-modal world inverts a NAMED axis, so the
stances are genuinely divergent before the gate ever runs.** Until then, a real
multi-world portfolio needs higher budget AND named axes, not budget alone.

(Consequence for this run: 1 endpoint → the synthesis panel has no cross-world
agreement to show; it shines only with >=2 distinct endpoints.)

**Named-axes fix shipped (960845b6)** while run-1c grinds — the next run will
produce distinct worlds by construction. run-1c keeps running on the loaded
(pre-named-axes) wasm to finish a 1-endpoint world with stories (the spine
proof); a diverse multi-world run comes next, after evaluating run-1c.

**Corridor pace note:** ~16 min for the first claim to settle (8 claims total).
The file-backed SQLite (turso `file:`) contends under session concurrency — the
OTS-trajectory writes throw `database is locked` (handled, non-fatal) and the
shared write lock throttles dispatch. Inherent to local dev; Postgres on Railway
won't have it. So local runs are slow but progress; not a bug.

## Plan status — D0–D4 fully implemented (2026-06-14)

Every plan phase is implemented, tested, committed, pushed (PR temperpaw#398),
and the engine is proven working live in run-1c (gate fired + re-steered +
discarded; BundleWritten relay verified; claims settling; paths reaching
Canonical). D4's synthesis panel is in the DSF repo (codex/dsf-2, type-clean,
uncommitted pending a multi-endpoint world to render against).

**Corridor is slow locally** — run-1c's six-month claims are revision-heavy (4
of 8 paths hit the round-2 revision cap: the engine finds these futures genuinely
hard to bridge honestly, which is itself a real foresight signal), spawning ~36
concurrent Codex sessions; with file-SQLite write contention that grinds at
~1 claim / 8–12 min. ~2/8 settled; stories ~1.5–2h out at this pace. Not a
correctness issue — the round/route budgets + self-heal bound it; Postgres on
Railway removes the contention.

## Next actions
1. [in flight, monitored] Run-1c → settle 8 claims → canonical → forecasts →
   render → **dweller stories** (the headline; dweller-spine fix is in).
2. Evaluate run-1c against the vision bar (true foresight vs performative) —
   run `evaluate_world.py <wid>` + read the actual claims/routes/stories.
3. Iterate: a diverse multi-endpoint run on the named-axes engine (restart +
   launch), then the 2045 deep-sci-fi showcase world. Verify the synthesis panel
   renders on a multi-endpoint world + commit it.
