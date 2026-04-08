# Skill: Wiki Maintenance

## Purpose

This skill encodes how Curator keeps the wiki healthy — finding orphan pages, broken cross-references, content gaps, contradictions, and stale information. A wiki that isn't maintained degrades over time. Maintenance ensures the knowledge graph stays accurate, navigable, and complete.

## Core Principle

**A healthy wiki has no dead ends, no broken links, and no blind spots.** Every page should be reachable via cross-references. Every concept in the persona's target profile should have wiki coverage. Every claim should trace back to a source that's still current.

## Lint Checks

Run these checks during every maintenance session. Report findings with severity levels.

### 1. Orphan Pages

**Definition:** A published WikiPage with zero inbound cross-references from other pages. Orphan pages are effectively invisible — no navigation path leads to them.

**Detection:**
1. Query all published pages: `temper.list("WikiPages", "$filter=PersonaId eq '{id}' and State eq 'Published'")`
2. Collect all `CrossReferences[].slug` values across all pages into a set of referenced slugs
3. Also scan markdown content of each page for `[[slug]]` references
4. Any published page whose slug does not appear in the referenced set is an orphan

**Severity:** Warning

**Remediation:** Create a `WikiJob(type=revise)` to update related pages with cross-references to the orphan. If no related pages exist (the orphan covers a topic disconnected from everything else), flag it for human review — it may belong in a different persona's wiki or may not belong at all.

**Lint status:**
```json
{ "orphan": true, "inbound_count": 0 }
```

### 2. Broken Cross-References

**Definition:** A `[[slug]]` reference in a page's markdown or a slug in the structured `CrossReferences` JSON that does not match any published WikiPage's slug.

**Detection:**
1. For each published page, extract all `[[slug]]` references from the markdown content
2. Also check each entry in the structured `CrossReferences[].slug` field
3. Verify each slug matches a published page: `temper.list("WikiPages", "$filter=Slug eq '{slug}' and PersonaId eq '{pid}' and State eq 'Published'")`
4. Any non-matching slug is a broken reference

**Severity:** Error

**Remediation:**
- If the referenced page never existed and the slug represents a valid topic, create a `WikiJob(type=source_search)` to find sources, followed by a `WikiJob(type=synthesize)` to create the missing page
- If the slug has a typo (close match exists), create a `WikiJob(type=revise)` to fix the reference
- If the referenced page was archived, create a `WikiJob(type=revise)` to remove or replace the reference

**Lint status:**
```json
{ "broken_refs": ["[[nara-conditional]]"], "broken_ref_count": 1 }
```

### 3. Contradictions Between Pages

**Definition:** Two or more pages make conflicting claims about the same topic.

**Detection:**
1. Group pages by overlapping `ConceptIds` — pages sharing 2+ concepts are candidates
2. Also check pages connected by `contrast` cross-references
3. For candidate pairs, compare key claims — especially grammar rules, usage guidelines, and frequency assertions
4. Flag cases where page A says X and page B says not-X, or where the same example is glossed differently

**Severity:** Error

**Remediation:**
- Check the source trail for both claims. If the sources themselves disagree on a substantive point, escalate to human review
- If one page cites a lower-quality source, create a `WikiJob(type=revise)` to correct it
- If both are valid perspectives (e.g., dialect differences, formal vs. informal register), add cross-references noting the distinction rather than eliminating one

### 4. Stale Content

**Definition:** A page whose sources have been superseded by newer information, or whose content hasn't been updated despite newer sources being available.

**Detection:**
1. For each published page, check its `SourceIds`
2. For each WikiSource, check the `UpdatedAt` date
3. Flag pages whose newest source is older than 12 months
4. Optionally search for newer sources on the same topic: `temper.web_search()` with the page's core keywords
5. If newer authoritative sources exist, the page is potentially stale

**Severity:** Info (unless the newer source contradicts the page content, then Warning)

**Remediation:** Create a `WikiJob(type=source_search)` for the updated sources, then a `WikiJob(type=revise)` to incorporate them.

**Staleness indicators:**
- Source URLs returning 404 (dead links)
- Pages not revised in 6+ months while related pages have been updated multiple times
- Pages marked with training-data-only sources that could now be replaced with web-sourced content

### 5. Unidirectional Cross-References

**Definition:** Page A references page B, but page B does not reference page A. Cross-references should be bidirectional.

**Detection:**
1. Build a directed graph from all `CrossReferences` fields across all published pages
2. For each edge A->B, check if B->A exists
3. Missing reverse edges are unidirectional references

