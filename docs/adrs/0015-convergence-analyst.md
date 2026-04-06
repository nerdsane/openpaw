# ADR-0015: Convergence Analyst Agent

## Status

Accepted

## Context

The Foresight engine's advance_step WASM detected convergence via string matching on `signal_refs`. If 2+ Probes referenced the same signal key (e.g., `"pr:97"`), one Observation was Confirmed. This was broken in two ways:

1. **False positives**: Two Probes reference the same PR but draw opposite conclusions. String matching confirms one, missing the contradiction.
2. **False negatives**: Two Probes describe the same risk using different signal keys. String matching sees no overlap.

Convergence detection is fundamentally a semantic judgment. It requires understanding what the Observations mean, not which strings they share.

## Decision

Replace string-matching convergence with a Convergence Analyst — a task agent (Agent + Session, no soul) spawned by advance_step after each step.

The analyst:
1. Receives all Observations from the completed step (serialized in user_message)
2. Compares pairs from different Probes for semantic convergence
3. Dispatches Confirm on genuinely converging Observations
4. Creates contradiction-flagging Observations when Probes disagree
5. Runs asynchronously — does not block step advancement

### Why a separate agent, not inline WASM

The advance_step WASM has no LLM access. Semantic analysis requires an LLM. The analyst runs as a standard Session (LLM caller + monty_repl), reusing all existing infrastructure.

### Why not a Probe

Probes must be independent — they MUST NOT read each other's Observations. The analyst is a judge, not an observer. It reads Probes' output to identify agreement/disagreement.

### Why no soul

The analyst has no persistent identity. It is a single-task agent that receives instructions (AGENT.md) and executes. No SOUL.md, no STYLE.md.

## Alternatives Considered

**Embedding model + cosine similarity**: Detects textual similarity but misses logical agreement/disagreement. Two Observations can use similar words yet draw opposite conclusions.

**LLM call inside WASM**: WASM modules have bounded execution time and no access to the Session turn loop. Would block the Projection state machine and risk timeout.

**Scheduled Projection action**: Adds unnecessary complexity to the Projection state machine for something that runs independently.

## Consequences

- Semantic convergence replaces string matching — catches real agreement and real disagreement
- Contradictions become a new signal type (Observations with "CONTRADICTION:" prefix)
- One additional LLM session per step (bounded by max_turns=30)
- Confirmations arrive asynchronously, not during advance_step execution
