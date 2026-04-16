# Run 001 Diagnosis — `Projection.Complete` guarded on `current_step > 0`

## Summary

**Borda: challenger 54.0 / incumbent 54.0 / max 72 → TIE → incumbent wins (tie-breaker).** Raw per-judge totals also average 29/48 on both sides.

The one-line spec change (`guard = "current_step > 0"` on `Projection.Complete`, `os-apps/paw-foresight/specs/projection.ioa.toml`) worked at the architectural layer — the engine ran **two full steps** for the first time in this era, producing 25 observations (vs Run 000's 16), 6 directions (vs 4), and a 34,431-byte synthesis (vs 28,911). `AdvanceStep` fired; step-1 probes re-spawned; `ProjectionUpdated` carried a real `projected_state_file_id`. But the extra depth did not differentiate in a way the judges could score higher under rubric v4: the per-criterion Borda deltas cancel to zero.

## Per-Criterion Scoreboard (challenger − incumbent)

| Criterion | Δ (Borda) | Note |
|-----------|-----------|------|
| Specificity | −1 | Incumbent's dates/thresholds were seen as crisper (Cedar/OPA/AGENTS.md specifics; quantified PR-share predictions). |
| Novelty | **+2** | Judges credited external framings (METR mergeability, verification-as-carrying-capacity, two-lane architecture, OWASP GenAI Top 10) as novel on challenger side. |
| Falsifiability | 0 | Both hit 4 on several predictions; no edge. |
| Breadth | 0 | Both cover comparable theme counts; cross-theme interactions felt similar. |
| Plausibility | 0 | Judges saw both as grounded in observations; no separation. |
| Progression | **−1** | Judges felt step-1's "Revisions to earlier predictions" extended the story but didn't genuinely REVISE earlier claims — temporal evolution was shallow despite step 1 running. |
| Actionability | **−1** | Incumbent's decision points cited concrete dollar/FTE costs; challenger's were more abstract. |
| Decision Clarity | 0 | Both opened with prioritized #1 decisions. |
| Completeness | 0 | Both cover the full pipeline; model-projector depth not visible to judges. |
| Grounding | 0 | Both cite observations + external URLs. |
| Challenge | **+1** | Challenger's METR/OWASP/Lovable-dev-backlash counter-narrative read as a source-contradicting challenge. |
| Information Density | 0 | Both had some padding; neither won. |

**Net:** +2 + 1 = +3 on challenger; −1 − 1 − 1 = −3 on incumbent → sums to zero, hence tie.

## What worked

### 1. Guard enforced multi-step execution

The spec change was observable in events:

```
step 0: ProbesReady → 3× ProbeStepDone → ConvergenceComplete → ProjectionUpdated → AdvanceStep
step 1: ProbesReady → 3× (×2) ProbeStepDone → ConvergenceComplete → Complete
```

`current_step=1` at Complete time. Previously (Run 000) `current_step` never left 0. The orchestrator could not have short-circuited without a `Projection.AdvanceStep` dispatch — the state machine enforced the precondition.

### 2. Step-1 probes produced genuinely new observations

Challenger gained **Novelty** because the step-1 probes (day-365 horizon) surfaced framings the step-0 probes didn't:

- verification-as-carrying-capacity (step-1 adjacent-domain probe; Run 000 stopped at step-0 scope)
- stigmergic coordination via shared artifacts (step-1 adjacent-domain probe)
- METR mergeability gap (step-1 critic probe, citing METR's March 2026 note)
- two-lane operating model (step-1 practitioner probe)

These are all observations produced *after* the AdvanceStep, so they exist because the guard fired.

### 3. Challenge improved

Challenger's critic-probe observations in step 1 made specific claims about prompt injection (OWASP GenAI Top 10) and benchmark theater (METR findings) that judges read as source-contradicting. The source essay favors an optimistic autonomy narrative; challenger directly called that narrative fragile. Run 000's convergence analyst had weaker external challenge; step-1 critic added the needed counter-evidence.

## Why it's still a tie (and tie → incumbent per program.md)

### 1. Specificity regressed (−1)

The incumbent had 5 dated predictions spanning 2026-06-30 to 2026-12-31 with specific policy tools (Cedar, OPA, AGENTS.md), percentage-share predictions (39% PRs, 12% widespread use), and stripe-specific recommendations. Challenger's specific named actors (Codex, Claude Code, Copilot) were covered, but the quantitative thresholds (~50% non-merge, 4 verification assets, 3 coordination metrics) were less tightly bound to concrete dates.

**Root cause in engine:** step-1 probes inherit the projected state file, not the incumbent's quantitative predictions. When they regenerate predictions, they re-cite observations but don't propagate numeric thresholds forward. A spec/Cedar invariant forcing each step-1 Direction to cite a `numeric_threshold` field (or a WASM post-processor that enriches each Direction with a dated trigger) would address this.

### 2. Progression did not move (−1)

The most surprising result. Judges reported:

> "Later-phase revisions mostly restate that the earlier framing was incomplete rather than showing later predictions causally depending on what the earlier framing observed." (Judge 3)

> "Both structure 4 temporal phases with explicit 'Revisions to earlier predictions' subsections that revise prior claims" (Judge 2, capped 2 by 3+ rule)

So the 4-phase Temporal Progression structure is authored, but after reading both outputs the judges saw it as largely cosmetic: the incumbent also has "Revisions to earlier predictions" sections. Since the incumbent wrote those in a single-step synthesis, their structural parity with the challenger's two-step synthesis means *running step 1 did not materially change how the synthesis reads*.

**Root cause in engine:** the orchestrator's Final Synthesis section (in `orchestrate-projection/SKILL.md`) authors the 3–6/6–9/9–12 month phases in one pass regardless of how many steps ran. The step-1 observations enrich the evidence pool but the phase structure is LLM-authored from scratch at the end. For Progression to move, each step should itself produce a narrative rollup that the next step explicitly revises, not a single end-of-loop synthesis.

### 3. Actionability regressed (−1)

> "Incumbent's decision points named concrete dollar/FTE costs ($20K-60K annual observability, 0.5-1 FTE review burden, median task time 30 minutes). Challenger's were more abstract." (Judge 3)

**Root cause in engine:** nothing in the engine asks a probe to attach dollar or FTE numbers to observations. The incumbent happened to include those because the single long-running orchestrator had more room in its one synthesis pass; two-step probes each produce 3–6 short observations under pressure and don't fill in cost tradeoffs.

### 4. Double ProbeStepDone in step 1

6 `ProbeStepDone` events in step 1 vs 3 in step 0. Looking at the orchestrator + probe transcripts: step-1 probes self-dispatched `ProbeStepDone` (per probe prompt step 6: "Report done: temper.action Projections ProbeStepDone"), AND the orchestrator also dispatched `ProbeStepDone` in its Step 3 "Read Observations" block. Double-reporting is idempotent but pollutes audit trail. Not scored, but worth a cleanup.

**Root cause:** both the probe prompt (in `SKILL.md` probe template, step 6) and the orchestrator prompt (step 3 "Report each probe done") include a `ProbeStepDone` dispatch. Pick one.

### 5. Only 6/7 sessions emitted OTS trajectories

`ss-019d9887-811c` (probe_practitioner_step1) did not emit to `ots_trajectories`. The other 6 (orchestrator + 5 other probes) did. Session completed in 3 turns with outcome success per fields but no emit_ots_trajectory row. Not scored; minor tooling concern for next round's diagnosis.

## Recommended Changes for Next Iteration

**Priority 1: Make Progression scorable by making each step's synthesis cumulative.** Change `handle_projection_updated` (or add a new `handle_step_done`) to write a per-step rollup file (e.g. `step_N_summary.md`) containing "predictions made this step" + "predictions from prior steps that still hold / are revised / are falsified". The final synthesis then assembles these in order, giving judges a visible progression chain. This is architectural (WASM emitting step-scoped artifacts) rather than a prompt tweak.

**Priority 2: Attach quantitative-threshold invariant to Direction.Propose.** A Cedar policy requiring each Direction to have a non-empty `counterfactual_summary` AND at least one of (`numeric_threshold`, `dollar_cost`, `fte_cost`, `timeline_date`) would regain the Specificity / Actionability points. This extends the existing Direction spec; no new WASM.

**Priority 3: Deduplicate ProbeStepDone dispatches.** Trivial prompt fix — remove the orchestrator's step-3 dispatch since probes self-report. Or make `ProbeStepDone` idempotent at the WASM level (reject duplicates). Small change; clean audit trail.

## Note on HEAD drift for verify_run.py #4

This run's X/Y assignments were derived from `HEAD = 230c4e868e79abae2d12179ee4dead565d055aa6` (the post-plan.md-commit HEAD, not the pre-round HEAD in the task prompt). When `verify_run.py` is invoked by `loop.sh`'s `enforce_post_round`, it calls `git rev-parse HEAD` at verify time — which will be this commit's SHA, post-diagnosis. For #4 to pass, the `--head-sha` passed to `verify_run.py` must equal the SHA used for X/Y derivation. If the harness passes a different HEAD, the check will produce a mismatch; locally running `verify_run.py --head-sha 230c4e86...` (or whatever HEAD the scoring was done under) should pass. Flagging for the loop harness author.

## Artifacts committed this run

- `plan.md` (Scope: `specs/projection.ioa.toml`, `meta/progress.md`)
- `changelog.md`
- `engine-output/synthesis.md` (34,431 bytes — provenance-verified)
- `engine-output/observations.json` (25 entries)
- `engine-output/directions.json` (6 entries)
- `engine-output/projection.json` (entity snapshot, Complete, `current_step=1`)
- `transcripts/MANIFEST.md` (identifies `orchestrator ss-019d9883-8987-7a01-a3cf-842c62eaac93`)
- `transcripts/orchestrator.jsonl` + 6 probe JSONLs (step 0 + step 1)
- `trajectories/*.ots.json` (6 of 7 — one step-1 probe didn't emit)
- `judge_{1,2,3}_raw.json` + matching `_attempt.log` (empty → invocation succeeded)
- `scores.json` (`self_scored=false`, template sha matches, HEAD-derived X/Y recorded)
- `borda.json` (tie, 54/54/72)
- `diagnosis.md` (this file)
