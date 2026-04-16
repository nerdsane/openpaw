# Run 005 Plan

## Context

The loop converged at Run 004 (streak 3). Convergence was driven by infrastructure,
not analytical ceiling. Runs 003 and 004 both scored their changes as "incumbent wins"
because the orchestrator never reached the steps where those changes execute.

The Run 004 diagnosis named the blocker explicitly:

> "This is the third consecutive run (002, 003, 004) where the openai_codex provider
> causes session timeouts or failures. … The provider is the systemic bottleneck
> preventing any change from being properly tested."

> "Priority 0: Fix provider reliability. The openai_codex provider is the blocking
> constraint. Options: Switch to a different provider (anthropic_codex, google_codex)
> for the orchestrator session …"

## Target Criteria

Every criterion is indirectly gated by orchestrator completion:

- **Progression (Run 004: 2.3 vs incumbent 3.0, -2 Borda)** — requires orchestrator to
  write the phase-by-phase synthesis with causal links and revisions.
- **Breadth (Run 004: 2.0 vs 3.0, -3 Borda)** — requires cross-probe integration in
  orchestrator's synthesis step.
- **Completeness (Run 004: 2.7 vs 3.0, -1 Borda)** — requires orchestrator's
  assumptions + confidence section.
- **Information Density (Run 004: 2/2 tied)** — the Run 004 dedup change lives in
  Step 4 of SKILL.md. It never executed.

The unified root cause is the orchestrator never running to completion. Fix the
infrastructure, every downstream criterion becomes testable again.

## Planned Change

**ONE change**: decouple orchestrator provider/model from `seed_provider`/`seed_model`.

Currently `wasm/spawn_orchestrator/src/lib.rs` reads `seed_provider` from the
ForesightModel and constructs `provider_codex = f"{seed_provider}_codex"`. The DSE v2
ForesightModel has `seed_provider = "openai"` and `seed_model = "gpt-5.4"`, so the
orchestrator session gets configured with `openai_codex` and `gpt-5.4`.

Architecturally this is wrong: the orchestrator's job (long multi-turn coordination,
many tool calls, large context) is fundamentally different from the seed agent's job
(single-shot knowledge-graph authoring from an essay). Tying them together conflates
two unrelated choices.

**The fix**: hardcode `anthropic_codex` + `claude-sonnet-4-6` for the orchestrator
session regardless of seed configuration. This matches what the first DSE
ForesightModel used — runs against that model completed reliably. Claude Sonnet is
empirically strong at multi-turn tool-use sessions, which is exactly the orchestrator's
profile.

### Specific edit

In `os-apps/paw-foresight/wasm/spawn_orchestrator/src/lib.rs`:

- Remove the code that reads `seed_model` / `seed_provider` from ForesightModel for
  the orchestrator's Session.Configure call.
- Hardcode `"model": "claude-sonnet-4-6"` and `"provider": "anthropic_codex"` in the
  Configure body.
- Keep fetching the ForesightModel for `fm_name` (used in the prompt for context).

Then rebuild WASM, hot-reload the app, run the engine.

## Expected Impact

1. Orchestrator session completes (no more 600s+ timeouts or DeliveryFailed).
2. Convergence step (Step 4) executes → Run 004 observation dedup finally tested.
3. Synthesis step (Step 5) executes → produces orchestrator-integrated output with
   cross-theme analysis, causal phase progression, confidence-graded assumptions.
4. If the hypothesis holds, challenger (Run 005) should recover the Breadth,
   Progression, Completeness ground lost in Runs 003-004 AND gain whatever dedup
   improvement the Run 004 change was meant to produce.

## Constraint Checklist

- [x] Domain-agnostic — change applies to any ForesightModel, any domain.
- [x] No authoring — does not inject or pre-compute output content.
- [x] Architectural — WASM change, not a prompt edit.
- [x] ONE change — the only edit is provider/model hardcoding in spawn_orchestrator.

## Risks

- `claude-sonnet-4-6` may not be available / keyed in this environment. If it fails
  with provider-not-configured, the run will surface this in scores.json methodology
  note and the next run can try `google_codex` or retain gpt but add retry logic.
- If Sonnet also times out, the root cause is not openai-specific — it's orchestrator
  scope (too many turns, too long a prompt). The next run would trim scope.
