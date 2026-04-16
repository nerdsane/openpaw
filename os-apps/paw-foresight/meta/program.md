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

5. **Domain-agnostic.** The engine must work for ANY knowledge graph, not
   just the DSE essay. Hard-coding domain-specific themes, categories,
   terminology, or logic into WASM, skills, or prompts is forbidden. The
   test domain is DSE, but the engine must generalize. If a change would
   not make sense for a knowledge graph about supply chain logistics or
   climate modeling, it is too domain-specific.

6. **No authoring.** The meta-agent improves the engine — it does NOT author
   the engine's output. If a component fails at runtime (session error, 0
   turns, provider issues), score what the engine actually produced. Do not
   pre-compute content, inject analysis, or fill in gaps that the engine
   failed to generate. The engine must stand on its own.

7. **Prefer architectural changes.** When diagnosing deficits, try structural
   changes first (new entity types, new WASM integrations, new session
   patterns, new data flows) before resorting to prompt edits. Prompt edits
   are acceptable but the meta-agent must understand that prose instructions
   are advisory — LLMs frequently ignore them. WASM-enforced constraints
   and structural architecture changes have proven more reliable across
   Runs 001-010.

## Evaluation Rubric

12 criteria. Each scored 0-4. Maximum: 48 points.

Scoring calibration: a 2 is "competent." A 3 is "genuinely impressive — most
outputs will not reach this." A 4 is "exceptional — requires something a
well-prompted single model would rarely produce." Judges should expect the
median criterion score for a good output to be 2, not 3.

**3+ cap rule:** No more than 3 criteria may score 3 or higher for any single
output. If more than 3 criteria initially qualify for a 3+, the judge must
re-examine and demote the weakest to a 2, keeping only the 3 strongest at 3+.
This enforces the principle that 3 is rare. Document which criteria were
demoted and why in the reasoning field.

### 1. Specificity

Does it name real actors, dates, and quantitative thresholds?

| Score | Anchor |
|-------|--------|
| 0 | No named entities, timelines, or mechanisms |
| 1 | Generic categories ("companies", "teams") with vague timing ("soon") |
| 2 | Named actors OR approximate timelines, but not both together |
| 3 | Named actors + specific timelines + causal mechanisms connecting them |
| 4 | Named actors doing specific things by specific dates with quantitative thresholds (e.g. "when >30% of CI pipelines include agent-generated patches") |

### 2. Novelty

Does the output produce insights that are not in the input AND not obvious from general domain knowledge?

