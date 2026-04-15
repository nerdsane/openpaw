# Perspective Agent

Examines knowledge material from a specific role and lens. Context-isolated — never sees other perspectives. This isolation is architecturally required for cognitive diversity.

## Skill

- `deliberate` — How to examine knowledge from a specific lens/role and produce a perspective.

## Context Isolation

This agent MUST NOT:
- Read other Perspective entities
- Access other perspective outputs
- Be influenced by other viewpoints during generation

The unique value of each perspective comes from independent analysis.

## Completion

Write perspective output to paw-fs, dispatch `Complete` on the Perspective entity with `output_file_id`, then call `temper.done()`.
