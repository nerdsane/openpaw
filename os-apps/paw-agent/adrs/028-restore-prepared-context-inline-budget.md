# ADR-028: Restore Prepared Context Inline Budget

- Status: Proposed
- Date: 2026-05-22
- Supersedes: ADR-026
- Related:
  - ADR-026: Raise Prepared Context Inline Budget
  - ADR-027: Prepared Context Artifact Byte Count
  - `os-apps/paw-agent/wasm/context_preparer/src/lib.rs`

## Context

ADR-026 raised the default prepared-context inline artifact budget from
`32 KiB` to `128 KiB` after traces showed medium artifacts paying expensive
internal File reads and writes. The optimization was locally reasonable, but
Datadog later showed hot production windows where many medium prepared-context
artifacts were kept inline while separate Temper projection reads were already
under memory pressure.

The inline budget did not explain an 8 GiB RSS spike by itself, but it made
Session rows and projection payloads heavier during the incident window. The
safe default should favor bounded memory and projection size. Operators can
still raise the explicit `prepared_context_inline_max_bytes` field for a
controlled experiment after the query/projection paths are proven bounded.

## Decision

Restore the default `DEFAULT_PREPARED_CONTEXT_INLINE_MAX_BYTES` to `32 KiB`.

Keep all ADR-026/ADR-027 metrics:

- `temper_session_prepared_context_artifact_bytes`
- `temper_session_prepared_context_artifact_bytes_total`
- `temper_session_prepared_context_artifact_storage_total`

Keep the existing `prepared_context_inline_max_bytes` override so future tuning
can be staged deliberately instead of requiring another state-machine change.

## Consequences

Positive:

- Default Session projection payloads return to the smaller pre-incident size.
- The File-backed path remains available and already covered by existing
  correctness behavior.
- Datadog still shows the storage-mode mix and artifact byte distribution.

Tradeoffs:

- Some medium tool-result turns may again pay an internal File read/write.
- A future latency optimization must prove memory stability before raising the
  default again.

## Verification

- Update the architecture guard test to require the `32 KiB` default and
  continued artifact metrics.
- Run focused Session turn architecture tests.
- Build the affected WASM module before deployment.
- In production, verify prepared-context inline mode drops for artifacts above
  `32 KiB` and RSS remains stable under hot Session workloads.

## Rollback

Set `prepared_context_inline_max_bytes` explicitly on controlled sessions, or
raise the constant again after projection memory safety and live RSS evidence
prove it is safe.
