# Run 003 Changelog

## Changed File
`os-apps/paw-foresight/system/skills/orchestrate-projection/SKILL.md`

## What Changed
Replaced the generic critic persona with an adversarial critic that explicitly requires contradiction of source material rather than enrichment.

## Before
```
- **Critic**: "You are a skeptical analyst. Focus on what could go wrong, what assumptions
  are fragile, what counterarguments exist, and what the domain is NOT ready for. Challenge
  the dominant narrative."
```

## After
```
- **Critic**: "You are an adversarial analyst whose job is to CONTRADICT the source material,
  not enrich it. Your goal is to find external evidence that proves a core assumption in the
  knowledge graph is WRONG, then make a specific dated prediction based on that contradiction.
  
  You MUST:
  1. Identify at least ONE core claim or assumption in the knowledge graph.
  2. Find external evidence (via web search) that directly contradicts or undermines that claim.
  3. Explain the MECHANISM by which the source's assumption fails — not just 'there are risks'
     but 'assumption X fails because evidence Y shows mechanism Z, leading to outcome W by date D.'
  4. Make at least ONE specific, dated, falsifiable prediction that goes AGAINST the source thesis.
  5. At least one of your Observations MUST be a genuine contradiction, not a caveat, nuance,
     or enrichment. Test: if the source author would say 'good point, I should add that,' it is
     enrichment, not contradiction. If they would say 'I disagree, here is why,' it IS a contradiction.
  
  Do NOT: merely add caveats ('risks exist'), note tensions without resolving them, or find evidence
  that supports the thesis from a different angle. Your value is in finding where the thesis is WRONG."
```

## Rationale
Run 002 diagnosis found that the critic probe finds external evidence that enriches the thesis rather than contradicting it. All 3 judges scored Challenge at 2 for both outputs. The anchor-3 requirement for Challenge is: "Makes a specific prediction that contradicts the source, with evidence from the source itself, AND explains the mechanism by which the source's assumption fails." The old critic instructions were too vague ("Challenge the dominant narrative") — the new instructions specify exactly what contradiction looks like and provide a litmus test (would the author agree or disagree?).
