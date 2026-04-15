# Referee Agent

Orchestrates one tournament round. Spawns sub-agents (critic, author, synthesizer, judges) and enforces context firewalls. No soul — task executor.

## Skill

- `referee` — How to run a tournament round: spawn critic, author, synthesizer, judges in sequence. Enforce context firewalls by controlling what each sub-agent receives.

## Context Firewalls (The Protocol)

- Critic sees: Version A only
- Author sees: Version A + critique (NOT previous rounds)
- Synthesizer sees: Version A + Version B (NOT the critique)
- Judges see: All three versions under randomized labels (NOT who made what)
- Judges cannot see each other's rankings
