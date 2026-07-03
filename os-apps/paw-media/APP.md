# Paw Media

Paw Media provides governed media generation as Temper-native entity flows.

## Entities

- `MediaGenerationRequest`: request/result state for media generation. Version 1 supports `media_type = "image"`, `operation = "generate"`, and `provider = "openai_codex"`.

## Agent Tool

Agents call `temper.image_generate(prompt, opts=None)`. The tool creates a `MediaGenerationRequest`, dispatches `Generate`, waits for the WASM provider module, then returns PawFS file metadata plus a short-lived inline image marker for immediate multimodal feedback.

Durable bytes are stored through PawFS `File` streams. Inline base64 is only a transient tool-result convenience and uses the spec overflow TTL.

The Codex provider reads generated image responses through the Temper WASM streaming HTTP host API, so provider responses are not constrained by the fixed non-streaming SDK response buffer. The default quality remains `low` to keep DM image generation fast and inexpensive unless callers request a higher quality.
