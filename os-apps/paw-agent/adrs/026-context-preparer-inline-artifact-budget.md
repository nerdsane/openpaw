# ADR-026: Raise Prepared Context Inline Budget

- Status: Proposed
- Date: 2026-05-20

## Context

Datadog production traces on `service.version=3c1b32f4301f30d6e01208dd49e03ac087e400c4`
show `Session.HandleToolResults.integrations` dominated by
`wasm:context_preparer`:

- `wasm:context_preparer`: avg about `623 ms`, p50 about `575 ms`, p95 about
  `1113 ms`
- `dispatch.wasm.phase.engine_invoke`: avg about `506 ms`, p95 about `876 ms`
- `dispatch.wasm.phase.host_chain_build`: avg about `88 ms`, p95 about
  `310 ms`

A narrow trace inspection for trace
`4dddabfa615bd6bdba39caf4230d0fe7` showed a concrete cause inside one slow
`HandleToolResults` prepare:

- `GET /tdata/Files(...)/$value` for the existing prepared context artifact:
  about `284 ms`, `response_bytes=39701`
- `PUT /tdata/Files(...)/$value` for the new prepared context artifact:
  about `432 ms`, `request_bytes=45714`

The artifact is only about `45 KiB`, but the default inline budget is `32 KiB`,
so normal tool-result turns cross the threshold and pay internal File IO. This
is not a fundamental Temper constraint. It is a threshold that became too low
after the Session turn artifacts gained richer context and observability.

## Decision

Raise the default `prepared_context_inline_max_bytes` from `32 KiB` to
`128 KiB`.

Keep the existing `prepared_context_inline_max_bytes` field/config override so
operators can lower or raise the budget without another app code change. Keep
external File storage for artifacts above the budget.

Add Datadog-visible metrics for prepared-context artifact storage:

- `temper_session_prepared_context_artifact_bytes`
- `temper_session_prepared_context_artifact_storage_total`

Both metrics carry the existing provider/model tags plus `mode=inline|file`.

## Semantics

The Session state machine does not change. `ContextReady` still carries either
`prepared_context_inline_json` or `prepared_context_file_id`, and
`provider_caller` continues to accept both forms.

Correctness is unchanged because the artifact payload is identical. The change
only selects the transport for medium-sized artifacts.

## Consequences

Positive:

- Normal tool-result turns under `128 KiB` avoid the internal File read/write
  pair on each prepare.
- Datadog can prove the storage-mode mix and artifact byte distribution.
- The change preserves the file-backed path for genuinely large contexts.

Tradeoffs:

- Medium prepared-context artifacts now live in Session fields, increasing
  action/event payload and projection size for those turns.
- Very large contexts still externalize, so this is a bounded hot-path
  improvement rather than an unbounded state-size expansion.

## Verification

- Add a test guard for the new default inline budget and storage metrics.
- Run focused Session turn architecture tests.
- Run formatting checks.
- Live proof after deploy:
  - before window records File `$value` read/write spans inside
    `context_preparer` for artifacts around `40-50 KiB`;
  - after window on the fixed version shows `mode=inline` metrics for the same
    artifact size class and no matching File `$value` spans inside
    `context_preparer`;
  - SessionEntry read-back and terminal Session correctness still pass.

## Rollback

Set `DEFAULT_PREPARED_CONTEXT_INLINE_MAX_BYTES` back to `32 * 1024`, or set
`prepared_context_inline_max_bytes=32768` in app/runtime config.
