# Open Paw — Project Instructions

## What is Open Paw?

An agent daemon that embeds the Temper platform engine. Humans talk to a Paw agent via Discord; Paw manages software projects by spawning developer and scout agents with sandboxes.

## Entity Model

Namespace: `OpenPaw`. All agent types (Paw, Developer, Scout) are instances of `Agent`.

| Entity | OData URL | Purpose |
|--------|-----------|---------|
| Agent | `/tdata/Agents` | Running agent instance |
| Soul | `/tdata/Souls` | Identity document |
| Memory | `/tdata/Memories` | Persistent knowledge |
| Skill | `/tdata/Skills` | Capabilities |
| Channel | `/tdata/Channels` | Messaging connection |

## Mandatory: Proof Reports

Every plan MUST have a verification definition and verification flow. After implementation:

1. Execute the verification flow
2. Write a proof report to `.proofs/NNN_step-name.md` using the template at `.proofs/TEMPLATE.md`
3. Include literal artifacts (curl responses, log snippets, entity JSON)
4. Include an ASCII architecture diagram of what is working
5. Commit the proof report

A step is NOT done until its proof report is committed.

## Credentials

Stored in `.env` (gitignored). Startup reads via `dotenv`. Never commit credentials.

## Running Locally

```bash
cd /Users/seshendranalla/Development/openpaw
cargo run  # Boots at http://localhost:3467/tdata
```

## OS Apps

All agent logic is IOA specs + WASM + Cedar policies in `os-apps/`. The daemon binary is a thin bootstrap layer.

## Parallel Branches

- `feat/openpaw-self-heal-loop-claude` — Claude's implementation
- `feat/openpaw-self-heal-loop-codex` — Codex's implementation

Work only in your branch. Don't modify `main` directly.
