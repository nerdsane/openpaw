# Run 000 Diagnosis

## Summary

**Engine: 34/48 | Baseline: 43/48 | Delta: -9**

The baseline single-shot output comprehensively outperforms the multi-agent engine. The engine tied on 4 criteria (Novelty, Internal Consistency, Human Readability, Parsimony) and lost the remaining 8.

## Lowest-Scoring Criterion: Transparency (Engine: 1/4)

The engine synthesis references "3 probes" and "27 total observations" in its methodology note, but no individual claim traces back to a specific observation, signal, or source. The baseline at least references named graph elements ("the graph's homeostasis-versus-exploration distinction") even without formal citations.

**Root cause in engine:** The orchestration skill's synthesis template says "For each active direction: title + FULL reasoning text from the direction entity" but doesn't instruct the synthesizer to cite observation IDs or signal references. The directions themselves contain reasoning but no explicit provenance chains. The probes create observations with `signal_refs` fields, but these are never surfaced in the synthesis.

**Fix:** The synthesis section of the skill should instruct the orchestrator to include signal_refs from observations in the final narrative, or at minimum cite observation IDs alongside claims.

## Second-Lowest: Actionability (Engine: 2/4)

The engine's decision points are bullet-point recommendations ("Invest first in...") without triggers, options, or tradeoffs. The baseline provides 4 full decision frameworks with timing triggers, 3 options per decision, and explicit tradeoffs.

**Root cause in engine:** The synthesis template's Decision Points section says only "[Actionable recommendations with timing triggers]" — it doesn't specify the options/tradeoffs structure. The baseline was explicitly prompted to produce "Timing triggers, Options available at each decision point, Tradeoffs between options."

**Fix:** Update the synthesis template to specify the full decision framework structure: timing triggers, multiple options, and tradeoffs per decision point.

## Third-Lowest: Progression (Engine: 3/4, Baseline: 4/4)

The engine uses 2 time steps (90 days, 365 days) and summarizes temporal development in the executive summary. The baseline has explicit 3-phase temporal progression with detailed sub-sections showing what changes, signals, what hasn't changed, and causal links between phases.

**Root cause in engine:** The engine's step_schedule was [90, 365] — only 2 steps. The synthesis template doesn't instruct explicit phase-by-phase breakdown; it relies on the multi-step structure to provide temporal depth. But 2 steps is too coarse, and the synthesis just summarizes rather than developing each phase.

**Fix:** Consider more steps (e.g., 4 quarterly). More importantly, the synthesis template should mandate an explicit temporal progression section with phase-by-phase breakdown.

## Why the Baseline Wins

The baseline benefits from three structural advantages:

1. **Full context window**: The single-shot model has its entire context window for one coherent response. The engine fragments context across probes, then the orchestrator must re-synthesize from summaries.

2. **Prompt specificity**: The baseline prompt explicitly requested temporal progression, confidence levels, decision frameworks with options/tradeoffs, and challenged assumptions. The engine skill doesn't specify these output structures.

3. **No coordination overhead**: No probe synchronization, no convergence analysis, no state serialization. Every token goes to substance rather than process.

## What the Engine Does Better

The engine has structural advantages that don't yet translate to output quality:

- **Internal tension management**: The multi-probe structure naturally creates practitioner vs critic vs adjacent-domain tensions, leading to the 4/4 Internal Consistency score. This was genuinely earned through the process.
- **Condensed insight density**: At 8K chars vs 24K, the engine's insights per character are higher. It achieves the same Novelty and Parsimony scores in 1/3 the length.
- **Methodology transparency**: The engine honestly reports its process (probes, observations, directions), even if it doesn't yet leverage that for claim-level transparency.

## Infrastructure Issues

Two issues blocked proper evaluation:

1. **Judge v1 failure**: Paw-agent judge sessions never called `temper_read` — the model went straight to hallucinated identical scores for both outputs. Root cause: `temper.read()` in the prompt was interpreted as pseudo-code, not as a directive to use the `temper_read` tool.

2. **Judge v2 failure**: Corrected prompt caused models to use `temper.read()` via the REPL `execute` tool, but tool calls stuck in processing pipeline for 10+ minutes without completing. Root cause: likely server-side session processing backlog or REPL tool execution failure.

**Impact**: Run 000 was scored by the meta-agent (Claude Code) instead of 3 independent blind judges. This is a methodology limitation that must be fixed before Run 001.

## Recommended Changes for Next Iteration

**Priority 1 (biggest score delta):** Update synthesis template to mandate:
- Temporal progression section with phase-by-phase breakdown
- Decision points with triggers, options, and tradeoffs  
- Confidence levels for each prediction
- Signal/observation references in claims

**Priority 2:** Fix automated judge infrastructure (file reading via paw-agent sessions)

**Priority 3:** Consider more time steps (4 quarterly instead of 2)

Per meta-loop rules: make ONE targeted change per iteration. Priority 1 (synthesis template update) addresses the 4 criteria where the engine scored lowest relative to baseline (Actionability -2, Transparency -1, Progression -1, Completeness -1).
