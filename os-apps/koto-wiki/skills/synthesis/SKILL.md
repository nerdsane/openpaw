# Skill: Synthesis

## Purpose

This skill encodes how Curator transforms raw sources into interlinked wiki pages. Synthesis is the core creative act — turning scattered references, corpus data, and native-speaker insights into coherent, navigable knowledge that Sensei can draw on mid-conversation.

## Core Principle

**A wiki page exists to make Sensei more effective.** Every page should answer: "If Sensei is teaching this concept right now, what context would make their encounter design better?" The page isn't for the student — it's for the teaching agent.

## Page Taxonomy

Every wiki page belongs to one category. Choose the most specific category that fits.

### Categories

| Category | Purpose | Example |
|----------|---------|---------|
| `grammar` | Single grammar point or construction | `tara-conditional`, `te-form-shimau` |
| `vocabulary_cluster` | Semantically related vocabulary group | `weather-words`, `cooking-verbs` |
| `kanji_family` | Kanji sharing a radical, component, or semantic field | `speech-radical-言`, `water-kanji-氵` |
| `cultural_context` | Cultural knowledge needed to understand language use | `uchi-soto-dynamics`, `seasonal-greetings` |
| `register_guide` | How language changes by social context | `keigo-levels`, `casual-contraction-patterns` |
| `genre_convention` | Language patterns specific to a media genre | `light-novel-narration-style`, `anime-character-speech` |
| `reading_strategy` | Techniques for approaching specific text types | `parsing-long-sentences`, `inferring-kanji-meaning` |
| `comparison` | Side-by-side analysis of confusable items | `conditional-forms-comparison`, `wa-vs-ga` |
| `index` | Category index or master index | `index` (per-persona root) |

### Category Selection

If a page spans categories, choose the primary category and use cross-references to connect to the secondary category. For example, a page about keigo in light novels is `register_guide/keigo-in-light-novels` with cross-references to `genre_convention/light-novel-narration-style`.

### Naming Convention

- Category names: lowercase, underscores for multi-word (`cultural_context`, not `CulturalContext`)
- Page slugs: lowercase, hyphens, descriptive (`conditional-forms`, not `grammar-01`)
- Slugs must be unique within a persona's wiki

### When to Add a Category

Add a new category when:
- 3+ pages do not fit cleanly into existing categories
- A natural grouping emerges from the sources that cuts across existing categories
- The persona's target profile emphasizes a domain that deserves its own namespace

## Create vs. Merge Decision

Before creating a new page, query existing pages in the target category:

```
temper.list("WikiPages", "$filter=PersonaId eq '{id}' and Category eq '{cat}'")
```

**Merge (revise existing page) when:**
- An existing page covers >60% of the same concepts
- The new content is an extension or update, not a new topic
- The existing page's slug would be the natural slug for the new content

**Create new page when:**
- No existing page covers more than 30% of the same concepts
- The new content represents a genuinely distinct topic
- Creating a separate page improves navigability (e.g., splitting a comparison into its own page)

**Edge case (30-60% overlap):**
- Prefer creating a new, more specific page and adding cross-references to the existing page
- Update the existing page to reference the new page

### Merge Workflow

1. Dispatch `BeginRevision` on the existing page with rationale
2. Download existing content from paw-fs
3. Merge new source material into existing content
4. Upload revised content to paw-fs (new file, update file_id)
5. Dispatch `Revise` with updated fields
6. Dispatch `Republish`

## Cross-Referencing Rules

Cross-references are the structural backbone of the wiki. They use `[[slug]]` syntax in the markdown body.

### Placement

In the body text, reference related pages inline:

```markdown
The て-form ([[te-form]]) is a prerequisite for understanding
the progressive aspect ([[progressive-te-iru]]).
```

In the Cross-References section at the bottom of each page, list all references explicitly with relationship descriptions.

### Relationship Types

| Relationship | Description |
|-------------|-------------|
| `prerequisite` | Understanding X requires understanding Y first |
| `related` | X and Y cover adjacent or overlapping topics |
| `contrast` | X and Y are often confused; this page distinguishes them |
| `extends` | X builds on Y (e.g., te-form + shimau extends te-form) |
| `example_of` | X is a specific instance of a general pattern described in Y |
| `used_in` | X appears frequently in the context described by Y |

### Bidirectional Links

Every cross-reference must be bidirectional. If page A references page B, page B must reference page A. When creating or updating a page, check all referenced pages and add backlinks if missing. If you cannot update the target page immediately, create a `WikiJob(type=revise)` to add the backlink.