**Severity:** Info

**Remediation:** Create `WikiJob(type=revise)` for the page missing the backlink. The revision should add a cross-reference with an appropriate relationship type.

## Gap Detection

Gap detection compares the wiki's coverage against what the persona needs.

### Concept Coverage Gaps

1. Read the persona's target profile: `temper.read_entity("Personas", persona_id)`
2. Extract the target concept set (all Concept IDs the persona should eventually cover)
3. Query all published WikiPages and collect their `ConceptIds`:
   ```
   pages = temper.list("WikiPages", "$filter=PersonaId eq '{id}' and State eq 'Published'")
   covered_concepts = union of all page ConceptIds
   ```
4. Any concept in the target set that does not appear in any page's `ConceptIds` is a gap

**Prioritization — rank gaps by:**
- **Prerequisite count:** Concepts that are prerequisites for many other concepts should be covered first — they unblock downstream coverage
- **Difficulty level:** Concepts at the learner's current mastery frontier are more urgent than concepts far ahead
- **Related page count:** Concepts adjacent to well-covered areas are easier to fill (sources and context likely already exist)
- **Domain frequency:** Concepts that appear frequently in the persona's target domain (e.g., narrative grammar for a light novel reader)

**Remediation:** Create `WikiJob(type=source_search)` for the highest-priority gaps. Batch related concepts — if five N4 grammar gaps share a theme, create one job to cover them together rather than five separate jobs.

### Category Coverage Gaps

Some categories may be systematically underpopulated:
- If the persona targets N3 readiness but `cultural_context/` has only 2 pages, cultural context is a category gap
- If `comparison/` pages exist for vocabulary but not grammar, grammar comparisons are a category gap
- If `reading_strategy/` is empty despite the persona being a reader, reading strategies are a category gap

Report category gaps in the health metrics and create sourcing jobs for the most impactful gaps.

### Growth Signals

Beyond the target profile, new gaps emerge from:
- **Encounter feedback:** Sensei creates insights when teaching reveals something the wiki does not cover
- **Cross-reference dead ends:** Pages that reference concepts with no wiki coverage
- **Learner questions:** Topics the student asks about that the tutor cannot find in the wiki

## Revision Management

### When to Revise vs. Create New

**Revise an existing page when:**
- New sources provide additional examples or nuance for the same topic
- A lint check found errors in the existing page
- The page's difficulty range needs adjustment based on new concept mapping
- Cross-references need to be added or updated
- The summary is too vague for effective LLM context injection

**Create a new page when:**
- The new content represents a genuinely distinct topic
- The existing page would become unwieldy (>2000 words) with the addition
- The new content deserves its own cross-reference target (other pages need to link to it specifically)

### Revision Workflow

1. Create `WikiJob(type=revise)` with:
   - Target WikiPage ID
   - Reason for revision (lint finding, new sources, gap fill)
   - New source IDs (if applicable)
   - Specific changes requested
