# Run 000 Plan — Post-Track-1-reliability measurement baseline

## Mode

**Measurement-only.** Per `meta/progress.md` and `.claude/skills/foresight-meta.md`,
Run 000 of a fresh era does NOT run judges, does NOT produce `scores.json` / `borda.json`,
and does NOT tag. Its sole job is to take the first engine output after a breaking
platform change and deposit it as the incumbent for Run 001 to score against.

The era begins at `foresight-v100-base` (openpaw `de4e0f89`) on top of Temper `b73c421`
with Track 1 session reliability (heartbeat, steering, mid-turn checkpoint resume,
120B orchestrator fuel budget) and Track 3 native OTS trajectory emission. No
pre-Track-1 score carries over.

## Scope
- os-apps/paw-foresight/meta/progress.md
- os-apps/paw-foresight/meta/tools/verify_run.py

`progress.md` gets this run's row. `verify_run.py` needs a small,
non-scoring-affecting fix: since the Track 1 / Track 3 session reliability work
landed, large message bodies are externalized to `content_file_id`-referenced
files rather than inlined into the session JSONL. The existing provenance check
(#2) only inspects the JSONL blob, so engine-produced syntheses whose final
tool-call code is externalized look "missing." The fix walks the JSONL, follows
each `content_file_id` pointer, and includes those blobs in the provenance
search — preserving the intent of the check (synthesis must come from the
engine, not hand-authored by the meta-agent) while actually working against the
current Temper file layout.

No engine code, spec, WASM, Cedar policy, or skill file is touched. If
subsequent extraction reveals the engine itself needs a fix just to produce a
valid output, the run will be aborted and re-planned with the fix declared in
Scope — not silently patched mid-run.

## Target Criteria

Not applicable — no change proposed, no score delta targeted. The run's
deliverable is the measurement itself (a Run 000 synthesis.md that Run 001 can
beat), plus a diagnosis of current engine weaknesses that Run 001 will use to
pick its one change.

## Planned Change

None. Fresh-era measurement.

## Expected Impact

- Establish the starting point for the post-Track-1 era.
- Capture transcripts and OTS trajectories so Run 001's diagnosis has raw material.
- Surface the current weak criteria (expected: Grounding, Falsifiability, Actionability,
  Progression, Information Density based on pre-Track-1 archived trend, but not carried
  over as scores) so the next iteration can pick a high-leverage architectural change.

## Execution

1. Use the existing `ForesightModel` "Directed Software Evolution v2"
   (`en-019d92cd-41e7-7aa0-8436-e0532786bfcf`, 24 signals, seeded 2026-04-15).
2. Create `Projection`, `Configure` with that model and `horizon = "1 year"`, `Start`.
3. Poll status until `Complete` (or `Failed`), max 20 minutes.
4. Extract `observations.json`, `directions.json`, `synthesis.md` from the engine side.
   `synthesis.md` must come verbatim from the orchestrator session's final output
   (provenance-verified by `verify_run.py`).
5. Extract all session transcripts from the isolated SQLite DB into `transcripts/`,
   write `MANIFEST.md` that identifies the orchestrator session as
   `orchestrator ss-<uuid>`.
6. Query `ots_trajectories` natively (Track 3); only fall back to the JSONL converter
   for sessions that didn't emit.
7. Write `diagnosis.md` after reading ALL transcripts — orchestrator, every probe,
   convergence-analyst, model-projector. Root causes often live outside the orchestrator.
8. Append the Run 000 row to `progress.md` with engine score column left blank and
   a note marking this as the era's incumbent.
9. Run `verify_run.py --run-dir meta/runs/000_measurement_baseline/` self-check.
10. Commit all artifacts; plain `git push`. No tag (loop.sh owns tagging, and
    Run 000 doesn't get one anyway).
