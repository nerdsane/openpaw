# Run 003 Plan

## Target Criteria
- **Challenge**: engine scored 2/4 (tied with baseline), no judge-level variation. Root cause: critic probe finds external evidence that enriches the source thesis rather than contradicting it. Anchor-3 requires "a specific prediction that contradicts the source, with evidence from the source itself, AND explains the mechanism by which the source's assumption fails."
- **Spillover targets**: Grounding (contradiction requires explicit reasoning chain), Novelty (contradictory insights are inherently novel)

## Diagnosis Summary (from Run 002)
The critic probe successfully used web search and found external evidence (OWASP, NIST, MCP auth spec), but used it to *enrich* the source thesis rather than *challenge* it. All 3 judges noted this: "identifies tensions rather than makes specific contradictory predictions" (J1), "adds a caveat... closer to anchor 2 than anchor 3" (J3). The probe is cooperative when it needs to be adversarial.

## Planned Change
**File:** `os-apps/paw-foresight/system/skills/orchestrate-projection/SKILL.md`
**Section:** Critic persona instructions in Probe Prompt Template

**What changes:** Replace the current generic critic persona with an adversarial critic that:
1. Must identify at least ONE core assumption in the knowledge graph that external evidence contradicts
2. Must make a specific, dated prediction based on the contradiction (not just note a tension)
3. Must explain the mechanism by which the source's assumption fails
4. Must create at least one Observation explicitly tagged as a contradiction (not enrichment)

This is a prompt-level change (not architectural), but it's the most targeted intervention for the Challenge criterion. The current critic instructions say "Challenge the dominant narrative" — too vague. The new instructions specify exactly what "challenge" means in terms of the rubric anchors.

## Expected Impact
- **Challenge**: 2 → 3 (if critic generates a genuine contradictory prediction with evidence)
- **Novelty**: potential +0.5 (contradictory insights are novel by definition)
- **Grounding**: potential +0.5 (contradiction requires explicit evidence→mechanism→conclusion chain)
- **Actionability**: monitor for regression (Run 002 showed longer output dilutes decision points)
- **Net Borda delta**: +3 to +6 if Challenge moves from tied to engine-winning
