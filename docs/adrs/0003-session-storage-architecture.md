# ADR-0003: Session Storage Architecture — Content-Per-File

## Status

Accepted

## Context

The session tree was a single JSONL file where every turn's content (user messages, assistant responses, tool results) was stored inline. For file-heavy developer work (reading source files, command outputs), the session tree grew to 180KB+ and blob reads failed.

The root cause is architectural: storing content inline in a structural manifest violates separation of concerns and creates unbounded growth.

## Decision

**Every piece of conversation content is stored as a separate TemperFS File entity. The session tree is a pure structural manifest of references.**

### Session tree format (JSONL)
```json
{"id":"h-1", "parentId":null, "type":"header", "version":2}
{"id":"t-1", "parentId":"h-1", "type":"message", "role":"user", "content_file_id":"file-001", "tokens":15}
{"id":"t-2", "parentId":"t-1", "type":"message", "role":"assistant", "content_file_id":"file-002", "tokens":20, "tool_calls":[...]}
{"id":"t-3", "parentId":"t-2", "type":"tool_result", "content_file_id":"file-003", "tokens":5000}
{"id":"c-1", "parentId":"t-3", "type":"compaction_summary", "content_file_id":"file-010", "summarizes":["t-1","t-2","t-3"]}
```

### Rules
1. **No content inline.** Every `content` field is replaced with `content_file_id` pointing to a TemperFS File.
2. **Files are immutable.** Content Files are never modified or deleted after creation.
3. **Compaction creates new Files.** A compaction summary is a new File. Original content Files persist for audit/recovery.
4. **Session tree stays small.** Only references, roles, tokens, and parent IDs. Never exceeds a few KB regardless of conversation length.

### LLM context assembly
1. Read session tree (one small read)
2. For recent turns (configurable window): fetch content from `content_file_id` Files
3. For older turns: use compaction summary File, or skip
4. Assemble messages array for Anthropic API

### Compaction
- Triggered by turn count or context token budget
- Creates a summary File from old turns' content
- Adds a `compaction_summary` entry to session tree
- Does NOT delete or modify old content Files
- LLM context uses summary for compacted region, full content for recent turns

## Consequences

### Positive
- Session tree never grows large (bounded by number of turns * ~100 bytes per entry)
- No blob read failures regardless of conversation length
- Full conversation history always recoverable (all Files persist)
- Clean separation: structure (session tree) vs content (Files)
- Natural garbage collection boundary (Files can be archived independently)

### Negative
- More TemperFS File entities created (one per turn)
- LLM context assembly requires N+1 reads (1 session tree + N content files for recent turns)
- Compaction logic is more complex (must create summary Files)

### Risks
- N+1 reads could be slow for very long conversations — mitigate with parallel reads
- File creation overhead per turn — mitigate with batching or async creation