### Structured Cross-References

In addition to markdown `[[slug]]` syntax, the WikiPage entity stores structured cross-references:

```json
"CrossReferences": [
  {"slug": "ba-conditional", "relationship": "contrast"},
  {"slug": "te-form", "relationship": "prerequisite"},
  {"slug": "conditional-forms-comparison", "relationship": "related"}
]
```

This enables programmatic link checking, graph analysis, and the tutor's ability to traverse the knowledge graph.

### Cross-Reference Rules

1. **Every page should have at least 2 outbound references** to related pages. Isolated pages are orphans.
2. **Do not reference non-existent pages.** Only use `[[slug]]` for pages that exist in Published or Revising state. If a page should exist but does not, note the gap and create a source_search job.
3. **Bidirectional awareness**: When creating a reference A -> B, check if B should also reference A. If so, update B or create a revision job.

## Concept Mapping

Wiki pages map to koto-learn Concepts. This connects the wiki to the teaching engine.

### Identifying Concept IDs

Query koto-learn Concepts to find matches:

```
temper.list("Concepts", "$filter=contains(Name, 'conditional') or contains(Description, 'tara')")
```

A single wiki page typically covers 1-5 concepts. A grammar page might cover one primary concept and reference 2-3 related ones. A comparison page covers all the items being compared.

### Mapping Rules

1. **Query existing concepts** before assigning concept_ids — do not guess IDs
2. **One page may map to multiple concepts** (a comparison page covers all compared items)
3. **One concept may appear on multiple pages** (the te-form concept appears on the te-form page and on pages about progressive, request, and sequential actions)
4. **Do not create concepts.** The Curator reads concepts from koto-learn but does not create or modify them. If a wiki page covers something that has no corresponding concept, note the gap in the page metadata but do not attempt to create the concept.

### ConceptIds Field

The WikiPage entity's `ConceptIds` field lists all concepts the page substantively covers (not merely mentions). A concept is "covered" if the page provides enough information for Sensei to teach it.

## Summary Writing

The Summary field is critical — it's what Sensei sees when browsing the wiki index or when the system injects wiki context into a teaching session.

**Rules:**
- 1-3 sentences maximum
- Optimized for LLM context injection (clear, self-contained, no references to "this page")
- State what the concept IS and what it DOES, not what the page contains
- Include the key distinction or insight that makes this concept notable
- Mention difficulty level or prerequisite knowledge

**Good:** "The tara-conditional expresses a temporal, realized condition ('when/if X happens, Y'). It is the most common conditional in spoken Japanese and the default choice when the condition is a specific, one-time event. Requires basic verb conjugation (N5)."

**Bad:** "This page covers the tara-conditional form in Japanese, including its conjugation, usage, and comparison with other conditional forms."

## Index Maintenance

After every synthesis session, update the persona's `index.md` in paw-fs.

### Index Format

```markdown
# {Persona Name} — Knowledge Index

## Grammar
- [[tara-conditional]] — Temporal/realized conditional; most common in speech. Difficulty: N4-N3.
- [[ba-conditional]] — Hypothetical conditional; formal/written preference. Difficulty: N4-N3.
- [[te-form]] — Connective form; base for many auxiliary constructions. Difficulty: N5-N4.

## Vocabulary Clusters
- [[weather-words]] — Core weather vocabulary with seasonal associations. Difficulty: N4-N3.
- [[cooking-verbs]] — Kitchen action verbs with transitivity pairs. Difficulty: N4.

## Kanji Families
- [[speech-radical-言]] — Kanji built on the speech/words radical. Difficulty: N4-N2.

## Cultural Context
- [[uchi-soto-dynamics]] — In-group/out-group social framing that drives register choice. Difficulty: N4-N2.

## Comparisons
- [[conditional-forms-comparison]] — tara vs ba vs to vs nara side-by-side. Difficulty: N4-N3.

## Register Guides
{...}

## Genre Conventions
{...}

## Reading Strategies
{...}

Last updated: {date}
Pages: {count} | Sources: {count}
```

### Update Rules

1. Update index.md after every page publish or republish
2. Sort entries within each category alphabetically by slug
3. Include the page summary (abbreviated to one line) and difficulty range
4. Add category headers only when the first page in that category is published
5. Update the footer counts

## Difficulty Range Assignment

Every WikiPage has a `DifficultyRange` JSON field (`{ "floor": 0.0, "ceiling": 1.0 }`) that indicates the difficulty band where the page is relevant.

