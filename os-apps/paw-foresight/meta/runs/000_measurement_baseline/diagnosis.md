# Run 000 Diagnosis — Post-Track-1-reliability measurement baseline

## Summary

**Mode:** measurement-only. No judges, no tournament, no scores. Run 000 establishes the first incumbent for Run 001 to score against.

**Engine raw output:** 28,911 bytes (`engine-output/synthesis.md`), 16 observations, 4 directions, 5 sessions (orchestrator + 3 probes + 1 convergence analyst). Engine completed in ~7m38s end to end. All 5 sessions emitted OTS trajectories natively (Track 3). `verify_run.py` passed locally after a tooling fix to follow `content_file_id` externalization — see Scope in `plan.md`.

Baseline (single-shot, locked): 27.0/48 raw, 54.5/72 Borda. The floor Run 001+ must beat.

## What Happened

1. Projection created with just `foresight_model_id` + `horizon=1 year` — no `max_steps` or `probe_config` overrides, so spec defaults applied.
2. Orchestrator (`gpt-5.4`/`openai_codex`, 22 turns) spawned 3 probes using its skill's default persona set (practitioner, critic, adjacent-domain) — the empty `Projection.probe_config` was ignored because the orchestrator's skill hard-codes the three personas.
3. Each probe ran independently with `temper_web_search`/`temper_web_fetch` enabled:
   - practitioner: 3 searches, 3 fetches, 4 tool calls total, 4 observations.
   - critic: 3 searches, 4 fetches, 9 tool calls, 4 observations.
   - adjacent-domain: 3 searches, 9 fetches, 7 tool calls, 4 observations.
4. After 3/3 `ProbeStepDone` events, `handle_probe_done` spawned a convergence analyst which did 4 Confirm calls, created 4 NEW observations, and proposed its OWN direction (out-of-role).
5. Orchestrator fetched all observations + directions, built a large synthesis via `s = s + '...'` string concatenation inside a single big `execute` tool call, wrote `projection_synthesis_<proj>.md` via `temper.write`, and dispatched `Complete`. No second step ran despite `max_steps=2`/`step_schedule=[90, 365]`.

The synthesis is well-structured (all 9 mandated sections) and densely cross-references observation IDs, but its methodology footer reports "12 recorded observations and 3 active directions" — the engine literally miscounted the entities it produced (actual: 16 obs, 4 directions). That's an internal-inconsistency smell the judges will catch even without seeing this report.

## Where the engine left leverage on the table

### 1. Single-step projection: whole temporal layer skipped

- **Observed:** `Projection.events` shows `Complete` at step 0. No `AdvanceStep` to step 1, no second probe round, no `ProjectionUpdated` carrying a real `projected_state_file_id` (the one emitted was empty). `handle_convergence`'s model-projector spawn was never reached.
- **Mechanism:** the orchestrator skill (`system/skills/orchestrate-projection/SKILL.md`) describes a `for step in range(max_steps)` loop, but the orchestrator session in practice only ran step 0 before calling `Complete`. Rereading the skill: the `Complete` action is fired inside Step G ("Assemble Complete Synthesis") at the end of the final step's logic. Because `max_steps` from `projection.ioa.toml` defaulted to `""` (empty string), the orchestrator's `int(fields.get("max_steps", "2"))` wrapper used the code default `2`, but the outer `for step in range(max_steps)` appears to have executed once and short-circuited to synthesis.
- **Why it matters for scoring:** criteria 6 (Progression) rewards "later phases explicitly revise earlier predictions based on what changed." The synthesis does write a Temporal Progression section with 4 phases, but the 3–6mo, 6–9mo, 9–12mo phases are **authored at one shot** with no actual model-projector evolution between them. The "Revisions to earlier predictions" sub-sections are composed in one go, not emergent. That gap shows up as thin, formulaic revisions rather than genuine temporal development.
- **Root cause in engine:** either (a) `spec.projection.ioa.toml` should default `max_steps="2"` so the skill's `int(...)` call sees 2 instead of "" → 2, but also (b) the skill's `for step in range(max_steps)` control flow ought to pass an assertion that all `step_schedule` offsets were actually projected before `Complete` fires. A WASM invariant (e.g., `step_count == max_steps` before accepting `Complete`) would make this un-gameable.

### 2. Probes are too short to challenge the baseline on Novelty, Grounding, Falsifiability

