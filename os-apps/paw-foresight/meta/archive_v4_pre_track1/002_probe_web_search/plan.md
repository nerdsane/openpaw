# Run 002 Plan

## Target Criteria

All 8 criteria currently tied at 2.0 (competent median) represent the highest-leverage targets. The diagnosis from Run 001 identified:

- **Novelty (2.0):** Probes produce extensions of the input knowledge graph without introducing truly external evidence or frameworks. Root cause: probes only have access to the knowledge graph via `temper.read()` — no external data sources.
- **Challenge (2.0):** Neither output overturns a source assumption using external evidence. Root cause: the critic probe reframes tensions within the input but has no mechanism to find contradicting external signals.
- **Grounding (2.0):** Reasoning chains from evidence → mechanism → conclusion have gaps. Root cause: observations cite KG signals but lack external corroboration.
- **Plausibility (2.0):** Claims reference mechanisms but lack named external signals. Root cause: probes cannot discover real-world signals (news, papers, announcements) that would ground claims.

## Planned Change

**File:** `os-apps/paw-foresight/system/skills/orchestrate-projection/SKILL.md`

**What changes:**
1. Add `temper_web_search,temper_web_fetch` to the probe sessions' `tools_enabled` field in the Configure action.
2. Update all three probe persona instructions (practitioner, critic, adjacent-domain) to explicitly mandate web search for external evidence before creating observations.
3. Add a new instruction step (between "read the current state" and "create observations") requiring probes to run at least 2 web searches for recent signals, news, or research not in the knowledge graph.

**What does NOT change:**
- Synthesis template (no prompt edits to the output structure)
- Entity specs, WASM modules, Cedar policies
- Number of probes, steps, or convergence logic

## Expected Impact

| Criterion | Current | Expected | Mechanism |
|-----------|---------|----------|-----------|
| Novelty | 2.0 | 2.5-3.0 | External signals from web search enable insights not in the input |
| Challenge | 2.0 | 2.5-3.0 | Critic probe can find external evidence that contradicts source assumptions |
| Grounding | 2.0 | 2.0-2.5 | External corroboration strengthens reasoning chains |
| Plausibility | 2.0 | 2.0-2.5 | Named external signals increase grounding of claims |

This is ONE structural change (enable web search for probes) that targets 4 criteria simultaneously. It is architectural (adding tool access) not a prompt edit (though the probe instructions must explain how to use the new tools).
