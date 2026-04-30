# ADR-0048: Production Blob Store Is External

Date: 2026-04-28

## Status

Accepted

## Context

TemperPaw runs on Railway with Turso for metadata and R2 for blob storage.
Startup reconcile traces showed long idle waits in `turso.upsert_wasm_module`
because WASM artifacts were still being written through a SQL-backed blob path.

Temper ADR-0063 moves blob bytes to a Temper-level object-store boundary:
WASM artifacts, field overflow, and TemperFS file content are object-store
objects; SQL stores metadata and refs only.

## Decision

TemperPaw production startup requires `BLOB_ENDPOINT`.

If `BLOB_ENDPOINT` is absent on Railway, startup fails instead of silently using
the internal blob route. Local development may still use the internal route; in
that mode Temper writes to its filesystem object store under the local data
directory, not to Turso.

## Consequences

Railway deployments cannot regress into DB-backed blob storage. Local boot stays
zero-config for development, but local blobs are filesystem objects and should
not be treated as production durability.
