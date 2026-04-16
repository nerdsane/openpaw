# Run 001 Plan

## Target Criteria
- **Progression** (Engine: 1/4, Baseline: 3/4): Orchestrator crashed before step 1; no temporal development
- **Breadth** (Engine: 2/4, Baseline: 3/4): No convergence analysis to merge/compress redundant observations
- **Decision Clarity** (Engine: 1/4, Baseline: 2/4): No synthesis = no prioritized recommendations
- **Completeness** (Engine: 1/4, Baseline: 3/4): No synthesis, no confidence levels, no assumptions stated
- **Information Density** (Engine: 1.7/4, Baseline: 2/4): No convergence to merge redundant probe outputs

All five lowest-scoring criteria share the same root cause: the orchestrator WASM fuel exhaustion prevented any work beyond step 0 probe dispatch.

## Root Cause
The `run_tools` integration (monty_repl WASM) in `paw-agent/specs/session.ioa.toml` has `max_fuel = "50000000000"` (50 billion instructions). The orchestrator session consumed this budget in 3-4 LLM tool call rounds — enough to read config and spawn probes, but not enough for: polling probe completion, reading observations, convergence analysis, writing projected state, advancing to step 1, or writing the final synthesis.

Evidence from Run 000 transcripts:
```
Event trail: ProcessToolCalls(4) → Fail("fuel exhausted -- module exceeded instruction budget")
```

## Planned Change
**File:** `os-apps/paw-agent/specs/session.ioa.toml` line 778
**Change:** Increase `max_fuel` from `"50000000000"` (50B) to `"500000000000"` (500B) — a 10x increase.

This is a platform-level change affecting all paw-agent sessions. The 10x multiplier gives the orchestrator headroom for 30+ LLM tool call rounds (estimated need: 20-30 for a full 2-step projection loop with 3 probes each).

No other changes. ONE change per iteration.

## Expected Impact
If the orchestrator completes the full loop (both steps + synthesis), the engine output will have:
- Temporal progression covering 0-365 days (Progression: should improve from 1 → 2+)
- Convergence analysis merging redundant observations (Breadth: 2 → 2+, Info Density: 1.7 → 2+)
- Full synthesis with executive summary, decision points, confidence levels (Decision Clarity: 1 → 2+, Completeness: 1 → 2+)

The engine already wins on Specificity (2.7 vs 2.0). If the synthesis layer works, the engine should become competitive or superior on most criteria.

## Risk
Increasing fuel globally could let runaway sessions consume more resources. Mitigated by the existing `timeout_secs = "900"` (15 min) and `max_turns = "100"` limits on sessions.
