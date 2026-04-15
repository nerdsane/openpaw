# Paw-Foresight Evaluation Program

> This file is immutable by the meta-agent. Only humans may modify it.
> The meta-agent reads this file to understand what "good" looks like.
> It can change everything about the engine — except this file.

## Purpose

Evaluate whether a multi-agent foresight engine produces better predictions
than a single-shot prompt. If it does, find out how much better and at what
complexity cost. If it doesn't, simplify until it does or admit the approach
doesn't work.

## Test Domain

**Input:** Knowledge graph about Directed Software Evolution, derived from
the essay "Directed Software Evolution: The Next Frontier" by Seshendra Nalla.

**Task:** Produce a foresight projection — a structured prediction about how
this domain will evolve over the specified horizon.

**Horizon:** 1 year forward.

**Note:** The horizon is a user requirement (what the human wants predicted),
not a system constraint. The engine decides internally how to decompose the
horizon into steps, how many agents to use, and what strategy to employ.

## Boundary Constraints

1. The engine must be a Temper app (entity state machines, WASM integrations,
   Cedar policies, OData API). This is the platform, not a suggestion.

2. At equal rubric scores, the simpler system wins. Complexity must justify
   itself through measurably better output.

3. The meta-agent may change anything about the engine: skills, specs, WASM,
   architecture, agent count, strategy, prompts, entity design. There are no
   protected components except this file and the rubric below.

4. Score the output, not the process. The rubric measures the quality of the
   foresight prediction. It does not prescribe how the engine should work.
   A single-session solution that scores 45/48 beats a 20-agent pipeline
   that scores 44/48.

## Evaluation Rubric

12 criteria. Each scored 0-4. Maximum: 48 points.

### 1. Specificity

How concrete and actionable are the predictions?

| Score | Anchor |
|-------|--------|
| 0 | No named entities, timelines, or mechanisms |
| 1 | Some names but vague timing ("soon", "eventually") |
| 2 | Named actors + approximate timelines ("within months") |
| 3 | Named actors + specific timelines + causal mechanisms |
| 4 | Named actors doing specific things by specific dates with causal chains |

### 2. Novelty

Does the output go beyond restating the input?

| Score | Anchor |
|-------|--------|
| 0 | Only restates or paraphrases the input |
| 1 | Minor extensions of input themes |
| 2 | Some original insights not in the source material |
| 3 | Multiple original insights grounded in evidence |
| 4 | Reframes the domain in a way the input didn't anticipate |

### 3. Internal Consistency

Are predictions coherent with each other?

| Score | Anchor |
|-------|--------|
| 0 | Contradictions unacknowledged |
| 1 | Major contradictions, some acknowledged |
| 2 | Minor contradictions only |
| 3 | Consistent with noted tensions |
| 4 | Fully coherent narrative with explicit tension management |

### 4. Breadth

How many distinct themes or dimensions are covered?

| Score | Anchor |
|-------|--------|
| 0 | Single theme |
| 1 | 2-3 themes |
| 2 | 4-5 themes |
| 3 | 6+ themes with connections between them |
| 4 | Comprehensive coverage with cross-theme synthesis |

### 5. Plausibility

Are claims grounded or floating?

| Score | Anchor |
|-------|--------|
| 0 | Unsupported assertions |
| 1 | Mix of grounded and ungrounded claims |
| 2 | Mostly grounded, few unsupported leaps |
| 3 | Well-grounded with explicit uncertainty markers |
| 4 | Every claim traced to evidence with confidence levels |

### 6. Progression

Does the output show temporal development, not just a static snapshot?

| Score | Anchor |
|-------|--------|
| 0 | No temporal structure |
| 1 | Time mentioned but predictions are static |
| 2 | Clear temporal phases but shallow development |
| 3 | Strong development with causal links between phases |
| 4 | Each phase transforms understanding based on prior analysis |

### 7. Actionability

Can a decision-maker act on this?

