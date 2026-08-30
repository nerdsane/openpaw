# ADR-004: Contributor-owned image editing

## Status

Accepted

## Date

2026-07-25

## Context

TemperPaw agents can act as contributors to external collections such as
Katagami. Those agents need a governed image-to-image operation to make and
evaluate their own visual submissions. The collection itself must not become a
central image-generation service: outside contributors bring their own proof
images, while a TemperPaw contributor may choose to create proofs with its own
PawMedia tools.

The existing PawMedia app supports text-to-image generation through the Codex
subscription route. It does not accept a source image and cannot perform the
same edit with two image models. A provider integration embedded in Katagami
would put provider credentials, billing, orchestration, and contributor
submission policy in the wrong service.

## Decision

PawMedia owns a second, explicit operation: `temper.image_edit`.

The operation creates the existing `MediaGenerationRequest`, then dispatches an
`Edit` action with:

- one exact prompt;
- one PawFS source file and, when available, its immutable version id;
- `provider = "fal"`;
- one allow-listed FAL edit model;
- mechanical output controls only.

The first provider-family module is `fal_image_edit`. It supports
`openai/gpt-image-2/edit` and `fal-ai/nano-banana-2/edit`. The module reads the
source from PawFS, sends it as a base64 data URI, stores the result back in
PawFS, and records source, prompt, result, model, and provider-request
provenance. The prompt text is not rewritten or augmented by PawMedia.

`Generate` and the existing Codex subscription path remain unchanged. `Edit`
has its own action and provider module so provider selection is visible in the
state machine and a non-Codex request never enters the Codex auth flow.

Katagami is not a PawMedia caller. Katagami accepts proof files and provenance
from contributors and validates them. A TemperPaw agent may voluntarily use
PawMedia before submitting; an outside contributor uses their own tools.

## Consequences

- Image-generation spend belongs to the TemperPaw caller that requested it.
- Katagami does not hold FAL credentials or silently generate images for
  submissions.
- Both edit models receive byte-for-byte identical aesthetic prompt text.
- Provider mechanics remain app-scoped and auditable through entity
  transitions.
- The source image is an edit input, not a style-reference dependency.
- Additional providers require their own provider-family module and explicit
  action routing; arbitrary endpoint strings are rejected.
