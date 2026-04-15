# Paw-Foresight Meta-Improvement System

## What This Is

A self-improving foresight engine. The engine makes predictions about a domain.
A meta-agent evaluates those predictions against a rubric, diagnoses weaknesses,
modifies the engine, and re-runs. Independent judges score each version blind.
The process repeats until the engine converges (stops improving) or the meta-agent
runs out of ideas.

Everything is documented. Every version is tagged. Every score is recorded.
Every change is reasoned about. The goal: produce a trail that can be read as
a paper showing how iterative meta-evaluation improves a multi-agent system.

## Architecture

```
                    HUMAN
                      |
                      | controls (immutable)
                      v
                 program.md
                 (rubric + domain + constraints)
                      |
                      | reads
                      v
              +---------------+
              |  META-AGENT   |  Claude Code (Opus)
              |               |  Reads rubric, reads traces,
              |               |  diagnoses, modifies engine,
              |               |  orchestrates everything
              +-------+-------+
                      |
          +-----------+-----------+
          |                       |
          v                       v
   +-----------+           +-----------+
   |  ENGINE   |           | BASELINE  |
   |  (v{N})   |           | (fixed)   |
   |           |           |           |
   | OpenPaw   |           | Single    |
   | agents    |           | paw-agent |
   | running   |           | session,  |
   | foresight |           | one shot  |
   +-----------+           +-----------+
          |                       |
          v                       v
     Output A/B              Output ref
     (artifacts)             (artifacts)
          |                       |
          +-----------+-----------+
                      |
                      v
              +---------------+
              |    JUDGES     |  3 independent paw-agent sessions
              |               |  Blind scoring against rubric
              |               |  Borda count aggregation
              |               |  Conservative tiebreak (A wins ties)
              +-------+-------+
                      |
                      v
                 Scores + Winner
                      |
                      v
              meta/progress.md
              meta/runs/{NNN}/
```

## Roles

### Human
- Authors and owns `program.md` (rubric, domain, constraints)
- Can audit any score, override any decision
- Reviews progress.md to track improvement arc
- Does NOT participate in the scoring loop

### Meta-Agent (Claude Code / Opus)
- Reads program.md for rubric and constraints
- Runs the foresight engine (triggers OpenPaw pipeline)
- Runs the baseline (single-shot session)
- Dispatches judge sessions
- Reads raw session transcripts for diagnosis (not summaries — full traces)
- Identifies lowest-scoring criteria
- Makes ONE targeted change per iteration to the engine
- Can change: skill text, entity specs, WASM modules, agent count, architecture
- Cannot change: program.md, the rubric, the judge protocol
- Documents everything in meta/runs/{NNN}/

### Engine (OpenPaw Agents)
- Runs autonomously on Temper
- The meta-agent creates a Projection entity, hits Start
- spawn_orchestrator WASM creates an agent session
- The orchestrator session runs the foresight loop (whatever that looks like for v{N})
- Produces: observations, directions, projected state, synthesis
- The engine's architecture can change between versions

### Baseline (Fixed Reference)
- Single paw-agent session
- Same model, same knowledge graph, same domain
- One prompt: "You are a foresight analyst. Given this knowledge graph about {domain},
  produce a structured foresight projection covering the next {horizon}."
- Run ONCE. Output stored in meta/baseline/
- Never re-run (unless the test domain changes)
- Tracked in every progress.md row for "is the engine worth it?" comparison

### Judges (3 Independent Paw-Agent Sessions)
- Each judge is a fresh session with NO context from the meta-agent or engine
- Each judge receives:
  - The rubric (12 criteria, 0-4 scale, with anchors)
  - Two anonymized outputs labeled "Output X" and "Output Y" (randomized assignment)
  - Instructions: "Score each output against each criterion. Cite specific evidence."
- Each judge produces 24 scores (12 criteria x 2 outputs) with reasoning
- Scores aggregated via Borda count across 3 judges
- Conservative tiebreak: incumbent (A) wins ties
- Judges do NOT know which output is incumbent vs challenger

