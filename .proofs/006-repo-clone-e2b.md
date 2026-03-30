# Proof Report: 006 — Developer Clone on E2B

## Date
2026-03-26

## Branch / Commit
`feat/openpaw-self-heal-loop-codex` @ `26b78156b03fde205e316b5c181fd0fa55ae706a` (verified from a dirty worktree)

## What Was Done
Patched the Open Paw agent loop so a real E2B-backed Developer agent can shallow-clone the private `deep-sci-fi` repository and complete normally.

Implemented fixes:
- Fixed Connect/envd request handling earlier in the turn so E2B process execution works end-to-end.
- Stopped large tool results from being copied into Agent entity state during session-tree runs; the full result stays in TemperFS and the entity now stores a compact marker.
- Injected `github_token` into bash tool execution as `GITHUB_TOKEN` and `GH_TOKEN`, and added an automatic `GIT_ASKPASS` bootstrap so plain HTTPS `git clone` can authenticate inside the sandbox.
- Added `timeout_secs = "120"` to the `run_tools` integration.
- Added `max_sync_files = "64"` plus broader default fsync excludes so repo-sized workspaces do not spend every turn syncing hundreds of files.
- Kept local sandbox parity by allowing explicit environment variables in the local sandbox HTTP helpers.

## Verification Flow
1. Rebuilt `tool_runner`, `llm_caller`, and `openpaw`.
2. Restarted the daemon from this worktree so the updated Agent spec and WASM modules were registered.
3. Created a fresh Agent through the OData API.
4. Configured the Agent with the Developer soul and a single-task prompt: clone `https://github.com/arni-labs/deep-sci-fi.git` into `/home/user/deep-sci-fi`, verify `.git` exists, print `CLONE_OK`, then answer with exactly `CLONE_OK`.
5. Provisioned the Agent so it used a real E2B sandbox.
6. Polled the Agent entity until it reached a terminal state.
7. Retrieved the file manifest from TemperFS to confirm repo files were synced back from the sandbox workspace.

## Verification Results
| Step | Expected | Actual | Status |
|------|----------|--------|--------|
| Daemon restart | New Agent spec + WASM modules loaded from this worktree | `tool_runner` hash `7319ce32f1c433fd944bd95e8d4b76b406c48ff9b08155ba6e4512ef6a76f734` registered; server listening on `http://localhost:3467/tdata` | PASS |
| E2B sandbox provision | Developer agent provisions on E2B | Sandbox `ipdwduuuahq8ol6pmudcp` created | PASS |
| Real repo clone | Agent clones `deep-sci-fi` with bash on E2B | Agent `019d2bbd-8c3a-7b92-947d-c44a064b8b62` reached `Completed` with result `CLONE_OK` | PASS |
| Post-tool LLM turn | Follow-up turn should not fail with oversized invocation context | `pending_tool_calls` field stored compact marker instead of giant payload; agent completed normally | PASS |
| Workspace sync evidence | TemperFS should contain synced repo files | Manifest file `019d2bbd-8d59-76d2-a32f-4e7563c37e7f` contains cloned repo paths including `README.md`, `CLAUDE.md`, and app/backend sources | PASS |

## What Worked
- Real E2B process execution for bash tools.
- GitHub HTTPS cloning with tenant `github_token`.
- Session-tree continuation after tool execution without hitting the SDK context ceiling.
- Limited repo fsync finished within the longer `run_tools` budget.

## What Didn't Work
- A prior attempt failed because `run_tools` still had the default 30-second timeout while cloning and syncing a repo-sized workspace.
- Earlier in the turn, storing raw tool results in `pending_tool_calls` caused `failed to read invocation context` on the next LLM turn.

## Limitations
The current fsync behavior intentionally caps uploads to the first 64 enumerated files and skips common large build directories. This keeps repo-scale tool runs tractable, but it does not guarantee a complete mirror of every cloned file in TemperFS after a single turn.

## What Still Doesn't Work
The full Step 6 self-healing loop is not yet proven here. This report verifies the repo-clone milestone on E2B, not the later Datadog alert -> SRE diagnosis -> fix -> PR flow.

## Artifacts
- Successful agent id: `019d2bbd-8c3a-7b92-947d-c44a064b8b62`
- Successful sandbox id: `ipdwduuuahq8ol6pmudcp`
- Successful result: `CLONE_OK`
- Successful workspace id: `019d2bbd-8d43-7063-9008-db820c1e64ab`
- Successful manifest file id: `019d2bbd-8d59-76d2-a32f-4e7563c37e7f`
- Compact field stored on Agent after tool execution:
  - `pending_tool_calls = "[stored 1 tool result(s) in session tree; 59 bytes retained outside entity state]"`
- Manifest excerpt from `Files('019d2bbd-8d59-76d2-a32f-4e7563c37e7f')/$value`:

```json
{
  "files": {
    "/home/user/deep-sci-fi/CLAUDE.md": {
      "file_id": "wsf-1d845efa5aed816c",
      "size_bytes": 16913
    },
    "/home/user/deep-sci-fi/README.md": {
      "file_id": "wsf-1d8471e49166782c",
      "size_bytes": 4098
    },
    "/home/user/deep-sci-fi/platform/app/page.tsx": {
      "file_id": "wsf-e252986937ca7a2d",
      "size_bytes": 12810
    },
    "/home/user/deep-sci-fi/platform/backend/alembic/versions/0001_initial_schema.py": {
      "file_id": "wsf-59a503ebead86f1f",
      "size_bytes": 47939
    }
  }
}
```

## Architecture Diagram
```text
curl -> Open Paw OData API
     -> Agent.Configure / Agent.Provision
     -> sandbox_provisioner (WASM)
     -> E2B sandbox created
     -> llm_caller (WASM)
     -> tool_runner (WASM, Connect/envd bash, GitHub auth injected)
     -> git clone deep-sci-fi in /home/user/deep-sci-fi
     -> limited fsync -> TemperFS manifest + file entities
     -> HandleToolResults (compact marker on entity, full result in session tree)
     -> llm_caller follow-up turn
     -> FinalizeResult = CLONE_OK
```
