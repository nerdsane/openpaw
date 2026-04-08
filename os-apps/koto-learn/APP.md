# koto-learn

Core learning engine for Kotowari. Manages the structured knowledge that powers language acquisition.

## Capabilities

- **Knowledge Graph:** Directed graph of language concepts (grammar patterns, vocabulary clusters, kanji, pragmatic skills) with prerequisite links, difficulty bands, and persona coverage tags. The graph defines what can be learned and in what order.

- **Mastery Tracking:** Per-learner, per-concept mastery profiles across four dimensions: recognition, production, contextual use, and transfer. Updated by tutor agents after each encounter. Supports mastery advancement and confirmed regression.

- **Encounter Lifecycle:** Entity state machine for encounters (designed, presented, active, completed, abandoned). Encounters link to target concepts, learner profiles, and mastery observations. The lifecycle ensures every teaching interaction is tracked and its outcomes recorded.

- **Knowledge Bank Import:** Ingestion pipeline for corpus material (light novel text, subtitle files, manga dialogue). Imported content is tagged with concept coverage, difficulty level, and persona relevance. Used by tutors to source authentic examples for encounters.

- **Learner Profiles:** Identity and configuration for each learner. Includes active persona, trust level (guided/balanced/autonomous), session preferences, and learning history. Evolved by GEPA/system, read by tutors.

- **Persona Definitions:** Target competency profiles that scope the curriculum. Each persona defines vocabulary, kanji, grammar, and skill targets along with corpus sources and a definition of done. Learner profiles reference a persona to determine what "mastery" means for that student.

## Entity Types

- `LearnerProfile` — A learner's identity and configuration
- `Persona` — A target competency profile
- `KnowledgeBank` — Domain dataset with import pipeline
- `Concept` — A teachable unit in the knowledge graph
- `ConceptLink` — A prerequisite or related-concept edge
- `Mastery` — Per-concept mastery state for a learner (4 dimensions)
- `Encounter` — A single teaching interaction

## Policies

- `learning.cedar` — Progressive scaffolding based on trust level
- `tutor.cedar` — Tutor agent permissions (read knowledge, assess mastery, cannot modify graph)

## Setup

To set up Kotowari in a Temper workspace:

1. **Install the koto-learn app** (provides all entity types):
   ```
   install_app("your-workspace", "koto-learn")
   ```

2. **Install the koto-tutor app** (provides Sensei soul, skills, policies):
   ```
   install_app("your-workspace", "koto-tutor")
   ```

3. **Bootstrap Sensei's Soul** (Paw or operator creates the Soul entity):
   - Create a TemperFS File entity for the soul content
   - Upload `koto-tutor/agents/sensei/SOUL.md` + `STYLE.md` to the File
   - Create a Soul entity with `ContentFileId` pointing to the File
   - Dispatch `OpenPaw.Publish` on the Soul
   (Same flow that Paw, SWE, SRE souls use at OpenPaw boot.)

4. **Create a KnowledgeBank** and trigger import:
   ```
   create("your-workspace", "KnowledgeBanks", {...})
   action("your-workspace", "KnowledgeBanks", id, "BeginImport")
   ```

5. **Create Personas** from seed data and publish them.
