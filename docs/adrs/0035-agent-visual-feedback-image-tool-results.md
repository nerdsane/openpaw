# ADR-0035: Agent Visual Feedback via Image Tool Results

## Status

Accepted

## Context

OpenPaw agents are multimodal (gpt-5.4, Claude) and can evaluate images. However, the tool result pipeline is entirely text-based. When a design agent generates an HTML embodiment in its sandbox and takes a Playwright screenshot, the image cannot flow back to the LLM. The agent works blind.

**Root cause**: `sandbox.read()` returns `String`, `HttpResponse.body` is `String`, `make_tool_result()` only accepts `&str`, and all three LLM provider paths (Codex, Anthropic, OpenRouter) format tool results as plain text.

This is a particular problem for Katagami design agents that generate visual embodiments but have no way to evaluate their own output, resulting in lower quality compared to agents that receive visual feedback.

## Decision

Extend the tool pipeline to support image content in tool results. The approach is transparent to agents: `sandbox.read("/tmp/screenshot.png")` automatically detects the image extension, base64-encodes the file inside the sandbox (via `sandbox_exec("base64 -w0 <path>")`), and returns a tagged JSON object that propagates as a multimodal content block through the conversation.

### How it works

1. **dispatch.rs**: When `sandbox.read()` is called with an image extension (.png, .jpg, .jpeg, .gif, .webp), dispatch runs `base64 -w0 <path>` via `sandbox_exec()` instead of `sandbox_file_read()` (which would corrupt binary). Returns `{"__openpaw_image": true, "media_type": "image/png", "base64_data": "...", "source_path": "..."}`.

2. **monty_repl/lib.rs**: Detects `__openpaw_image` tagged results and creates a multimodal tool result with content array `[{"type": "text", ...}, {"type": "image", "source": {"type": "base64", ...}}]` instead of a plain text string. Image data bypasses the 16KB text truncation limit.

3. **llm_caller**: Each provider path formats images per its API spec:
   - **Codex Responses API**: `function_call_output` (text) + `input_image` item (image)
   - **Anthropic Messages API**: Native image blocks in tool results (pass-through)
   - **OpenRouter Chat Completions**: Tool message (text) + user message with `image_url`

4. **Observability**: Image data is replaced with `[image: image/png]` placeholders in Datadog traces. Old tool result images are stripped during context pruning.

### What agents do

No pipeline knowledge required. The workflow is:

```python
sandbox.bash("pip install playwright && playwright install chromium")
sandbox.write("/tmp/page.html", html_content)
sandbox.bash("python3 -c '...playwright screenshot...'")
result = sandbox.read("/tmp/screenshot.png")  # image flows to LLM automatically
# LLM sees the screenshot on next turn, evaluates, iterates
```

## Consequences

- Any agent with a sandbox can now take screenshots and evaluate them visually.
- No new tools needed. `sandbox.read("screenshot.png")` just works.
- Image data bypasses the 32KB entity param limit via existing session tree file storage (`compact_tool_results_marker` stores only tool IDs in entity params).
- Agents are responsible for image size. They control screenshot dimensions via Playwright viewport settings.
- Non-image binary files (.pdf, .zip) still go through `sandbox_file_read()` and get UTF-8 corrupted. Only image extensions get the base64 path.
- The base64 command fallback (`base64 -w0 ... || base64 ...`) handles both Linux coreutils and macOS/BSD sandboxes.

## References

- nerdsane/openpaw#68 (sandbox pre-configuration for pre-installed tools)
- ADR-0022 (lazy sandbox provisioning)
- ADR-0024 (sandbox provider abstraction)
