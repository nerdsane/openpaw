# ADR-0031: Discord Attachment Support

**Status:** Accepted
**Date:** 2026-04-15
**Related:** ADR-0010 (Slack socket-mode transport)

## Context

When a Discord user sends a file attachment (e.g. a `.md` document) alongside their message, the Paw agent cannot see it. The `MessageCreateData` struct in `paw-transport` has no `attachments` field, so serde silently drops Discord's `attachments` array during deserialization. The entire downstream pipeline — Channel entity spec, `route_message` WASM, Session configuration, and `llm_caller` — never receives any attachment data.

This is a complete gap: no transport in the system handles file attachments. Discord is the immediate pain point because users are sending documents for the agent to analyze and getting no response about the file content.

Discord's MESSAGE_CREATE event includes an `attachments` array with metadata per file: `id`, `filename`, `size`, `url`, `proxy_url`, and `content_type`. The CDN URLs are ephemeral and expire after a period, so any fetch must happen promptly after message receipt.

## Decision

### Transport-layer fetch with content inlining

The Discord transport layer fetches text-type attachment content immediately upon message receipt and inlines it into the `content` string before dispatching `Channel.ReceiveMessage`. This keeps the change contained to 2 files in `crates/paw-transport/src/discord/` with zero downstream modifications.

### Why transport-layer, not WASM-layer

Three alternatives were considered:

1. **Transport-layer fetch + inline into content** (chosen): The transport already has `reqwest::Client`, runs async, and processes messages before dispatching to the entity pipeline. Fetching here avoids CDN URL expiry. Inlining into `content` means no schema changes anywhere downstream.

2. **WASM-layer fetch in route_message**: Would require adding an `attachments` parameter to the Channel spec, passing structured JSON through entity state, and having the WASM module download files via `ctx.http_call()`. More changes, and CDN URLs may expire by the time the WASM module runs.

3. **Structured attachment params through the full pipeline**: Would require changes to `channel.ioa.toml`, `route_message`, `session.ioa.toml`, and `llm_caller`. Correct for multimodal content (images) but over-engineered for text files.

Option 1 was chosen because it solves the immediate problem (text files) with minimal blast radius. Option 3 is the right path for future image support but should be a separate ADR.

### Text-type detection

Attachments are classified as text-type using `content_type` from Discord (preferred) or file extension fallback. Text content types include `text/*`, `application/json`, `application/xml`, `application/toml`, `application/yaml`. Common code file extensions (`.md`, `.txt`, `.rs`, `.py`, `.ts`, `.js`, `.json`, `.toml`, `.yaml`, etc.) are recognized when `content_type` is absent.

### Size limits

Attachments larger than 100KB are skipped to prevent context window bloat. A 100KB text file is roughly 25K-30K tokens, which is significant but manageable within a 200K-token context window.

### Fault tolerance

Download failures for individual attachments are logged as warnings but do not fail the message. The user's text content is always delivered to the agent, even if attachment downloads fail.

### Content format

Attachment content is appended to the message text using a clear delimiter:

```
<original message text>

---
[Attached file: example.md]
<file content>
---
```

This format gives the LLM clear context about the file's origin and name without requiring any changes to how `user_message` is processed downstream.

## Consequences

- Users can send text-based file attachments (`.md`, `.txt`, code files, etc.) on Discord and the agent will see the content immediately.
- Non-text attachments (images, PDFs, archives) are silently ignored. The agent will not know they were sent. This is acceptable for MVP but should be addressed in a follow-up ADR for multimodal support.
- The 100KB size limit means very large text files will be skipped. The agent will not know a large file was sent. A future enhancement could notify the agent that a file was too large to inline.
- The Slack transport does not gain attachment support from this change. The same pattern can be applied there separately.
- No entity schema, WASM module, or LLM caller changes are required, keeping the upgrade path clean for existing deployments.
- Future image/multimodal support will require a separate effort touching `llm_caller` to construct Anthropic image content blocks. This ADR's approach of inlining text does not conflict with that future work.