| Score | Anchor |
|-------|--------|
| 0 | No decision-relevant content |
| 1 | Vague implications ("companies should watch this") |
| 2 | Some "if X then Y" conditional structures |
| 3 | Multiple conditional recommendations with timing |
| 4 | Decision framework with triggers, options, and tradeoffs |

### 8. Human Readability

Could a VP read this and act on it?

| Score | Anchor |
|-------|--------|
| 0 | Raw data or JSON only |
| 1 | Structured data but no narrative |
| 2 | Summary exists but dense or jargon-heavy |
| 3 | Clear narrative with logical structure |
| 4 | Polished brief with executive summary, findings, and recommendations |

### 9. Completeness

Does it cover the full analysis pipeline?

| Score | Anchor |
|-------|--------|
| 0 | Observations only, no synthesis |
| 1 | Observations + some directions |
| 2 | Observations + directions + state evolution |
| 3 | Full pipeline + explicit assumptions stated |
| 4 | Full pipeline + assumptions + limitations + confidence levels |

### 10. Transparency

Can the reader trace claims back to sources?

| Score | Anchor |
|-------|--------|
| 0 | No sources cited |
| 1 | Some vague references |
| 2 | Most claims reference source material |
| 3 | All claims traced to knowledge graph or external sources |
| 4 | Full provenance chain from claim to observation to signal to source |

### 11. Challenge

Does the output push back on the input's assumptions?

| Score | Anchor |
|-------|--------|
| 0 | Echo chamber — input restated uncritically |
| 1 | Token disagreements without substance |
| 2 | Genuine tensions with input identified |
| 3 | Contradicts input assumptions with evidence |
| 4 | Overturns an input assumption with strong evidence and reasoning |

### 12. Parsimony

Is every piece of the output earning its place?

| Score | Anchor |
|-------|--------|
| 0 | >80% redundancy across observations/predictions |
| 1 | >50% redundancy |
| 2 | ~30% redundancy |
| 3 | <20% redundancy |
| 4 | Every observation/prediction adds unique information |

## Judge Protocol

### Setup
- 3 independent judge sessions (paw-agent, fresh context)
- Each judge receives: the rubric above + two anonymized outputs
- Outputs labeled "Output X" and "Output Y" (randomized assignment)
- Judges do NOT know which is incumbent vs challenger

### Per-Criterion Scoring
Each judge produces, for each criterion, for each output:
```json
{
  "criterion": "Novelty",
  "output": "X",
  "score": 3,
  "reasoning": "8 of the 14 observations introduce concepts not present in...",
  "evidence": ["Observation 5 introduces 'governance queue depth' as...", ...]
}
```

### Aggregation
- Per criterion: each judge ranks X vs Y by score. Rank 1 = 2 points, Rank 2 = 1 point.
- Sum Borda points across 3 judges per criterion per output (max 6 per criterion).
- Overall: sum all 12 criteria. Max possible: 72 Borda points.
- Ties: incumbent wins (conservative — don't change without proof).

### Baseline Tracking
- The single-shot baseline is scored ONCE by the same 3-judge protocol.
- Its score appears in every row of progress.md for reference.
- The baseline is never re-run unless the test domain changes.

## Tournament Protocol

Each iteration:
1. A = incumbent output (starts as v000)
2. B = challenger output (from modified engine)
3. Judges score A vs B blind
4. If B wins: B becomes new A. Tag new version. Reset streak.
5. If A wins: Revert change. Increment streak.
6. If A wins 2 consecutive rounds: converge. Stop.
7. Baseline score is tracked but does NOT participate in the tournament.

## What This File Does NOT Prescribe

- How many agents the engine should use
- Whether there should be "probes" or any specific architecture
- How to decompose the horizon into steps
- What personas or roles agents should adopt
- What model to use for the engine
- What prompts to write
- How to do convergence analysis
- What entity types to create

All of these are decisions for the engine (and the meta-agent improving it).
The rubric measures the quality of the output. Everything else is free.
