# ADR-0021: Per-App ADR Folders

## Status

Accepted

## Context

Temper apps already colocate their specs, policies, reactions, and WASM modules, but the reasoning behind those choices is often lost in chat transcripts or pull request discussions. That makes it harder for humans and agents to evolve an app safely when they were not present for the original design.

## Decision

Every Temper app in OpenPaw may include an `adrs/` directory with app-local architecture decision records. These ADRs live beside the app they govern, not in a central documentation folder, and capture why the app uses its current entity types, state machines, policy shapes, integration patterns, or cross-app dependencies.

The convention is:

```
os-apps/my-app/
├── APP.md
├── adrs/
│   └── 001-initial-design.md
├── specs/
├── policies/
├── wasm/
└── reactions/
```

Agent-facing guidance for creating and evolving apps must instruct agents to record ADRs before submitting materially new specs. Runtime-created apps should follow the same convention through TemperFS paths under `/apps/{app-name}/adrs/`.

## Consequences

### Positive

- App-level reasoning stays close to the code and specs it governs.
- Agents can inspect prior design decisions before evolving an app they did not create.
- Reviewers gain a consistent place to look for architecture intent without searching old chats.

### Negative

- App authors now have one more artifact to maintain when design changes.
- Lightweight apps may still need judgement about how much ADR detail is worth writing.
