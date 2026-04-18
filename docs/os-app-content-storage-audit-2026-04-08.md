# OS App Content Storage Audit

Date: 2026-04-08

## Summary

TemperPaw does **not** have a blanket "all souls are broken" problem.

The core soul and skill paths already follow the correct pattern:

- create a `Files` entity
- upload bytes through `Files('{id}')/$value`
- store only `ContentFileId` on the entity

Temper issue `#106` is now fixed at the platform level, so oversized field values survive via blob-backed overflow refs. That is a safety net, not the preferred app design for document-sized artifacts.

## Classification

| App | Classification | Notes |
|-----|----------------|-------|
| `paw-agent` | Mostly correct | `Soul` and `Skill` are file-backed via `content_file_id`. Conversation/session tree data is also file-backed. `Memory.content` is explicitly documented as small inline content. `Session.result` remains inline and should stay short; if it evolves into report-sized output, migrate it to a file-backed field. |
| `paw-channels` | Correct | Message/session transcript handling already appends file-backed content into the session tree. |
| `paw-compute` | No issue found | No document-sized content fields found in the app schema. |
| `paw-foresight` | Mixed but acceptable | Large artifacts already use file ids: `ModelSnapshotFileId`, `ProjectedStateFileId`, `ArtifactFileId`. Inline fields such as `Observation.Content`, `DirectionFeedback.Response`, and `Direction.Reasoning` are currently review/note-sized. `Direction.Reasoning` is the main watchlist item if agents start storing full reports or long model dumps there. |
| `paw-fs` | Correct | Filesystem app is already the backing primitive for file-based content. |
| `paw-harness` | No issue found | No document-sized content fields found in the app schema. |
| `paw-heal` | No issue found | No document-sized content fields found in the app schema. |
| `paw-ingest` | No issue found | No document-sized content fields found in the app schema. |
| `paw-pm` | Acceptable inline text | `Comment.Body` is normal inline comment text, not a generated artifact. |
| `paw-research` | Correct pattern, one schema drift fixed | The WASM path already writes large fetch results to `result_file_id`. This audit fixes the matching CSDL so `ResultFileId` and `result_file_id` are actually modeled. |

## Concrete Good Patterns

### File-backed content

- `paw-agent` souls and skills:
  - create a file
  - upload content via `/$value`
  - persist `ContentFileId`
- `paw-research` web fetch:
  - stores small fetches inline
  - stores large fetches in `result_file_id`
- `paw-foresight`:
  - stores model snapshots, projected state, and implementation artifacts in file ids

### Safe inline content

These look intentionally bounded and are reasonable to keep inline:

- `paw-agent.Memory.content`
- `paw-pm.Comment.Body`
- `paw-foresight.Observation.Content`
- `paw-foresight.DirectionFeedback.Response`

## Watchlist

These are not immediate bugs, but they should migrate if their payloads become document-sized:

- `paw-agent.Session.result`
- `paw-foresight.Direction.reasoning`
- `paw-foresight.Direction.grounding`
- `paw-foresight.Direction.counterfactual_summary`

## Rule Going Forward

Use inline string fields for:

- titles
- short descriptions
- labels
- comments
- bounded notes
- prompts or summaries that are intentionally short

Use `Files` plus a `*FileId` field for:

- markdown pages
- reports
- compiled analyses
- transcripts
- fetched documents
- HTML or rendered output
- generated JSON artifacts
- long LLM outputs you may need to read back in full

## Follow-up

The next practical enforcement step is a lint or review check that flags suspicious schema fields like `content`, `body`, `report`, `article`, `markdown`, or `results` when they appear on entities whose semantics are document-like.
