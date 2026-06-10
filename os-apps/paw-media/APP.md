# Paw Media

Paw Media provides governed media generation as Temper-native entity flows.

## Entities

- `MediaGeneration`: request/result state for media generation. Version 1 supports `media_type = "image"`, `operation = "generate"`, and `provider = "openai_codex"`.

## Agent Tool

Agents call `temper.image_generate(prompt, opts=None)`. The tool creates a `MediaGeneration`, dispatches `Generate`, waits for the WASM provider module, then returns PawFS file metadata plus a short-lived inline image marker for immediate multimodal feedback.

Durable bytes are stored through PawFS `File` streams. Inline base64 is only a transient tool-result convenience and uses the spec overflow TTL.

The default Codex image quality is `low` for the current buffered WASM HTTP path, keeping the streamed response under the Temper host response buffer while still producing a durable PNG. Callers may pass a higher `quality` once the provider module moves to host-level response streaming.
