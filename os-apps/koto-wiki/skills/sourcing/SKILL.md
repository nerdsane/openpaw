# Skill: Sourcing -- Web Research and Content Acquisition

## Purpose

This skill governs how the Curator finds, evaluates, downloads, and stores source material that feeds the wiki. Sources are the raw evidence from which wiki pages are synthesized. The quality of the wiki is bounded by the quality of its sources.

## Core Principle

**Primary sources over training data.** The Curator's value is that it goes out and finds real content -- authoritative grammar guides, corpus frequency data, native-speaker explanations -- rather than regurgitating what the LLM already knows. Training data is the fallback, not the default.

## Web Search Strategy

### Query Formulation

Build search queries that target authoritative, example-rich content:

- **Be specific**: "JLPT N4 conditional forms tara ba comparison examples" over "Japanese grammar"
- **Target known authorities**: Include site names when quality is known -- "Tae Kim grammar guide conditional"
- **Use domain terms**: "Japanese particle wa vs ga contrastive topic" over "difference between wa and ga"
- **Try multiple angles**: If the first query yields thin results, reformulate. Try English queries first, then Japanese queries for corpus data.
- **Include level markers**: "JLPT N4" or "intermediate" to filter by difficulty

### Search Budget

- **Per job**: 3-8 searches maximum. Stop when you have 3+ quality sources for the target category.
- **Per query**: Evaluate the top 5-8 results. Skip results from known low-quality domains.
- **Diminishing returns**: If the third search on the same topic yields nothing new, move on.

### Source Evaluation

Before fetching a URL, assess it from the search snippet:

| Signal | Accept | Reject |
|--------|--------|--------|
| Domain | Educational (.edu), established grammar sites, corpus projects | Content farms, auto-generated sites |
| Title | Specific topic + "guide", "explanation", "examples" | "Top 10", "easy", clickbait patterns |
| Snippet | Contains Japanese examples, grammatical terms | Generic descriptions, no examples visible |
| Author | Named author with language credentials or teaching experience | Anonymous, no attribution |

### Known Quality Sources (not exhaustive)

- **Tae Kim's Japanese Grammar Guide** -- comprehensive, example-rich, free
- **Imabi** -- detailed, academic-leaning, thorough on nuance
- **Maggie Sensei** -- conversational, good examples, informal register
- **JLPT Sensei** -- organized by level, test-focused
- **Bunpro** -- structured grammar points (may be paywalled)
- **Japanese Stack Exchange** -- community answers, variable quality but good for edge cases
- **BCCWJ / NINJAL corpus data** -- frequency data, authentic usage
- **Genki / Minna no Nihongo references** -- textbook standard, widely cited

## Content Extraction

### Parsing HTML

When `temper.web_fetch()` returns HTML content:

1. Extract the main content area (skip nav, sidebar, footer, ads)
2. Preserve Japanese text exactly -- do not transliterate or simplify
3. Preserve example sentences with their translations/explanations
4. Preserve tables (grammar conjugation tables are high-value)
5. Convert to clean markdown for storage
6. Note the fetch date in metadata

### Parsing CSV / Structured Data

Frequency lists, vocabulary databases, and corpus exports:

1. Identify the column structure (word, reading, meaning, frequency, JLPT level)
2. Store as-is in paw-fs (CSV format preserved)
3. Note the schema in the WikiSource metadata field
4. During synthesis, the Curator reads and interprets the data

### Content Too Large

If a fetched page exceeds what can reasonably be stored:

1. Extract the relevant section only
2. Note in metadata that this is a partial extraction
3. Store the full URL for reference

## Storing Source Content

After `temper.web_fetch(url)`, immediately write the content to paw-fs and create the WikiSource entity. Always set `file_id` from the write result and `source_url` from the fetched URL.

### Python Patterns

```python
results = temper.web_search('query')
for i in range(min(3, len(results))):
    r = results[i]
    content = temper.web_fetch(r['url'])
    f = temper.write('/wiki/sources/' + slug + '.md', content)
    s = temper.create('WikiSources', {})
    temper.action('WikiSources', s['entity_id'], 'Submit', {
        'persona_id': 'light-novel-reader',
        'title': r['title'],
        'source_type': 'web_article',
        'source_url': r['url'],
        'file_id': f['file_id'],
        'metadata': '{}'
    })
```

### Python Sandbox Rules

The execution environment is Monty (a restricted Python interpreter), NOT standard Python.

- Do NOT use `import` statements (no `import json`, `from typing`, etc.)
- Do NOT use `enumerate(x, start=N)` -- use `range(len(x))` instead
- Do NOT use f-strings with nested quotes
- `temper` and `sandbox` objects are pre-loaded -- all calls are synchronous
- All data from temper calls is already parsed (dicts/lists) -- no json.loads() needed

## Training Data Fallback

Use training data (parametric LLM knowledge) only when:

- Web search yields no results for a specific topic after 3+ query attempts
- The topic is well-established and unlikely to be wrong (e.g., basic kana charts)
- The content is structural, not factual (e.g., how to organize a page, not what a grammar rule means)

When using training data, mark it explicitly in the WikiSource:
- `source_type = "training_data"`
- `source_url = ""` (empty)
- `metadata = {"provenance": "LLM training data", "confidence": "high|medium|low"}`

The synthesized WikiPage must note unsourced claims: "Based on general linguistic knowledge (not sourced from a specific document in this wiki)."

## Error Handling

- **Search returns no results**: Reformulate query twice. If still empty, note the gap and move on.
- **Fetch fails (404, timeout, paywall)**: Log the URL and error, skip this source, continue with others.
- **Content is in a language you cannot parse**: Note in metadata, skip extraction, store URL only.
- **Duplicate source**: If the same URL has already been stored as a WikiSource, skip it. Check with `temper.list("WikiSources", "$filter=SourceUrl eq '{url}'")`.