## The Meta-Loop

### Per Iteration

```
1. READ    program.md (rubric) + progress.md (history) + previous diagnosis
2. PLAN    Identify one targeted change based on weakest criterion
3. MODIFY  Change one thing in the engine (skill, spec, WASM, architecture)
4. RUN     Trigger foresight engine on test domain → Output B (challenger)
5. JUDGE   3 blind judges score Output A (incumbent) vs Output B
6. SCORE   Borda count aggregation
7. DECIDE  B wins → new incumbent, tag version. A wins → revert, increment streak.
8. RECORD  Write scores, diagnosis, changelog, artifacts to meta/runs/{NNN}/
9. REPEAT  Until convergence (A wins k=2 consecutive) or max iterations
```

### Convergence

- If the incumbent wins 2 consecutive rounds, the engine has converged
- The meta-agent reports final scores and the improvement arc
- Convergence does NOT mean the engine is perfect — it means the meta-agent
  can't find improvements that survive blind judging

### Version Control

Each iteration is a git tag on the app state:

```
foresight-v000  — initial hybrid architecture (Phase 1 output)
foresight-v001  — first meta-improvement
foresight-v002  — second improvement
...
```

To reproduce any version: `git checkout foresight-v{NNN}`, rebuild, reinstall, re-run.

## Scoring Protocol

### Rubric: 12 Criteria, 0-4 Scale

Each criterion has specific anchors for each score level.
See program.md for the full rubric.

### Judge Input Format

Each judge receives a prompt structured as:

```
You are evaluating two foresight projections about {domain}.
Score each against the rubric below. For each criterion, provide:
- Score (0-4)
- Reasoning (1-2 sentences)
- Evidence (specific quotes or references from the output)

[RUBRIC]
{12 criteria with anchors}

[OUTPUT X]
{anonymized output — could be incumbent or challenger}

[OUTPUT Y]
{anonymized output — could be challenger or incumbent}
```

### Aggregation

For each criterion:
1. Each judge ranks X vs Y (higher score = rank 1)
2. Rank 1 gets 2 points, Rank 2 gets 1 point
3. Sum across 3 judges: max 6 points per criterion per output
4. Overall winner: sum of Borda points across all 12 criteria
5. Ties: incumbent wins

### Score Storage

Each run directory contains:

```
meta/runs/{NNN}/
├── scores.json         — raw scores from all 3 judges
├── borda.json          — aggregated Borda counts and winner
├── diagnosis.md        — root cause analysis of weakest criteria
├── changelog.md        — what was changed and why
├── engine-output/      — full engine artifacts (synthesis, observations, etc.)
├── incumbent-output/   — the A output (copy or reference to prior version)
└── judge-transcripts/  — full judge session transcripts
```

## What From AutoReason

Adapted from NousResearch/autoreason (tournament-based self-refinement):

| Borrowed | Rationale |
|----------|-----------|
| Blind judging with randomized order | Prevents positional bias |
| Borda count over majority voting | Captures ranking nuance, not just first-place |
| Conservative tiebreak (incumbent wins) | Prevents change-for-change's-sake |
| Convergence at k=2 incumbent wins | Natural stopping criterion |
| Fresh context per judge (no authorship stake) | Eliminates cascade bias |
| Inaction as first-class outcome | "No change needed" is a valid result |

Not borrowed:
- AB synthesis (our outputs are multi-artifact, can't merge easily)
- Critique-then-revise (our meta-agent reads raw traces, not summaries)
- Writing-focused prompts (ours is architectural modification)

## Constraints (from program.md)

1. Engine must remain a Temper app (entity specs, WASM, Cedar)
2. Rubric is immutable by the meta-agent
3. Score outputs, not process — the rubric measures prediction quality
4. Simpler wins at equal scores
5. The meta-agent can change anything about the engine except these constraints
