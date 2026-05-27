# ADR-0061: Directed Evolution Human Episode Starter

## Status

Proposed.

## Context

The Directed Evolution control app can already represent human-gated growth:
directions are proposed from observed pressure, an episode can enter
negotiation, an Adaptation Goal and Viability Constraints can be recorded, and
`StartEpisode` triggers the normal generation pipeline. Repair can also
auto-start through app-side WASM when an active AutonomyPolicy permits it.

The remaining growth-lane gap is the bridge from human-brain conversation to
real Temper entities. In the desired V1, the human negotiates the adaptation
goal, viability constraints, selection pressure, evaluation stages,
elimination rules, scoring rules, and metrics with Codex in chat. Mission
Control stays mostly observational. After the conversation reaches agreement,
Codex needs a bounded way to write that agreement into the Directed Evolution
control plane without inventing a second workflow or pretending the dashboard is
the negotiation surface.

## Decision

`paw-codex-worker` will expose a one-shot local command for starting a
human-directed Directed Evolution episode from a JSON contract:

```bash
paw-codex-worker directed-evolution-start-episode episode-contract.json
```

The command:

- reads the contract produced by the human-Codex conversation;
- fetches the selected Direction and Organism for missing defaults;
- creates and activates MetricDefinition, AdaptationGoal,
  ViabilityConstraint, EliminationRule, ScoringRule, SelectionPressure, and
  EvaluationStage entities;
- records the Episode contract;
- dispatches `Direction.SelectDirection`; and
- dispatches `Episode.StartEpisode`, letting the existing Directed Evolution
  app trigger generation and worker brain runs.

This command is not a brain. It does not choose the contract, silently approve
fitness, or bypass the human-gated lane. It is the local agent hand that
persists an already-negotiated contract through real Temper actions.

The command sends its Directed Evolution writes as a `codex` agent by default
(`DIRECTED_EVOLUTION_DIRECTOR_AGENT_TYPE=codex`) because the worker daemon
itself normally uses `worker`, which is intentionally not authorized to start
human-gated episodes. Operators may override
`DIRECTED_EVOLUTION_DIRECTOR_ID` and `DIRECTED_EVOLUTION_DIRECTOR_AGENT_TYPE`
for a specific run.

## Consequences

- Human-gated growth can enter the same real pipeline as repair: generation,
  hot-loaded variants, AI simulated-user stages, review, selection, promotion,
  and lineage.
- Mission Control can remain truthful and mostly observational; it can show the
  selected direction and resulting episode without hosting the chat.
- The worker remains the local execution plane. Codex is not deployed inside
  Railway.
- A malformed or incomplete contract fails before `StartEpisode`, leaving
  normal Temper state and Cedar policies in charge.

## Verification

- Unit tests cover contract normalization, direction/organism defaults, metrics,
  rule metric resolution, and required growth-stage defaults.
- A live proof should take a proposed growth Direction, run this command with a
  human-Codex contract, and verify that Mission Control shows a Running episode
  with the recorded Adaptation Goal, Viability Constraints, Selection Pressure,
  evaluation stages, generated variants, eliminations, and promotion lineage.
