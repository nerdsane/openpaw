# Foresight Meta-Improvement Progress

## Era

**Post-Track-1-reliability** — engine rebuilt against temper `b73c421` after merging Track 1 session reliability (heartbeat/steering/checkpointing/fuel), `session_recoverer` mid-turn checkpoint resume, and Track 3 OTS trajectory emission with native paw-foresight consumer.

Pre-Track-1 runs are archived under `meta/archive_v4_pre_track1/` for historical reference. Their scores do NOT carry over — the engine has changed meaningfully enough that cross-era score comparison is not valid.

## Rubric & Methodology

- **Rubric:** v4 — 12 criteria (Grounding replaces Transparency, Information Density replaces Quantitative Precision) — see `program.md` (immutable)
- **Judges:** 3 Claude Code subagents (`claude -p`), side-by-side blind comparison, **deterministic** X/Y via `tools/assign_judges.py`
- **Judge prompt:** LOCKED template at `meta/judge_prompt_template.md` — sha recorded in each `scores.json`
- **Baseline (locked, re-seeded 2026-04-16 with web tools):** see `meta/baseline/scores.json` (27.0/48 raw, 54.5/72 Borda — single-shot reference with matched tool access, does NOT participate in tournament). Old no-web baseline archived at `baseline/archive_no_web/synthesis.md`.
- **Constraints:** domain-agnostic, no-authoring, prefer-architecture
- **Convergence:** 2 consecutive incumbent wins
- **Run 000:** measurement-only; first tournament round is Run 001 (incumbent = Run 000 engine output)
- **Integrity enforcement:** `meta/tools/verify_run.py` gates every round — 12 invariants, fail-closed (see `.claude/skills/foresight-meta.md` for the full list)

## Engine Base

- **Starting tag:** `foresight-v100-base` (openpaw `de4e0f89`)
- **Temper:** `b73c421` (main HEAD as of 2026-04-16)
- **Orchestrator max_fuel:** 120B (down from 500B band-aid — checkpointing makes this sufficient)
- **OTS trajectories:** emitted natively by `emit_ots_trajectory` WASM, no JSONL reconstruction required

## Score Table

| Run | Tag | Engine Score | Incumbent Score | Delta | Engine Borda | Incumbent Borda | Winner | Streak | Key Change | Key Insight |
|-----|-----|-------------|-----------------|-------|-------------|----------------|--------|--------|------------|-------------|
| 000 | foresight-v100-base | (measurement-only) | — | — | — | — | — | 0 | Post-Track-1 measurement baseline — no engine change | Engine ran single-step (max_steps=2 ignored), 16 obs + 4 directions + full synthesis (29KB). 37.5% observations confirmed; model-projector never spawned; directions stayed in Proposed. Diagnosis prioritizes forcing multi-step progression via Complete precondition. Incumbent for Run 001. |
| 001 | (reverted) | 29.0/48 avg raw | 29.0/48 avg raw | 0 | 54.0 | 54.0 | incumbent (tie) | 1 | `Projection.Complete` guarded on `current_step > 0` (spec invariant) | Spec change forced two-step execution for the first time, producing +Novelty and +Challenge from step-1 probes. But Progression did not move because the orchestrator's Final Synthesis still authored the 4-phase narrative in one pass. Net Borda tied; tie → incumbent per program.md. Code reverted by loop.sh. |
| 002 | (pending loss revert) | 28.7/48 avg raw | 29.0/48 avg raw | −0.3 | 53.5 | 54.5 | incumbent | 2 | Per-step `step_{N}_rollup.md` artifacts + "compose verbatim" Final-Synthesis contract in orchestrate-projection SKILL.md | Rollups were written as required and produced +2 Borda on Specificity (rollup evidence/anchor rows propagated). But the "compose verbatim" contract for the Temporal Progression section was advisory prose and the LLM ignored it — paraphrased rollups into the same 4-phase narrative. Progression unchanged; Novelty/Breadth/Info Density each −1 Borda. 2nd consecutive incumbent win → converged. Next round needs a WASM-assembled synthesis section (not LLM-authored) to close the advisory-prose loophole. |

## Convergence Status

**Status:** Run 002 complete. **2 consecutive incumbent wins → converged per program.md.**
**Converged:** Yes (2-round streak: Run 001 tie→incumbent, Run 002 incumbent by 1 Borda)
**A-wins streak:** 2
**Current engine version:** foresight-v100-base (Run 001 reverted, Run 002 change lost — incumbent is Run 001's engine-output/synthesis.md at 34,431 bytes)
**Note:** Two architectural attempts landed (Run 001 spec guard, Run 002 rollup artifacts) and both failed to move the tournament. The next iteration — if the human chooses to resume — must address the root cause flagged in BOTH diagnoses: LLM-authored phase narratives ignore prose composition contracts. The structural fix is WASM-assembled synthesis sections. See Run 002 diagnosis Priority 1.
