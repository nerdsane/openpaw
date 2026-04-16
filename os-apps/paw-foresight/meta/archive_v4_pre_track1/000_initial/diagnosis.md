# Run 000 Diagnosis

## Summary
**Engine: 21.3/48 | Baseline: 27.0/48 | Delta: -5.7**
**Borda: Engine 48/72, Baseline 60/72 | Winner: Baseline**

The engine failed catastrophically: the orchestrator session hit the WASM fuel limit after only 3 turns, completing step 0 (0-90 day probes) but never performing convergence, advancing to step 1, or writing a synthesis. The engine's output is 12 raw observations and 3 directions from step 0 — no synthesis, no temporal progression, no convergence analysis. Against a polished baseline that covers the full year with phased temporal development, decision frameworks, and confidence levels, the engine loses on 6 of 12 criteria, ties on 5, and wins only on Specificity.

## Critical Failure: Orchestrator WASM Fuel Exhaustion

The orchestrator session (`ss-019d967b-9c98-7163-9fb2-231ff41733e4`) ran on model `gpt-5.4` via `openai_codex` provider with `max_turns: 100`. It completed only 3 LLM call rounds before the WASM integration hit its instruction budget:

```
Event trail: Created → Configure → ProvisionWorkspace → WorkspaceReady → 
  ProcessToolCalls(1) → HandleToolResults(1) → 
  ProcessToolCalls(2) → HandleToolResults(2) → 
  ProcessToolCalls(3) → HandleToolResults(3) → 
  ProcessToolCalls(4) → Fail("fuel exhausted -- module exceeded instruction budget")
```

The orchestrator managed to: (1) read the projection/model config, (2) spawn 3 probe sessions, (3) report ProbesReady. It failed during/after the 4th tool call round — likely while polling for probe completion or attempting convergence. The probes themselves all completed successfully.

**Root cause:** The WASM fuel budget for the `spawn_orchestrator` integration is too low for the orchestrator's workload. The orchestrator must: read config, write state files, spawn probes, poll for probe completion (multiple iterations), read observations, do convergence, write projected state, advance step, and repeat — all within one WASM execution. Each LLM call + tool execution consumes fuel, and 3 rounds wasn't enough.

## Lowest-Scoring Criteria (Engine vs Baseline)