2. During the revision session:
   - Download the current page content from paw-fs
   - Make targeted changes (don't rewrite the whole page unless necessary)
   - Update the Sources section if new sources were used
   - Update cross-references if new connections were discovered
   - Re-upload to paw-fs
   - Dispatch `BeginRevision` then `Revise` then `Republish` actions on the WikiPage entity
3. After revision:
   - Update `index.md` if the summary changed
   - Check cross-referenced pages for needed backlinks

## Cross-Reference Hygiene

Beyond lint checks, proactive cross-reference maintenance improves wiki quality.

### Ensuring Bidirectional Links

After any page creation or revision:
1. For every `[[slug]]` in the new/updated page, check if the target page links back
2. If not, add a backlink with the appropriate relationship type
3. Update both the markdown content and the structured `CrossReferences` JSON

### Relationship Accuracy

Periodically review cross-reference relationship types:
- `prerequisite` links should follow the koto-learn knowledge graph's prerequisite structure
- `contrast` links should connect genuinely confusable items, not just related ones
- `extends` links should reflect true compositional relationships (X is built from Y)

### Discovering New Cross-References

When reviewing pages during maintenance:
- If two pages share 2+ ConceptIds but don't cross-reference each other, they likely should
- If a page mentions a concept by name but doesn't `[[link]]` to its page, add the link
- If a new page is created that should logically connect to 5+ existing pages, ensure all connections are made

## Maintenance Job Creation

When lint or gap detection finds issues, create WikiJob entities to track remediation:

```
temper.create("WikiJobs", {
  "Type": "source_search|synthesize|revise|lint",
  "PersonaId": "...",
  "Priority": 1-5,
  "Description": "...",
  "Parameters": {
    "topic": "...",
    "concept_ids": ["..."],
    "target_page_id": "...",
    "reason": "..."
  }
})
```

### Priority Assignment

| Priority | Criteria | Examples |
|----------|----------|----------|
| 1 (Critical) | Factual error in a published page — Sensei may teach wrong information | Contradiction with authoritative source, incorrect grammar rule |
| 2 (High) | Broken cross-reference — navigation is broken | `[[slug]]` pointing to non-existent page |
| 3 (Medium) | Concept gap for a concept at the learner's current frontier | Missing page for a concept Sensei needs to teach this week |
| 4 (Low) | Orphan page, unidirectional cross-reference, category gap | Structural issues that affect navigability but not accuracy |
| 5 (Background) | Stale content, stylistic improvements, additional examples | Quality improvements with no immediate teaching impact |

## Health Metrics

Produce these metrics in every maintenance report.

### Summary Metrics

```
Wiki Health — {persona_name}
  Total pages: {published_count} published, {draft_count} draft
  Orphan pages: {count} (pages with zero inbound cross-references)
  Broken refs: {count} ([[slug]] references with no matching page)
  Contradictions: {count} (conflicting claims between pages)
  Concept gaps: {count} of {target_count} target concepts without wiki coverage ({coverage_pct}%)
  Category coverage:
    grammar: {page_count} pages ({concept_coverage}% of target grammar concepts)
    vocabulary_cluster: {page_count} pages ({concept_coverage}%)
    kanji_family: {page_count} pages ({concept_coverage}%)
    cultural_context: {page_count} pages
    register_guide: {page_count} pages
    genre_convention: {page_count} pages
    reading_strategy: {page_count} pages
    comparison: {page_count} pages
  Avg cross-refs/page: {avg}
  Unidirectional refs: {count}
  Stalest page: {slug} (last updated {date}, {newer_source_count} newer sources available)
```

### Trend Indicators

Compare against the previous maintenance run:
- Pages added since last run
- Gaps closed since last run
- New gaps introduced (if persona target_profile expanded)
- Orphan count trend (increasing = structural problem, needs attention)
- Average cross-refs trend (decreasing = new pages being added without proper linking)

### Lint Status Schema

The `lint_status` field on WikiPage stores the latest results per page:

```json
{
  "checked_at": "2026-04-06T12:00:00Z",
  "orphan": false,
  "inbound_count": 3,
  "broken_refs": [],
  "broken_ref_count": 0,
  "stale": false,
  "contradiction_flags": [],
  "overall": "healthy"
}
```

Possible `overall` values: `healthy`, `needs_attention`, `critical`.

## Reporting Format

Structure findings by severity, with specific actionable items.

```
## Maintenance Report — {persona_name} — {date}

### Errors (must fix)
1. **Broken ref:** [[nara-conditional]] referenced by grammar/conditional-forms-comparison — page does not exist. Job created: WJ-087 (source_search, priority 2).
2. **Contradiction:** grammar/passive-form states ni always marks agent. grammar/indirect-passive states ni marks experiencer in indirect passives. Sources WS-023 and WS-034 disagree. Escalated for human review.

### Warnings (should fix)
1. **Orphan:** vocabulary_cluster/onomatopoeia-weather has zero inbound links. Related pages: grammar/to-quotative, cultural_context/seasonal-expressions. Job created: WJ-088 (revise, priority 4).
2. **Coverage gap:** 14 N3 grammar concepts have no wiki coverage. Top 5 by prerequisite count: causative, passive-causative, volitional-form, potential-form, nara-conditional. Job created: WJ-089 (source_search, priority 3).

### Info (nice to fix)
1. **Stale:** grammar/copula-da last updated 2025-11-03. 2 newer sources available. Job created: WJ-090 (source_search, priority 5).
2. **Unidirectional:** 7 cross-references are one-way. Job created: WJ-091 (revise, priority 4).

### Health Metrics
{structured summary as above}
```

## Maintenance Session Cadence

- **After every synthesis session:** Run a targeted lint on the newly created/updated pages and their cross-referenced neighbors
- **Periodic full lint:** Run a full wiki lint every N sessions (configurable). Create a WikiJob(lint) with `Parameters: {"scope": "full", "persona_id": "..."}`
- **On-demand:** The tutor or human can create a WikiJob(lint) targeting specific pages or categories
