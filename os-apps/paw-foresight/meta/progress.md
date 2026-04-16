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
| 001 | (pending tag decision) | 29/48 | 29/48 | 0 | 54.0/72 | 54.0/72 | incumbent (tie → tie-break) | 1 | `guard = "current_step > 0"` on `Projection.Complete` in `specs/projection.ioa.toml` — forces multi-step progression | Structural win (engine ran 2 steps for the first time: 25 obs vs 16, 6 directions vs 4, 34KB synthesis vs 29KB) but scoring tie. Challenger gained Novelty (+2) and Challenge (+1) from step-1 external-signal probes; regressed on Specificity (−1), Progression (−1), Actionability (−1) because the orchestrator's final synthesis still authors phase structure in one pass regardless of step count, and step-1 probes don't propagate quantitative thresholds. Next iteration: make each step emit a cumulative summary so progression is visible to judges. |

## Convergence Status

**Status:** Run 001 complete. Tournament underway.
**Converged:** No
**A-wins streak:** 1 (one more incumbent win → converged)
**Current engine version:** foresight-v100-base (Run 001 change reverted on loss per loop.sh enforce_post_round)
**Note:** Run 001 proved the architectural change (spec guard `current_step > 0` on `Projection.Complete`) works — the engine now runs multi-step and produces deeper evidence — but per-criterion Borda ties net to zero because the synthesis's phase structure is still authored end-of-loop, not per-step. Run 002 should target Progression by emitting per-step rollups (architectural) rather than tweaking the orchestrator prompt.
