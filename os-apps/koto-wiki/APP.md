# koto-wiki

Persona-scoped knowledge plane for Kotowari, following Karpathy's LLM Wiki pattern. The wiki provides a structured, interlinked knowledge surface that the tutor agent consults when designing encounters -- a curated layer between raw sources and the teaching moment.

## Why a Wiki?

LLMs work best when they can ground reasoning in retrieved, structured knowledge rather than relying solely on parametric memory. The wiki acts as the tutor's "textbook shelf": each persona maintains its own corpus of wiki pages synthesized from source material, cross-referenced to the koto-learn knowledge graph, and kept current by an automated Curator agent.

## Entity Types

### WikiSource

A raw knowledge source submitted for processing. Sources can be web articles, corpus data files, reference materials, file downloads, or encounter insights (knowledge extracted from completed encounters). Each source tracks its paw-fs file reference, extracted topics, and the wiki pages derived from it.

**States:** `Submitted` -> `Indexed` | `Failed`

### WikiPage

A synthesized, versioned wiki article. Pages are scoped to a persona and linked to koto-learn Concepts via `concept_ids`. They carry cross-references to other wiki pages, difficulty ranges, and tags for retrieval. Content is stored in paw-fs (referenced by `file_id`); the entity holds metadata, summary, and linkage.

**States:** `Drafting` -> `Published` <-> `Revising` -> `Archived`

Pages support a revision cycle: a published page can be pulled back into revision (with a rationale), updated, and republished with an incremented version counter. The lint status tracks health checks without triggering state changes.

### WikiJob

A trackable unit of work for the Curator agent. Jobs cover ingest (processing a source into pages), query (tutor requesting pages for encounter design), and lint (health-checking page quality, broken cross-references, stale content).

**States:** `Queued` -> `Running` -> `Completed` | `Failed`

## The Wiki Curator Agent

The Curator is a Soul-based agent (defined separately in koto-tutor or a dedicated soul spec) that:

1. **Ingests sources** -- when a WikiSource is submitted, the Curator reads the raw content from paw-fs, extracts topics, synthesizes one or more WikiPage drafts, and dispatches `Index` on the source.
2. **Responds to queries** -- when the tutor needs knowledge for encounter design, the Curator finds relevant pages by concept, tag, or difficulty and returns summaries.
3. **Lints the wiki** -- periodic health checks verify cross-references are valid, pages cover their linked concepts adequately, and no stale content lingers.

All state transitions are dispatched by the Curator agent directly -- there are no WASM integrations in koto-wiki. The agent reads sources, calls LLMs for synthesis, and writes results back through the Temper OData API.

## Integration with koto-learn

- `WikiPage.concept_ids` is a JSON array of Concept entity IDs from koto-learn. This links wiki knowledge to the teaching graph.
- `WikiPage.difficulty_range` aligns with Concept difficulty bands, allowing the tutor to select wiki content at the right level for a learner.
- `WikiSource.source_type = "encounter_insight"` captures knowledge that emerges from completed encounters, feeding the wiki from teaching experience.
- Personas scope the entire wiki: each persona maintains its own set of sources and pages, ensuring the wiki reflects the persona's target competency profile.

## Storage Model

Entities hold metadata, linkage, and summaries. Actual content (raw source text, synthesized wiki page markdown) lives in paw-fs File entities, referenced by `file_id` fields. This keeps entity payloads small and allows content to be versioned and streamed independently.

## Operations

| Operation | Flow |
|-----------|------|
| **Ingest** | Submit WikiSource -> Curator reads paw-fs file -> extracts topics -> creates WikiPage drafts -> publishes pages -> dispatches Index on source |
| **Query** | Tutor requests pages by concept/tag/difficulty -> Curator filters WikiPages -> returns summaries + file references |
| **Lint** | Curator creates WikiJob(lint) -> checks cross-references, concept coverage, staleness -> updates lint_status on pages -> completes job |
