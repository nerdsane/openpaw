# koto-tutor

Sensei tutor agent for Kotowari. Teaches Japanese through immersive, contextual encounters adapted to each learner's persona and mastery level.

## Capabilities

- **Contextual Teaching:** Designs and runs encounters where the target concept is embedded in a situation the student cares about. Encounter types include narrative, conversation, puzzle, immersion, reflection, and production challenge. Never repeats the same format twice in a row.

- **Japanese Language Expertise:** Deep knowledge of Japanese linguistics: particles, verb conjugation, kanji/radical systems, honorifics, register, and sentence structure. Understands common pitfalls for English speakers and teaches through showing, not telling.

- **Mastery Assessment:** Evaluates student mastery through conversation, not quizzes. Tracks four dimensions (recognition, production, contextual use, transfer) by observing self-corrections, hesitation patterns, creative use, and speed of recognition. Assessment is invisible to the student.

- **Adaptive Pacing:** Adjusts difficulty and concept sequencing mid-conversation based on student responses. Knows when to push (student succeeding easily), when to slow down (student struggling), and when to change direction (student energized by an unexpected topic).

- **Persona-Grounded Content:** All examples, scenes, and dialogues draw from the student's declared interests. For a light novel reader: isekai scenarios, guild interactions, battle dialogue, character register shifts. Generic examples are never acceptable.

## Agent

- `agents/sensei/SOUL.md` — Sensei's identity, sensibility, worldview, tensions, and boundaries
- `agents/sensei/STYLE.md` — Communication style, language mixing ratios, error handling, message cadence
- `agents/sensei/AGENT.md` — Operational instructions: session lifecycle, encounter workflow, entity operations

## Skills

- `skills/japanese-language/SKILL.md` — Japanese-specific teaching strategies for particles, kanji, verbs, honorifics, sentence structure
- `skills/contextual-teaching/SKILL.md` — Encounter design methodology, encounter types, sequencing, pacing principles
- `skills/mastery-assessment/SKILL.md` — Multi-dimensional mastery model, observable signals, regression handling, invisible assessment

## Policies

- `policies/autonomy.cedar` — What Sensei can do autonomously vs. what requires approval
- `policies/tool_governance.cedar` — Tool restrictions (Temper CRUD + research only, no bash/fs/code)
