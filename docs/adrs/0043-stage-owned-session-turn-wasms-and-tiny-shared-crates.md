# ADR-0043: Stage-Owned Session Turn WASMs and Tiny Shared Crates

- Status: Accepted
- Date: 2026-04-24
- Extends: ADR-0040

## Context

ADR-0040 removed the legacy `llm_caller` turn path and made the staged
Session-turn WASMs authoritative:

- `context_preparer`
- `provider_caller`
- `provider_response_applier`

That cutover removed the old orchestration boundary, but the staged crates still
carried a large amount of copied code:

- duplicate tool catalog constants and alias normalization
- duplicate session-turn artifact structs and param builders
- duplicate `gen_ai.*` payload builders
- foreign helper code from neighboring stages
- stale tests copied forward from the monolithic module

This left the runtime path correct but the code ownership blurry. The staged
WASMs were authoritative in the spec while still behaving like partial copies of
the old giant crate internally.

## Decision

We make the staged Session-turn crates own exactly one stage each, and we only
permit tiny shared Rust crates for pure shared data/helpers.

### Stage ownership

- `context_preparer` owns `PreparingContext`
- `provider_caller` owns `CallingProvider`
- `provider_response_applier` owns `ApplyingProviderResponse`

Each staged crate must expose only its own `run_*` entrypoint and must not
carry public entrypoints for any other stage.

### Allowed shared crates

Shared Rust crates are allowed only when they are:

- pure data/model definitions
- pure helper functions
- hot-load-insensitive support code

For this cut:

- `tool-catalog` is the single source of truth for tool aliases, defaults, and
  prompt-visible method listing
- `session-turn-artifacts` owns the shared prepared/provider artifact structs
  and `gen_ai.*` param builders

These crates are support code only. They are not orchestration layers and they
do not replace the spec-owned WASM stage boundaries.

### Disallowed structure

The staged WASMs must not locally define:

- shared tool catalog constants or alias normalization
- shared session-turn artifact structs
- shared provider-response param builders
- shared `gen_ai.*` payload builders
- foreign stage entrypoints
- large foreign-stage helper blocks copied from neighboring WASMs

## Consequences

### Positive

- stage ownership is visible in both the spec and the code
- hot-loadable behavior still lives in standalone WASMs
- prompt and executor tool surfaces share one catalog
- provider response observability payloads share one implementation
- dead copied code is removed instead of hidden under wrapper crates

### Tradeoffs

- some small pure support code now lives in `rlib` crates
- staged WASMs still duplicate a little runtime plumbing where that is simpler
  than introducing a heavier shared layer

This is acceptable because the duplicated remainder is local runtime glue, while
the removed duplication was the part most likely to drift across stage
boundaries.

## Guardrails

The staged cutover structural test must fail if any staged crate reintroduces:

- foreign `run_*` entrypoints
- local shared tool catalog definitions
- local shared artifact or `gen_ai.*` builders
- obvious foreign-stage helper blocks

This keeps the cleanup enforceable as the turn pipeline evolves.
