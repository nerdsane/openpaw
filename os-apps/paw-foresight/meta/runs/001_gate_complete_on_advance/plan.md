# Run 001 Plan — Gate `Projection.Complete` on `current_step > 0`

## Scope
- os-apps/paw-foresight/specs/projection.ioa.toml
- os-apps/paw-foresight/meta/progress.md

Only the projection spec's state-machine action set is touched. The
`progress.md` update is the row-append for this run. No WASM, no Cedar, no
skill, no other file.

## Target Criteria

From Run 000 diagnosis's Priority 1 (force multi-step progression):

- **Progression** (criterion 6) — the whole "later predictions revise earlier
  ones" axis is dead when the engine runs a single step. Anchor 3 explicitly
  rewards "each phase causally depends on prior phases AND later phases
  explicitly revise, qualify, or strengthen earlier predictions based on what
  changed." A one-shot synthesis can't earn that.
- **Completeness** (criterion 9) — "full pipeline with explicit assumptions,
  limitations, and confidence levels." The model-projector spawn never happens
  in a single-step run, so `ProjectionUpdated → handle_convergence` →
  model-projector is dead code. Second step unblocks it.
- **Decision Clarity** (criterion 8) — multi-phase decision framing is the
  difference between "3–6mo / 6–9mo / 9–12mo" being authored in one sitting
  vs. emerging from observations accumulated at two different horizons.
- **Actionability** (criterion 7) — triggers observable at step-1's horizon
  (the 365-day mark in the current default schedule) only exist if step 1
  actually ran.

Secondary criteria that should move with the above: Info Density (less
repetition when later phases are actually incremental), Plausibility (more
observations = more grounding).

## Planned Change

Add a single-line guard to the `Complete` action in
`os-apps/paw-foresight/specs/projection.ioa.toml`:

```toml
[[action]]
name = "Complete"
kind = "input"
from = ["Running"]
to = "Complete"
params = []
guard = "current_step > 0"   # <-- NEW
hint = "All steps finished. Mark the projection as complete."
```

### Why this specific change

Run 000's orchestrator ran step 0 and then jumped straight to synthesis +
`Complete`, skipping the loop-body `AdvanceStep`. The `for step in
range(max_steps)` in the skill is a prose instruction; LLMs ignore prose ~80%
of the time (see `program.md` constraint 7). A spec-level guard is
state-machine-enforced — the orchestrator literally cannot dispatch
`Complete` until `current_step > 0`, which is only possible after at least
one `AdvanceStep` has fired (since `AdvanceStep` is the only action whose
effect increments the counter).

### Why NOT a stronger guard like `current_step >= max_steps - 1`

Guards in this spec format operate on counter/boolean comparisons with
literals (see `paw-pm/cycle.ioa.toml:49` `issue_count > 0`,
`paw-agent/specs/session.ioa.toml:591` `follow_up_count < 100`). Comparing a
counter to a string-typed field like `max_steps` isn't clearly supported.
`current_step > 0` reaches the same outcome under the current default
`max_steps=2` config while staying inside the proven guard vocabulary —
smaller blast radius than introducing a new state field, a new WASM, or a
counter-to-string comparison.

### Why architectural, not prompt

Per program.md constraint 7 and the foresight-meta skill's
"Prefer architecture" rule, enforce via spec invariants before SKILL.md
edits. This change removes the LLM's ability to skip — it doesn't try to
convince it not to.

## Expected Impact

- `current_step > 0` at `Complete` time → `AdvanceStep` dispatched at least
  once → step-1 probe fan-out runs → model-projector spawn reached →
  synthesis has two horizon rollups (step-0 @ 90d and step-1 @ 365d, per
  current `step_schedule` default).
- **Progression** should move from 1 → 2 (likely) or 3 (stretch). Needs
  later phases to actually revise earlier ones, which depends on whether the
  orchestrator's step-1 logic uses step-0 observations as context — a
  downstream question, but one that only matters *after* this unblocks step 1.
- **Completeness** should move from 2 → 3 once the model-projector writes
  an actual `projected_state_step_0.json`.
- **Progression + Completeness** shifting is the minimum bar for a challenger
  win; other criteria (Specificity, Grounding, Breadth, Info Density) will
  drift based on whether the extra observations are higher-signal than step 0's
  or just more of the same.
- Domain-agnostic: the guard is a counter check, not a DSE-specific assertion.
  Works for any knowledge-graph domain that runs a multi-step projection.

## Risks & Mitigations

- **Risk:** orchestrator can't recover from a rejected `Complete` dispatch
  (no AdvanceStep fired, guard rejects, no retry logic).
  **Mitigation:** if this happens, the failed dispatch surfaces in the
  orchestrator JSONL and the synthesis either still gets written (stuck
  mid-session, heartbeat-times-out) or doesn't (no challenger output, run
  fails cleanly). Either outcome is a stronger diagnostic signal than a
  "succeeded but skipped" run. Run 002 can then layer a prompt reinforcement
  on top if needed.
- **Risk:** the TOML guard doesn't parse against the Temper guard evaluator,
  app install fails.
  **Mitigation:** the syntax is copy-equivalent to proven guards in
  `paw-pm/cycle.ioa.toml`, `paw-agent/specs/session.ioa.toml`. If install
  fails during the engine run, the loop aborts with a clear error and the
  run is reverted by loop.sh.
- **Risk:** step 1 doesn't actually produce *better* observations, and
  scores don't move.
  **Mitigation:** still an informative run — Run 002 can target the next
  ranked issue in the Run 000 diagnosis (Issue 2: probe shallowness, or
  Issue 3: confirmation-rate gate).

## Execution

1. Write + commit `plan.md` (this file) — anchor for loop.sh revert on loss.
2. Edit `os-apps/paw-foresight/specs/projection.ioa.toml`: add `guard = "current_step > 0"` to the `Complete` action.
3. Reinstall app via `POST /api/os-apps/paw-foresight/install` (spec hot-reload, no WASM rebuild needed).
4. Create Projection with `foresight_model_id = en-019d92cd-41e7-7aa0-8436-e0532786bfcf` (DSE v2), `horizon = "1 year"`.
5. `Start`, poll for Complete/Failed up to 20 minutes.
6. Extract synthesis.md (provenance-verified), observations.json, directions.json.
7. Extract all session transcripts from isolated SQLite DB into `transcripts/`. Write MANIFEST.md with `orchestrator ss-<uuid>` line.
8. Query `ots_trajectories` natively; fall back to converter only if missing.
9. Run 3 judges via `claude -p` + locked template + deterministic X/Y from `assign_judges.py`.
10. `borda.py --run-dir ...` to produce borda.json.
11. Write `diagnosis.md` after reading ALL transcripts.
12. Append row to `progress.md`.
13. Self-run `verify_run.py`. Commit. Plain push. No tag (loop.sh owns that).
