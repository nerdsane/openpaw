# ADR-024: Context Preparer Prompt Read Batching

## Status

Proposed for PERF-025.

## Context

After PERF-024, the fixed production samples on
`5342702975d1f79cbdbd9687b160e5703e0ecbbb` show that first-turn Session latency
is no longer explained by one missing projection read-back optimization.
Datadog trace `506355a6e9104036022a440cece4fb9e` and the matching after-trace
aggregate show these hot spans:

- `wasm:provider_response_applier`: about `386 ms`
- `wasm:context_preparer`: about `237 ms`
- `wasm:provider_caller`: about `177 ms`
- `wasm:emit_ots_trajectory`: about `177 ms`
- `wasm:workspace_provisioner`: about `164 ms`

Inside `context_preparer`, the live first-turn proof logged:

- `load_messages`: about `12 ms`
- `assemble_system_prompt`: about `74 ms`
- `write_prepared_artifact`: about `0 ms`

The trace also shows several independent prompt metadata reads during the
system-prompt assembly window: project harness lookup, system/project/agent
skill-index lookups, and memory lookup. These reads are independent, but the
guest currently issues them serially. The individual OData spans are small
single-digit to low-teens milliseconds, yet serializing them burns enough
first-turn wall time to be worth removing before considering riskier
SessionEntry materialization changes.

## Decision

Batch the independent prompt metadata reads in `context_preparer`:

1. Build one ordered `ctx.http_call_batch` request set for:
   - `Harnesses('<project_harness_id>')`, when a project harness is configured
   - each enabled skill index prefix (`/system/skills/`, project skills, agent
     skills)
   - active Memories for the current context key
2. Parse responses in memory and render the same prompt blocks in the same
   order as before.
3. Preserve the existing behavior for optional sections:
   - non-200 harness responses log and produce no harness block
   - non-200 skill prefix responses log and are skipped
   - non-200 memory responses produce no memory block
4. If the batch host call itself fails, fall back to the existing serial reads.

This does not change prompt content policy, skill precedence, Cedar access, or
any Session transition. It only changes the request shape used to gather
already-optional prompt metadata.

## Correctness Rules

1. Prompt blocks must stay in the existing semantic order: soul, agent
   instructions, explicit override, project harness, skills, plan-mode or active
   plan, memory, SDK reference.
2. Skill precedence remains agent > project > system.
3. `skills_prompt_mode=index` must still advertise only skill names, paths, and
   workspace IDs; it must not inject skill bodies.
4. `skills_prompt_mode=full/body/bodies/legacy` may still perform follow-up
   file reads after the index batch, because body reads depend on the index
   result.
5. A batch-host failure must not fail the Session turn. It must degrade to the
   previous serial read behavior.

## Observability And Verification

Before evidence:

- Trace `506355a6e9104036022a440cece4fb9e`
- Fixed-version aggregate over traces `506355...`, `8edfbe...`, and
  `85d1eb...`
- `wasm:context_preparer` avg about `237.5 ms`
- `Session.WorkspaceReady.integrations` avg about `237.7 ms`
- `assemble_system_prompt` logged about `74 ms` in the inspected trace

Acceptance requires:

- local tests covering the batched skill/prompt helper behavior
- `session_turn_architecture` asserting the active context preparer owns the
  prompt-read batch contract
- release WASM build for `context_preparer`
- PR, merge, Docker, Railway deploy
- live e2e Session proof with exact output token and SessionEntry correctness
- after Datadog trace/window showing `context_preparer` and
  `Session.WorkspaceReady.integrations` before/after timing

## Rollback

Revert the `context_preparer` helper changes. The rollback restores the serial
prompt metadata reads while keeping all earlier SessionEntry and projection
fixes intact.
