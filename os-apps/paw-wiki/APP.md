# paw-wiki

Reusable LLM Wiki pattern -- scope-scoped knowledge plane for storing synthesized, interlinked knowledge articles.

## Entity Types

- **WikiSource** -- Raw material submitted for processing. Lifecycle: `Submitted -> Indexed | Failed`
- **WikiPage** -- Synthesized, versioned knowledge article. Lifecycle: `Drafting -> Published <-> Revising -> Archived`
- **WikiJob** -- Task queue entry for wiki agent sessions. Lifecycle: `Queued -> Ready -> Running -> Completed | Failed`

## WASM Integrations

- **build_session_message** -- On WikiJob.Submit: constructs a domain-specific prompt from job fields, creates a Session entity, dispatches Configure on it, then dispatches SessionSpawned back on the WikiJob. Supports built-in templates for `source_search` and `synthesize` job types, or custom `mission_template` from the job's input JSON.
- **finalize_spawned_session** -- On WikiJob.Complete/Fail: finalizes the spawned Session (records result or fails it). On `source_search` completion, automatically spawns a follow-up `synthesize` WikiJob.

## Configurability

Consuming apps override behavior via `[integration.config]` in their own `.ioa.toml`:

- `odata_namespace` -- OData action namespace for URL construction (default: `WikiCore`)
- `temper_api_url` -- Temper API endpoint (default: resolved from secret)

Custom job types are supported by providing a `mission_template` key in the WikiJob's `input` JSON field. The shared operating model (workspace rules, Monty REPL, tooling constraints) is always prepended; the template provides the domain-specific mission.

## Dependencies

- `paw-agent` -- Session entity for agent spawning
- `paw-fs` -- File storage for wiki content
