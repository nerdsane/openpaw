# Proof: PawFS Context Read Plane Live E2E

Date: 2026-04-24
Worktree: `/Users/seshendranalla/Development/openpaw-worktrees/pawfs-context-read-plane`
Server: `temperpaw-server` on `http://127.0.0.1:3474`

## Goal

Prove the cross-repo architecture works on a real local server:

- file writes create explicit `FileVersion` lineage
- immutable batch reads return historical version content correctly
- a live Session completes end to end with the new context-prep and file-read path
- OpenAI provider wiring no longer misroutes to Anthropic secrets

## Environment

Server command:

```bash
PORT=3474 \
PUBLIC_BASE_URL=http://127.0.0.1:3474 \
OTEL_ENABLED=false \
TEMPER_API_KEY=live-e2e-key \
TEMPERPAW_WASM_STARTUP_POLICY=build \
PAW_TENANT=default \
cargo run -p temperpaw --bin temperpaw-server
```

Health check:

```bash
curl -fsS http://127.0.0.1:3474/healthz
```

Result:

- healthy

## Live proof 1: immutable file-version lineage

Created a file, wrote it twice, then queried both the mutable file head and the immutable version chain.

Observed:

- `version_count=2`
- latest `FileVersion.status=Current`
- previous `FileVersion.status=Superseded`
- `GET /tdata/Files('<id>')/$value` returned `second version from live e2e`

Immutable batch read call:

```bash
curl -fsS \
  -H "Authorization: Bearer live-e2e-key" \
  -H "x-tenant-id: default" \
  -H "x-temper-principal-kind: admin" \
  -H "content-type: application/json" \
  -d '{"file_version_ids":["019dbe60-14ed-78b2-bc40-88cd2eeb5925","019dbe60-153c-7f50-b264-1334a6d1ee49"]}' \
  http://127.0.0.1:3474/api/files/read-version-text-batch
```

Result:

- first item text: `first version from live e2e`
- second item text: `second version from live e2e`

This proves the live server is reading immutable historical content, not just the current file head.

## Live proof 2: Session completes end to end

Created and configured a live Session with:

- `provider=mock`
- `model=mock`
- `user_message="Reply with exactly: live session ok"`
- `max_turns=1`

Final entity state:

- `status=Completed`
- `result=Reply with exactly: live session ok`

Observed state transitions:

1. `Created`
2. `Provisioning`
3. `PreparingContext`
4. `CallingProvider`
5. `ApplyingProviderResponse`
6. `Steering`
7. `Completed`

Important live fields on the finished Session:

- `prepared_context_entries_loaded=1`
- `prepared_context_content_files_loaded=1`
- `prepared_context_file_id` set
- `provider_response_file_id` set
- `session_leaf_id=a-2`

This proves the updated context preparation path runs on the live server and the full Session lifecycle still completes.

## Live proof 3: OpenAI miswire fixed

Before this change, a live Session configured with `provider=openai` failed after `ContextReady` with:

```text
provider=openai api key is unresolved secret template: '{secret:anthropic_api_key}'
```

After the fix, the same live check fails with:

```text
provider=openai api key is unresolved secret template: '{secret:openai_api_key}'
```

That is the correct failure mode for this machine, because no OpenAI secret is configured locally. The important part is that OpenAI no longer inherits Anthropic secret wiring.

## What this proves

- `FileVersion` lineage is live, not just present in specs/tests.
- batch immutable reads work against the real local server.
- Session context preparation still functions with the new file-version + batch-read architecture.
- provider wiring is now provider-specific for OpenAI vs Anthropic vs Codex.

## Remaining external limitation

This machine does not currently have a live `openai_api_key` configured for the local server, so a real OpenAI completion could not be proven here. The server-side wiring is fixed and the failure mode is now correct and explicit.
