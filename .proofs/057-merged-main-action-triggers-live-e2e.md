# Proof Report: 057 — Merged-Main Action Trigger Live E2E

Date: 2026-04-24
OpenPaw repo: `/Users/seshendranalla/Development/openpaw-action-triggers`
OpenPaw branch: `feat/action-triggers-migration`
Paired Temper repo: `/Users/seshendranalla/Development/temper-action-triggers`
Paired Temper branch: `feat/action-triggers-unified`
Paired Katagami repo: `/Users/seshendranalla/Development/katagami-action-triggers`
Paired Katagami branch: `feat/action-triggers-migration`

## Scope

- Verify OpenPaw after merging latest upstream `main`.
- Run against the local merged Temper worktree, not GitHub `main`.
- Re-prove `paw-fs` live lineage, Session runtime health, provider-secret wiring, and Katagami prompt-boundary behavior.

## Local Integration Wiring

For this proof only:

- Added ignored `.cargo/config.toml` patch overrides so OpenPaw resolved Temper crates from `/Users/seshendranalla/Development/temper-action-triggers`.
- Temporarily repointed `os-apps/katagami-curation` and `os-apps/katagami-commons` symlinks to `/Users/seshendranalla/Development/katagami-action-triggers/...`.
- Reverted the symlink override after the proof run so no local path change remains staged in the branch.

## Built Artifacts

Rebuilt the live WASM artifacts used by the server:

```bash
bash os-apps/paw-fs/wasm/blob_adapter/build.sh
bash os-apps/paw-fs/wasm/workspace_fs/build.sh
bash os-apps/paw-agent/wasm/build.sh
bash /Users/seshendranalla/Development/katagami-action-triggers/katagami-curation/wasm/build.sh
```

## Server

Started a fresh OpenPaw server:

```bash
HOME=/tmp/openpaw-merge-e2e-home \
PORT=4477 \
PUBLIC_BASE_URL=http://127.0.0.1:4477 \
OTEL_ENABLED=false \
TEMPER_API_KEY=live-e2e-key \
TEMPERPAW_WASM_STARTUP_POLICY=build \
PAW_TENANT=default \
TURSO_URL=file:/tmp/openpaw-merge-e2e.db \
RUST_LOG=info \
./target/debug/temperpaw-server
```

Health:

```bash
curl -fsS http://127.0.0.1:4477/healthz
```

Result: healthy

## Live Proof 1: PawFS Lineage And Immutable Reads

Executed:

- `POST /tdata/Files`
- two `PUT /tdata/Files('<id>')/$value` writes
- `GET /tdata/Files('<id>')/$value`
- `POST /api/files/read-version-text-batch`

Observed:

```json
{
  "file_id": "fl-019dbff7-0892-7683-b4fe-b853ffe32ee6",
  "version_count": 2,
  "last_version_id": "019dbff7-0bb9-7103-bba1-2762494b5583",
  "previous_version_status": "Superseded",
  "current_version_status": "Current",
  "latest_text": "second version from openpaw merge proof",
  "batch_texts": [
    "first version from openpaw merge proof",
    "second version from openpaw merge proof"
  ]
}
```

## Live Proof 2: Session Completes End To End

Created a Session and dispatched:

```json
{
  "provider": "mock",
  "model": "mock",
  "user_message": "Reply with exactly: live session ok",
  "max_turns": 1,
  "tools_enabled": false
}
```

Observed final state:

```json
{
  "session_id": "ss-019dbff7-0c00-7ba0-bfb5-f60788865780",
  "status": "Completed",
  "result": "Reply with exactly: live session ok",
  "prepared_context_entries_loaded": 1,
  "prepared_context_content_files_loaded": 1,
  "prepared_context_file_id": "fl-019dbff7-1173-71d2-a57e-f9f43d6237b3",
  "provider_response_file_id": "fl-019dbff7-1431-7fd2-a645-fa3c5cb9de15",
  "session_leaf_id": "a-2",
  "events": [
    "Created",
    "Configure",
    "ProvisionWorkspace",
    "WorkspaceReady",
    "ContextReady",
    "Heartbeat",
    "ProgressMade",
    "ProgressMade",
    "ProviderResponseReady",
    "CheckSteering",
    "FinalizeResult"
  ]
}
```

## Live Proof 3: Provider-Specific OpenAI Secret Wiring

Created a second Session with:

```json
{
  "provider": "openai",
  "model": "gpt-5.2",
  "user_message": "Say hello",
  "max_turns": 1,
  "tools_enabled": false
}
```

Observed:

```json
{
  "status": "Failed",
  "error_message": "provider=openai api key is unresolved secret template: '{secret:openai_api_key}'. set tenant secret and retry"
}
```

This is the correct post-fix failure mode. OpenAI no longer inherits Anthropic secret wiring.

## Live Proof 4: Katagami Quality-Review Prompt Boundary

Installed `katagami-curation` on the same OpenPaw server:

```bash
curl -fsS \
  -H 'Authorization: Bearer live-e2e-key' \
  -H 'content-type: application/json' \
  -d '{"tenant":"default"}' \
  http://127.0.0.1:4477/api/os-apps/katagami-curation/install
```

Created a `CurationJob`, configured it for `quality_review` with `provider=mock`, then dispatched `Katagami.Curation.Submit`.

Observed:

```json
{
  "job_id": "en-019dbff7-e4e3-7192-941d-e193736fe5a8",
  "job_status": "Running",
  "session_id": "ss-019dbff7-e64f-7f91-8e03-f607ec33fdb1",
  "session_status": "Provisioning",
  "contains_review_boundary": true,
  "contains_fail_fast_line": true,
  "contains_regenerate_redirect": true,
  "contains_synthesize_redirect": true
}
```

Prompt excerpt from the spawned Session:

```text
## Review Boundary

This job is a QUALITY REVIEW, not a research-and-rewrite spec synthesis pass.

1. Validate the spec sections before reviewing the embodiment.
2. If the spec is incomplete, empty, or skeleton-quality, STOP.
3. Do NOT repair the spec inside this quality_review job.
4. Fail the job with a concrete error_message explaining which sections are invalid and instruct the caller to run `regenerate_embodiment` or `synthesize` first.
```

Cleanup:

- Dispatched `Katagami.Curation.Fail`
- Job moved cleanly to `Failed`

## What This Proves

- OpenPaw is green live after folding in latest `main`.
- The live server used the merged local Temper worktree, not remote GitHub `main`.
- `paw-fs` lineage and immutable batch reads still work on the integrated stack.
- The bounded Session pipeline still completes on the live server.
- OpenAI provider wiring now fails against the correct secret key.
- The Katagami prompt boundary is enforced on the current worktree through the OpenPaw server.
