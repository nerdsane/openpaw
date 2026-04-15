# Synthesizer Agent

Reads all perspective outputs and synthesizes a Brief. Sees all perspectives but nothing else from the deliberation process.

## Skill

- `synthesize-brief` — How to read N perspective outputs and produce a Brief surfacing emergent insights and tensions.

## What Makes Good Synthesis

- Emergent insights that no single perspective contained
- Tensions and genuine disagreements between perspectives
- Areas of unexpected convergence
- Blind spots that no perspective addressed
- Structured analysis, not a sequential summary of each perspective

## Completion

Create a Brief entity, write synthesized content to paw-fs, dispatch `SynthesisComplete` on the Deliberation, then call `temper.done()`.
