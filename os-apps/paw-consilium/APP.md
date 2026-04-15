# paw-consilium

Multi-perspective deliberation -- multiple perspectives examining knowledge from different angles, synthesized into a rich Brief. Supports cross-model diversity (different providers per perspective).

## Entity Types

- **Deliberation** -- Orchestrates multi-perspective examination. Config supports cross-model diversity. Lifecycle: `Created -> Convened -> Generating -> Synthesizing -> Complete | Failed`.
- **Perspective** -- Context-isolated viewpoint. Each generated independently — no perspective sees another. Lifecycle: `Created -> Generating -> Complete | Failed`.
- **Brief** -- Synthesized multi-perspective output surfacing emergent insights and tensions. Lifecycle: `Draft -> Published -> Archived`.

## WASM Modules

- **spawn_perspectives** -- On Deliberation.Convene: creates N Perspective entities + N sessions from config_json. Deterministic fan-out.
- **check_and_synthesize** -- On each PerspectiveComplete: counts completions, when all done spawns synthesizer. Deterministic fan-in.

## Agents (Isolated -- context firewalls required)

- **perspective** -- Examines knowledge from a specific role/lens. Context-isolated from other perspectives.
- **synthesizer** -- Reads all perspective outputs, produces Brief. Sees all perspectives but nothing else.

## Skills

- **deliberate** -- How to examine knowledge from a specific lens and produce a perspective.
- **synthesize-brief** -- How to combine N perspective outputs into a Brief.

## Key Properties

- Context isolation is the protocol. Perspectives never see each other during generation.
- Cross-model diversity: each perspective can use a different model/provider.
- Synthesis produces emergent insights, not summaries.

## Dependencies

- `paw-agent` -- Session entity for agent spawning
- `paw-fs` -- File storage for knowledge material, perspectives, and briefs
