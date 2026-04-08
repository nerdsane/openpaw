# Skill: Wiki Consultation -- Querying the Knowledge Wiki for Encounter Design

## Purpose

This skill teaches Sensei how to query the koto-wiki knowledge base when designing encounters. The wiki contains synthesized, cross-referenced knowledge pages maintained by the Curator agent. Sensei consults the wiki to ground encounters in accurate, sourced content rather than relying solely on parametric memory.

## When to Consult the Wiki

### Always Consult

- **Before designing an encounter** for a concept: check if a wiki page covers it
- **When the student asks a question** you are not certain about: verify against the wiki
- **When choosing examples**: wiki pages contain sourced, domain-appropriate examples
- **When calibrating difficulty**: wiki pages carry difficulty_range metadata

### Skip the Wiki

- **For basic conversational flow**: greetings, session management, encouragement
- **For concepts you have already consulted this session**: cache the summary mentally
- **When the student is in flow**: do not pause a productive exchange to look things up

## Querying WikiPages

### Find Pages by Concept

When you know which koto-learn Concept you are teaching:

```
temper.list("WikiPages",
    "$filter=contains(ConceptIds, '{concept_id}') and PersonaId eq '{persona_id}' and State eq 'Published'"
)
```

This returns all published wiki pages that cover the target concept.

### Find Pages by Category

When you want to browse a topic area:

```
temper.list("WikiPages",
    "$filter=Category eq 'grammar' and PersonaId eq '{persona_id}' and State eq 'Published'&$orderby=Title"
)
```

### Find Pages by Tag

When looking for specific content types:

```
temper.list("WikiPages",
    "$filter=contains(Tags, 'conditional') and PersonaId eq '{persona_id}' and State eq 'Published'"
)
```

### Find Pages by Slug

When following a cross-reference from another page:

```
temper.list("WikiPages",
    "$filter=Slug eq '{slug}' and PersonaId eq '{persona_id}' and State eq 'Published'"
)
```

## Reading Content: Summary vs. Full Page

### Use the Summary (most of the time)

The `Summary` field on WikiPage is a 1-3 sentence abstract designed for context injection. Use it when:

- You need a quick refresher on what a page covers
- You are selecting among multiple pages for relevance
- You are building encounter context and need lightweight grounding

Access: The summary is directly on the entity -- no paw-fs read needed.

### Read Full Content from paw-fs (when depth is needed)

The full markdown content lives in paw-fs, referenced by the page's `FileId`. Read it when:

- You need specific examples, conjugation tables, or detailed explanations
- You are building an encounter that directly teaches the content on the page
- The summary is insufficient to answer a student's specific question
- You want to use the page's cross-references to find related topics

```
content = temper.download("Files", page["FileId"])
```

### Decision Framework

| Situation | Read Summary | Read Full Content |
|-----------|:---:|:---:|
| Selecting which concept to teach next | Y | |
| Designing encounter context/narrative | Y | |
| Writing specific teaching content | | Y |
| Answering a student's detailed question | | Y |
| Checking if a concept has wiki coverage | Y | |
| Finding examples for the persona's domain | | Y |
| Verifying cross-references and prerequisites | Y | |

## Following Cross-References

Wiki pages link to each other via `[[slug]]` in markdown and structured `CrossReferences` on the entity.

### Navigating the Knowledge Graph

When designing an encounter, check the page's cross-references to:

- **Find prerequisites**: If the page lists prerequisite pages, verify the student has mastery of those concepts before teaching this one
- **Find related content**: Related pages can provide additional examples or alternative explanations
- **Find contrasts**: Contrast pages highlight distinctions the student needs to understand

```
page = temper.read_entity("WikiPages", page_id)
cross_refs = json.loads(page["CrossReferences"])
for ref in cross_refs:
    if ref["relationship"] == "prerequisite":
        prereq_page = temper.list("WikiPages",
            "$filter=Slug eq '{}' and PersonaId eq '{}'".format(ref["slug"], persona_id)
        )
        # Check if student has mastery of prereq concepts
```

### Using Cross-References for Encounter Sequencing

If the student completes an encounter on topic A, and topic A's wiki page has a `builds_on` reference to topic B, consider designing the next encounter around topic B. The wiki's cross-reference graph can inform your teaching sequence alongside the koto-learn concept prerequisite graph.

## Reporting Wiki Gaps

If you need knowledge that the wiki does not have:

1. **Check if a page exists but is not published**: Query with `State ne 'Archived'` to include Drafting and Revising pages
2. **If no page exists**: Create a WikiSource entity with `source_type = "encounter_insight"` to signal to the Curator that this topic needs coverage:

```
source = temper.create("WikiSources", {})
temper.action("WikiSources", source["entity_id"], "Submit", {
    "persona_id": "{persona_id}",
    "title": "Gap: {topic description}",
    "source_type": "encounter_insight",
    "source_url": "",
    "file_id": "",
    "metadata": json.dumps({
        "origin": "encounter",
        "student_question": "...",
        "concept_ids": ["..."],
        "session_id": "..."
    })
})
```

This creates a signal that the Curator will pick up during its next maintenance or sourcing session. The Curator will search for sources, synthesize a page, and make it available for future encounters.

## Reading the Index

The persona's wiki index is a paw-fs file at `/{persona_id}/index.md`. It provides a curated table of contents:

```
index_files = temper.list("Files",
    "$filter=Name eq 'index.md' and Path eq '/{persona_id}/' and WorkspaceId eq '{koto-wiki-workspace-id}'"
)
if index_files:
    index_content = temper.download("Files", index_files[0]["Id"])
```

The index is useful for:
- Getting an overview of what the wiki covers for this persona
- Finding pages when you do not know the exact concept_id
- Discovering categories and their coverage depth

## Integration with Encounter Design

### Grounding an Encounter

When designing an encounter for concept X:

1. Query WikiPages for concept X
2. Read the summary -- does it give you enough for the encounter context?
3. If yes: use the summary to inform your encounter narrative
4. If no: fetch full content from paw-fs for specific examples and explanations
5. Check cross-references for prerequisite concepts -- has the student mastered them?
6. Set the encounter difficulty within the page's `DifficultyRange`

### When the Wiki Has No Page

If no wiki page covers the concept you want to teach:

1. Rely on your own knowledge (training data) for this encounter
2. Create a WikiSource(encounter_insight) to signal the gap
3. Note in your memory that this concept lacks wiki coverage
4. In future sessions, check again -- the Curator may have filled the gap

## Boundaries

- You CAN read WikiPages, WikiSources, and the index
- You CAN create WikiSource entities with `source_type = "encounter_insight"` to report gaps
- You CANNOT create or modify WikiPages (that is the Curator's job)
- You CANNOT create or modify WikiJobs (the Curator manages its own work queue)
- You CANNOT modify wiki content in paw-fs (read-only access to wiki files)