These map to koto-learn's difficulty system (0.0-1.0 scale):
- **0.0-0.2:** N5 equivalent — basic grammar, hiragana/katakana, core vocabulary
- **0.2-0.4:** N4 equivalent — foundational grammar, basic kanji, everyday vocabulary
- **0.4-0.6:** N3 equivalent — intermediate grammar, compound sentences, broader vocabulary
- **0.6-0.8:** N2 equivalent — advanced grammar, nuanced expressions, literary vocabulary
- **0.8-1.0:** N1 equivalent — complex constructions, rare kanji, specialized vocabulary

A grammar page on the tara-conditional might have `DifficultyRange: { "floor": 0.2, "ceiling": 0.5 }` — introduced at N4 level but with nuances relevant up to N3.

Set the floor at the difficulty where the concept is first introduced. Set the ceiling at the highest difficulty where the page provides useful information (including advanced exceptions or nuances).

## Markdown Formatting Standards

Use this formatting for all wiki page content:

```markdown
# Page Title

Introduction paragraph explaining the concept.

## Section Name

**Japanese term** (romaji) -- brief definition.

> **例文:** 魔王が現れた。(Maou ga arawareta.) -- "The Demon Lord appeared."

| Pattern | Meaning | Example |
|---------|---------|---------|
| ~ている | progressive | 食べている (is eating) |

See also [[related-page]] for more on this topic.
```

### Content Storage Pattern

```python
markdown = '# Title\n\nContent with [[wikilinks]]...'
f = temper.write('/wiki/grammar/' + slug + '.md', markdown)
p = temper.create('WikiPages', {})
temper.action('WikiPages', p['entity_id'], 'Draft', {
    'persona_id': 'light-novel-reader',
    'slug': slug,
    'title': title,
    'category': 'grammar',
    'file_id': f['file_id'],
    'summary': '1-3 sentences for tutor context',
    'concept_ids': '[]',
    'cross_references': '[{"slug":"other-page","relationship":"related"}]',
    'source_ids': '["source-entity-id"]',
    'tags': '["grammar","n5"]',
    'difficulty_range': '{"floor":0.0,"ceiling":0.5}'
})
temper.action('WikiPages', p['entity_id'], 'Publish', {})
```

## Markdown Conventions

### Page Structure

Every wiki page follows this structure:

```markdown
# {Title}

{Summary -- 1-3 sentences optimized for LLM context injection}

## Overview

{2-4 paragraphs explaining the concept, its function, and why it matters}

## Details

{Main content -- examples, rules, patterns, exceptions}
{Use ### subheadings for sub-topics}

## Examples

{3-5 examples from the persona's domain, with glosses and translations}

### Example 1: {brief label}
**Japanese:** {example}
**Reading:** {furigana if needed}
**Gloss:** {word-by-word or phrase-by-phrase}
**Translation:** {natural English}
**Note:** {what this example demonstrates}

## Cross-References

- [[{slug}]] -- {relationship description}
- [[{slug}]] -- {relationship description}

## Sources

- {WikiSource ID}: {brief description}
- {WikiSource ID}: {brief description}
```

### Heading Conventions

- `#` (H1): Page title only, once per page
- `##` (H2): Major sections (Overview, Details, Examples, Cross-References, Sources)
- `###` (H3): Sub-topics within Details, individual examples
- `####` (H4): Avoid if possible — if needed, the page may be too large and should be split

### Example Formatting

Japanese examples always include:
1. The Japanese text (with kanji as appropriate for the persona's level)
2. Reading aid (furigana in parentheses for kanji above the persona's current level)
3. Gloss (structural breakdown)
4. Natural English translation
5. Note explaining what the example demonstrates

### Source Citation Format

In-page citations use WikiSource IDs: `(WS-023)` or `(WS-023, WS-041)`. The Sources section at the bottom provides full details.

## paw-fs Workflow for Synthesis

1. Determine the file path: `/{persona}/{category}/{slug}.md`
2. Check if the directory exists; create if needed via `temper.create("Directories", {...})`
3. Create the file metadata: `temper.create("Files", {"Name": "{slug}.md", "Path": "/{persona}/{category}/{slug}.md", "WorkspaceId": "...", "MimeType": "text/markdown"})`
4. Upload the synthesized markdown: `PUT /tdata/Files('{file_id}')/$value`
5. Use the returned `file_id` in the WikiPage entity's `FileId` field
6. After creating/updating the page, update `/{persona}/index.md` with the new entry