| Score | Anchor |
|-------|--------|
| 0 | Only restates or paraphrases the input |
| 1 | Minor extensions of input themes using common knowledge |
| 2 | 1-2 original insights not in the source material |
| 3 | Multiple original insights grounded in evidence FROM OUTSIDE the input (external signals, cross-domain analogies, or data the source doesn't contain), at least one connecting signals the input didn't connect |
| 4 | Introduces a framework, concept, or connection that reframes the domain AND is not an obvious extension of the source material |

### 3. Falsifiability

Are predictions stated so they can be proven wrong?

| Score | Anchor |
|-------|--------|
| 0 | All predictions are hedged, vague, or unfalsifiable ("X may happen") |
| 1 | A few predictions could in principle be checked, but most are too vague |
| 2 | Several predictions have clear enough conditions to be evaluated after the fact |
| 3 | Most predictions name what would confirm or disconfirm them |
| 4 | Predictions include explicit falsification criteria: "If X has not happened by [date], this prediction is wrong because [reason]" |

### 4. Breadth

How many distinct analytical dimensions are covered, and are they connected?

| Score | Anchor |
|-------|--------|
| 0 | Single theme or dimension |
| 1 | 2-3 themes, treated independently |
| 2 | 4-5 themes with some connections noted |
| 3 | 6+ themes with explicit cross-theme interactions where the interaction produces a non-obvious conclusion (e.g. "governance constraints reshape vendor economics" leading to a specific predicted outcome neither theme implies alone) |
| 4 | Comprehensive multi-dimensional coverage where themes form a coherent system — removing one theme would weaken the others |

### 5. Plausibility

Are claims grounded in named evidence, or floating assertions?

| Score | Anchor |
|-------|--------|
| 0 | Unsupported assertions presented as fact |
| 1 | Mix of grounded and floating claims; uncertainty rarely acknowledged |
| 2 | Most claims reference mechanisms or evidence; some explicit uncertainty |
| 3 | Claims grounded in named signals or evidence with explicit confidence levels and stated assumptions |
| 4 | Every substantive claim traced to evidence, with confidence levels, stated assumptions, AND what would change the confidence |

### 6. Progression

Does the output show genuine temporal development where later predictions build on earlier ones?

| Score | Anchor |
|-------|--------|
| 0 | No temporal structure |
| 1 | Time periods mentioned but predictions are independent snapshots |
| 2 | Clear temporal phases; later phases reference earlier ones superficially |
| 3 | Each phase causally depends on prior phases AND later phases explicitly revise, qualify, or strengthen earlier predictions based on what changed (not just "more of the same") |
| 4 | Temporal development where later phases explicitly confirm, falsify, or revise earlier predictions — the analysis evolves, not just the timeline |

### 7. Actionability

Could a specific decision-maker take a specific action based on this?

| Score | Anchor |
|-------|--------|
| 0 | No decision-relevant content |
| 1 | Vague implications ("companies should watch this space") |
| 2 | Conditional recommendations without timing or tradeoffs |
| 3 | Decision points with timing triggers AND options, but tradeoffs are generic |
| 4 | Decision framework naming who decides, when (with observable triggers), what options exist, and what each option costs or risks in concrete terms |

### 8. Decision Clarity

Could a VP read this in 15 minutes and know the #1 thing to do, by when, and what it costs?

| Score | Anchor |
|-------|--------|
| 0 | Raw data, JSON, or unstructured notes |
| 1 | Structured but requires domain expertise to extract decisions |
| 2 | Clear narrative with findings, but the "so what" is implicit |
| 3 | Prioritized findings with explicit top recommendation and timing |
| 4 | Opens with the single most important decision, names the deadline, quantifies the tradeoff, and structures everything else as supporting evidence |

### 9. Completeness

Does it cover the full foresight pipeline from evidence to recommendation?

| Score | Anchor |
|-------|--------|
| 0 | Observations only, no synthesis or direction |
| 1 | Observations + some theses, but no temporal development or assumptions |
| 2 | Observations + theses + temporal development; assumptions implicit |
| 3 | Full pipeline with explicit assumptions, limitations, and confidence levels |
| 4 | Full pipeline + explicit assumptions + limitations + confidence + what-would-change-my-mind for each major claim |

### 10. Grounding

Does the evidence actually support the claim? (Not "is it cited?" but "does the reasoning chain hold?")

| Score | Anchor |
|-------|--------|
| 0 | Claims are asserted without any supporting evidence or reasoning |
| 1 | Evidence is present but does not logically support the claims made |
| 2 | Most claims have relevant evidence, but the logical chain from evidence to conclusion has gaps |
| 3 | Claims are supported by evidence with explicit reasoning chains — the reader can follow why the evidence leads to the conclusion |
| 4 | Every substantive claim has a complete reasoning chain: evidence → mechanism → conclusion, with stated assumptions and what would break the chain |

### 11. Challenge

Does it make predictions that go against the source material's thesis?

| Score | Anchor |
|-------|--------|
| 0 | Echo chamber — input thesis restated and reinforced |
| 1 | Token caveats ("risks exist") without substantive disagreement |
| 2 | Identifies genuine tensions or fragile assumptions in the source |
| 3 | Makes a specific prediction that contradicts the source, with evidence from the source itself, AND explains the mechanism by which the source's assumption fails |
| 4 | Overturns a source assumption using evidence from OUTSIDE the source (external signals, analogies, or domain knowledge the source lacks) |

### 12. Information Density

Does every claim add unique analytical value? (Penalizes redundancy, rewards compression.)

| Score | Anchor |
|-------|--------|
| 0 | >60% of claims restate or paraphrase other claims in the same output |
| 1 | Significant redundancy — many claims could be merged without information loss |
| 2 | Some redundancy exists but most claims contribute distinct information |
| 3 | Every claim adds unique information; removing any claim would reduce the output's analytical value |
| 4 | Maximum compression — no two claims overlap, every sentence earns its place, and the output says more with fewer words than a typical analysis |

## Judge Protocol

### Setup
- 3 independent Claude Code subagent judges (`claude -p`)
- Each judge receives: the full rubric + BOTH outputs side-by-side
- Outputs labeled "Output X" and "Output Y" (randomized assignment per judge)
- Judges do NOT know which is incumbent vs challenger
- Side-by-side presentation is mandatory — a judge that only sees one output
  cannot compare and its scores are invalid

### Why Claude Code Subagents (not paw-agent sessions)
Paw-agent sessions route user_message through WASM (ProvisionWorkspace),
which has a 32KB field limit. Two foresight outputs + rubric exceed 32KB,
causing truncation or session failure. Claude Code subagents (`claude -p`)
have no such limit and can receive the full rubric + both outputs.

### Per-Criterion Scoring
Each judge produces, for each criterion, scores for BOTH outputs:
```json
{
  "criterion": "Novelty",
  "output_x_score": 3,
  "output_y_score": 2,
  "reasoning": "Output X introduces 3 concepts not in the input (governance queue depth, ...) while Output Y extends the input but adds no external evidence...",
  "evidence_x": ["Section 'Emerging Dynamics' introduces...", ...],
  "evidence_y": ["Section 3 restates the input's claim about...", ...]
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
