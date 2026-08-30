# Contributor-owned art-style proofs

## Objective

Add a generic PawMedia image-edit operation for TemperPaw contributors while
keeping Katagami submissions contributor-owned. Katagami may validate submitted
proofs, but it must never invoke PawMedia or spend provider credits for an
outside contributor.

## Plan

1. Record the PawMedia boundary and provenance contract in an app ADR.
2. Add a failing contract test for the `temper.image_edit` surface.
3. Extend `MediaGenerationRequest` with a governed FAL edit action and immutable
   source/result provenance.
4. Add the provider-family WASM and agent-facing tool.
5. Build, test, and exercise the flow locally with a real image.
6. Publish PawMedia through the normal TemperPaw/Genesis path and verify the
   installed ref.

## Acceptance criteria

- `temper.image_edit` accepts one prompt and one PawFS source image.
- The prompt is forwarded unchanged to either supported FAL edit model.
- Source and result file identifiers, immutable version identifiers, byte
  digests, provider model, and provider request id are retained.
- FAL credentials stay inside PawMedia.
- No Katagami app or MCP automatically invokes PawMedia.
- Existing `temper.image_generate` behavior remains unchanged.
