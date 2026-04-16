# Run 002 Changelog

## Changed File
`os-apps/paw-foresight/system/skills/orchestrate-projection/SKILL.md`

## What Changed

### 1. Added web search tools to probe sessions

**Before:**
```
"tools_enabled": "temper_get,temper_list,temper_action,temper_create,temper_read"
```

**After:**
```
"tools_enabled": "temper_get,temper_list,temper_action,temper_create,temper_read,temper_web_search,temper_web_fetch"
```

### 2. Added web search step to probe instructions

Inserted a new step 2 ("SEARCH FOR EXTERNAL EVIDENCE") between reading the knowledge graph (step 1) and projecting forward (now step 3). This step requires probes to:
- Run at least 2 `temper.web_search()` queries for recent signals not in the KG
- Use `temper.web_fetch()` to read promising results
- Look for news, announcements, research papers, or industry signals
- Cite external sources in their observations

Renumbered subsequent steps (3→4, 4→5, 5→6).

### 3. Added external evidence rule to probe Rules section

Added a new rule requiring at least 2 observations to cite external evidence found via web search, with source URL or title.

## Rationale

Run 001 diagnosis identified that 8 criteria are tied at the competent median (2.0) because probes only have access to the knowledge graph. The highest-leverage targets (Novelty, Challenge, Grounding, Plausibility) all require evidence from OUTSIDE the input — which probes cannot currently discover. Adding web search is a structural change (tool access) that enables probes to bring real-world signals into their observations.
