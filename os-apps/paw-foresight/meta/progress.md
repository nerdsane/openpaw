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

## Convergence Status

**Status:** Run 000 complete (measurement-only). Tournament begins at Run 001.
**Converged:** No
**A-wins streak:** 0
**Current engine version:** foresight-v100-base
**Note:** Post-Track-1-reliability measurement complete. `meta/runs/000_measurement_baseline/engine-output/synthesis.md` is the incumbent Run 001 must beat. Diagnosis identifies 7 leverage points; priority is forcing multi-step projection via a `Projection.Complete` invariant (see Run 000 diagnosis for full ranking).
