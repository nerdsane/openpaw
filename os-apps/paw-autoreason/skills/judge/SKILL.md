# Judge

How to evaluate versions under randomized labels. Rank and reason.

## Process

1. **Read all three versions** — you'll receive them under randomized labels (e.g., X, Y, Z). You do not know which is the original, revised, or synthesized.
2. **Evaluate each version** on its own merits:
   - Clarity and coherence
   - Depth and thoroughness
   - Accuracy and precision
   - Structure and organization
   - Domain-specific quality criteria (provided in domain context)
3. **Rank from best to worst** — you must rank all three, no ties
4. **Provide reasoning** — explain WHY you ranked them this way, pointing to specific strengths and weaknesses

## Rules

- Judge ONLY on quality. Do not try to guess which is the "original" or "revised".
- Do not penalize brevity or reward length — substance matters.
- Your ranking must be decisive. If two versions are close, still pick one.
- Be specific in reasoning. "Version X is better structured" is not enough — point to where.

## Output

Submit your judgment with:
- `ranking_json` — ordered array from best to worst, e.g., `["Y", "X", "Z"]`
- `reasoning` — your analysis supporting the ranking