- **Observed:** practitioner probe did 4 tool calls and emitted 4 observations in ~90s; critic 9 tool calls in ~90s; adjacent-domain 7 tool calls in ~120s. Probe sessions have `max_turns=30` but each ended at 3–6 turns.
- **Cumulative evidence base:** 12 probe observations. Even with confirmation + 4 analyst-created observations, the pool of ideas the synthesis can draw from is narrow.
- **Why it matters:** Novelty (criterion 2) wants external signals the input doesn't contain. Grounding (criterion 10) wants each substantive claim to have a complete evidence→mechanism→conclusion chain. With ~3 web searches per probe and each observation citing 0–2 URLs, the synthesis's external corroboration is thin. The baseline, which single-shot-authored the same 1-year projection with the same web tools, can cite comparable external evidence because it was one long-running session instead of 3 short parallel ones. So the engine's probe parallelism isn't paying off at current depth.
- **Root cause in engine:** probe prompt in `spawn_probes` (and the orchestrator's inline probe-prompt in `SKILL.md`) says "at least 2 web searches" — which is a floor that probes hit exactly. No incentive to go deeper. An entity invariant (Observations with `signal_refs=[]` fade) or a policy (`Observation.Record` requires non-empty `signal_refs`) would enforce grounding. Alternatively: raise the web-search floor to 4, or swap parallel-short probes for fewer-deeper probes.

### 3. Confirmation rate is 37.5% — most probe output never got peer-confirmed

- **Observed:** 6 of 16 observations are `Confirmed`; 10 stayed in `Created`. The convergence analyst ran one pass with 6 `Confirm` dispatches and then created 4 new observations of its own.
- **Mechanism:** `Observation.Confirm` is gated on peer agreement, but nothing in the spec requires the convergence analyst to meet a coverage threshold before dispatching `ConvergenceComplete`. The analyst's skill tells it "Be conservative: only Confirm when genuinely saying the same thing" but gives no floor for how much convergence it must attempt.
- **Why it matters:** Breadth (criterion 4), Information Density (criterion 12), and Challenge (criterion 11) all improve when unconfirmed observations get escalated or faded rather than ignored. Unconfirmed observations that nonetheless got cited in the synthesis weaken judges' confidence that claims are grounded.
- **Root cause in engine:** no entity invariant enforcing "every Observation is either Confirmed, Escalated, or Faded before Projection can Complete." Adding that as a Cedar policy or a `Projection.Complete` precondition would force the analyst to resolve every observation.

### 4. Directions never leave `Proposed`

- **Observed:** all 4 directions are in `Proposed` state. None ran through `SubmitForReview` / `RecordReview` / `RecordConfirmation` / `MarkCrossModelAgreement` / `BeginImplementation`. The spec has a full review lifecycle that's effectively dead code.
- **Mechanism:** the orchestrator skill's post-loop flow goes straight from "read observations + directions" to "write synthesis" without sending any direction through review.
- **Why it matters:** Plausibility (criterion 5), Grounding (10), and Challenge (11) each reward externalized validation. The direction lifecycle exists precisely to provide it; bypassing it wastes the machinery.
- **Root cause in engine:** the orchestrator skill never dispatches `Direction.SubmitForReview`. Either (a) add a review-dispatch step before synthesis, (b) add a separate peer-review agent spawn on `handle_convergence`, or (c) mark directions' `UnderReview` state as the Cedar-policy precondition for being cited in the synthesis.

### 5. Convergence analyst over-reached: 4 new observations + 1 direction proposed

- **Observed:** `aj-019d986e-00ec` (the convergence analyst) authored 4 observations and 1 direction (`en-019d986f-ad29`). The synthesis excluded that 4th direction from its "Active Directions" section — indicating the orchestrator itself treated the analyst's output as second-class.
- **Mechanism:** the analyst's prompt (`handle_probe_done.rs`, `user_message`) tells it to "Confirm" or emit a "CONTRADICTION" observation. The analyst went further and proposed a direction despite nothing in the prompt asking for it.
- **Why it matters:** lets probe-vs-analyst provenance blur. Judges reading the synthesis can't tell which observations came from independent probes and which from the analyst's after-the-fact synthesis. Criterion 11 (Challenge) wants probe-originated challenges, not analyst-manufactured ones.
- **Root cause in engine:** Cedar could block analysts from calling `Direction.Propose` (role-based), or the analyst's prompt should be tightened. Better: separate convergence entity (`ConvergenceReport`) that the analyst writes to, so probe-created entities and analyst-created entities are structurally distinguishable.

### 6. Orchestrator did 8 web searches — out of skill scope

- **Observed:** orchestrator session ran 8 `temper.web_search` calls. The skill doesn't ask for it; probes are the ones supposed to do web research.
- **Mechanism:** `spawn_orchestrator.rs` gives orchestrator `temper_web_search,temper_web_fetch` in `tools_enabled`. The skill doesn't restrict their use, so the LLM used them.
- **Why it matters:** moderate impact — it might improve grounding for the synthesis, but it blurs role separation. An orchestrator that does its own research alongside directing probes makes the probe outputs redundant; the synthesis can then overfit to what the orchestrator found rather than to what probes agreed on.
- **Root cause in engine:** two options — (a) drop web tools from the orchestrator's `tools_enabled` so probes are the sole research layer, or (b) let the orchestrator keep them but add a Cedar policy that forbids the orchestrator from citing a URL not already in an observation's `signal_refs`.

### 7. Synthesis miscounts its own corpus

- **Observed:** the Methodology footer says "12 recorded observations and 3 active directions" — actual counts are 16 and 4. The 4 analyst-created observations and the analyst's direction were both silently excluded.
- **Why it matters:** Information Density (criterion 12) and Grounding (10) penalize internal inconsistency. A judge reading this methodology statement alongside `observations.json` (which they won't see, but the text itself is self-evidently wrong by inspection) registers the miscount as sloppiness.
- **Root cause in engine:** the synthesis-building code in the orchestrator's final `execute` call computes counts from a truncated view of the directions list. Either a post-synthesis assertion (`len(obs) == synthesis.counts.observations`) or — better — generating the methodology stats from the live `temper.list` inside the same tool call removes the opportunity for drift.

## Highest-leverage target for Run 001

Rank order of the 7 issues above by (a) number of criteria affected and (b) whether the fix is architectural (preferred) vs prompt-only:

| # | Issue | Criteria affected | Fix type |
|---|---|---|---|
| 1 | Single-step projection | Progression, Completeness, Actionability, Decision Clarity | architectural (spec invariant + WASM) |
| 2 | Probe shallowness | Novelty, Grounding, Falsifiability, Plausibility, Breadth | architectural (obs invariant / depth floor) |
| 3 | Low confirmation rate | Breadth, Info Density, Challenge | architectural (Projection.Complete precondition) |
| 4 | Dead direction lifecycle | Plausibility, Grounding, Challenge, Actionability | architectural (skill dispatch + Cedar) |
| 5 | Analyst over-reach | Challenge, Grounding | Cedar + spec split |
| 6 | Orchestrator web search | Grounding, Plausibility (indirect) | `tools_enabled` trim or Cedar |
| 7 | Synthesis miscount | Info Density, Grounding | orchestrator skill tweak |

**Priority 1 (Run 001 target):** *Issue 1 — force multi-step progression*. The architecture already has `step_schedule`, `AdvanceStep`, `handle_projection_updated`, and a Model-Projector WASM, but the orchestrator bypasses all of them. Adding a `Projection.Complete` precondition that asserts `current_step == max_steps - 1` (or equivalent WASM check) would force the orchestrator to actually run the full schedule. That single change reopens the entire temporal-evolution layer (model-projector spawns, step-1 probe re-spawn with previous-direction context, genuine revision of earlier predictions). Impacts Progression directly plus Completeness, Decision Clarity, Actionability.

**Priority 2 (fallback):** *Issue 3 — Projection cannot Complete until all observations are Confirmed/Faded/Escalated*. This is a one-line spec invariant (`Observation.status ≠ Created` when `Projection.status` transitions to `Complete`) plus making the convergence analyst's terminate action gated on coverage. Minor code, enforceable via Cedar, forces richer convergence without requiring a temporal-evolution rewrite. Good backup if the priority-1 fix triggers cascading WASM work.

Neither fix touches `program.md` or the rubric, and both generalize to any knowledge-graph domain.

## Artifacts committed this run

- `plan.md`, `changelog.md`
- `engine-output/synthesis.md` (28,911 bytes — the incumbent for Run 001)
- `engine-output/observations.json` (16 entries)
- `engine-output/directions.json` (4 entries)
- `engine-output/projection.json` (entity snapshot)
- `transcripts/MANIFEST.md` + `orchestrator.jsonl`, `probe_practitioner.jsonl`, `probe_critic.jsonl`, `probe_adjacent_domain.jsonl`, `convergence-analyst.jsonl`
- `trajectories/*.ots.json` (native Track 3 emission for all 5 sessions)
- `diagnosis.md` (this file)