### Progression (Engine: 1/4, Baseline: 3/4) — Borda: 3 vs 6 (-3)
- **What the engine lacks:** The engine only covers 0-90 days (step 0). There is zero temporal development. No phase 2, no phase 3, no causal links between phases. The orchestrator failed before step 1.
- **What the baseline has:** Three clear phases (0-3, 3-6, 6-12 months) with explicit causal links and "what has NOT changed" sections.
- **Root cause:** Orchestrator WASM fuel exhaustion prevented step 1 from executing. Even if fuel were sufficient, the engine only planned 2 steps (0-90 days, 91-365 days) — fewer temporal phases than the baseline's 3.
- **Fix:** Increase WASM fuel budget. Consider increasing step count to 3 (matching baseline's temporal granularity).

### Breadth (Engine: 2/4, Baseline: 3/4) — Borda: 3 vs 6 (-3)
- **What the engine lacks:** While 12 observations cover multiple themes, they overlap heavily. All 3 probes converge on the same core thesis (governance > generation, harness-first, control-plane focus). The adjacent-domain probe adds biology/economics/industrial-control analogies, but they all reach the same conclusion.
- **What the baseline has:** Covers governance, verification, platform economics, memory/telemetry, autonomy scaling, typed environments, human approvals, formal methods — more distinct analytical dimensions with explicit cross-theme connections.
- **Root cause:** No convergence analysis was performed (orchestrator crashed), so redundancy was never identified or compressed. Also, 3 probes generating 4 observations each tends to produce 12 loosely-connected points rather than a coherent multi-dimensional analysis.
- **Fix:** The convergence step (which never ran) is designed to address this. Priority is getting the orchestrator to complete. Secondarily, probe prompts could emphasize distinct analytical dimensions.

### Decision Clarity (Engine: 1/4, Baseline: 2/4) — Borda: 3 vs 6 (-3)
- **What the engine lacks:** No synthesis means no prioritized recommendations. The raw observations contain implicit decision points but a VP would need 30+ minutes to extract them. There's no "here's the #1 thing to do."
- **What the baseline has:** Structured decision points with timing triggers, options, and tradeoffs.
- **Root cause:** The synthesis step (which produces the executive summary and decision framework) never ran.
- **Fix:** Get the orchestrator to complete. The synthesis template in SKILL.md already calls for "Decision Points" but it never executed.

### Completeness (Engine: 1/4, Baseline: 3/4) — Borda: 3 vs 6 (-3)
- **What the engine lacks:** Observations and directions only — no synthesis, no temporal development, no confidence levels, no assumptions stated. Missing the full pipeline.
- **What the baseline has:** Full pipeline: executive summary, key findings, temporal progression, active directions with reasoning, decision points, confidence levels, methodology.
- **Root cause:** Orchestrator crash. The SKILL.md template for the final synthesis covers all these sections, but it never executed.
- **Fix:** Same as Progression — get the orchestrator to complete.

### Actionability (Engine: 2/4, Baseline: 2/4) — Borda: 4 vs 5 (-1)
- **What the engine has:** Quantitative thresholds embedded in observations (70% PR pass rate, 5% rollback frequency, 2-week onboarding limit) — specific enough to act on.
- **What the baseline has:** Structured decision points but with more generic tradeoffs.
- **Root cause:** The engine's probes actually produce more specific, actionable thresholds than the baseline — but without synthesis, they're scattered across 12 observations rather than organized into a decision framework.
- **Fix:** If the synthesis step runs, the actionable thresholds from probes should produce a strong Decision Points section.

### Information Density (Engine: 1.7/4, Baseline: 2/4) — Borda: 4 vs 5 (-1)
- **What the engine lacks:** Significant redundancy across observations. Multiple observations from different probes say essentially the same thing (governance > generation, control-plane first, harness-first). Without convergence, these redundancies are never merged.
- **What the baseline has:** More compressed — each section adds distinct information.
- **Root cause:** No convergence analysis. The 3-probe architecture is designed to produce redundant observations that convergence then merges. Without convergence, redundancy is the output.
- **Fix:** Convergence step (which confirms, merges, or contradicts cross-probe observations).

## Where the Engine Wins

### Specificity (Engine: 2.7/4, Baseline: 2/4) — Borda: 5.5 vs 3.5 (+2)
- The engine's observations include precise quantitative thresholds: "fewer than 70% of agent-generated pull requests pass," "rollback frequency above 5%," "time-to-onboard > 2 weeks," "30% or more of low-risk infrastructure PRs." The baseline names actors but with less quantitative precision.
- This is the multi-agent advantage showing through: independent probes produce more specific, testable claims than a single-shot synthesis.

## Why the Baseline Wins

The baseline wins because it's complete. A single-shot prompt produces a fully structured output: executive summary, temporal phases, directions, decisions, confidence levels. The engine's multi-agent architecture produces more specific observations (winning on Specificity) but fails to synthesize them. The orchestrator crash means the engine's output is effectively step 0 observations only — no synthesis, no temporal development, no convergence.

The engine's architecture (spawn probes → wait → converge → advance → synthesize) is fundamentally sound for producing better foresight, but the WASM execution environment is too constrained. The probes work; the orchestrator doesn't survive long enough to do anything with their output.

## Recommended Changes for Next Iteration

**Priority 1:** Fix the WASM fuel budget for `spawn_orchestrator`. The orchestrator needs enough fuel to complete the full loop: read config, spawn probes, poll for completion, read observations, do convergence, write projected state, advance step (x2), and write final synthesis. The current budget allows only ~3 LLM turns, but the orchestrator needs 10-20+ turns minimum.

**Priority 2:** If fuel budget can't be increased (platform constraint), restructure the orchestrator to use fewer tool calls per step — e.g., batch operations, reduce polling iterations, or split the orchestration across multiple WASM invocations with entity state transitions between them.

Per meta-loop rules: make ONE targeted change per iteration.
