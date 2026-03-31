# Proof Report: 012 — Gap Closure Rerun

## Date
2026-03-27

## Branch
Integration worktree based on `feat/openpaw-self-heal-loop-codex`

## Purpose
This report supersedes the earlier partial status check and records a fresh rerun after:

- fixing the upstream Temper HTTP-vs-Connect framing bug
- switching Open Paw to consume that Temper branch
- extending channel continuation to use file-backed session content
- re-running build, channel continuation, E2B execution, self-heal, and restart recovery

---

## What Changed Since 011

### 1. Upstream Temper framing bug was fully corrected
The real bug was not just the `Content-Type` header. The upstream Temper host implementation was framing ordinary HTTP requests as Connect payloads and leaving Connect requests unframed.

Fixed in `/Users/seshendranalla/Development/temper` on branch `feat/temper-claw`:

- `a530a0e` `fix: envelope Connect JSON requests for envd`
- `a6e8c00` `fix: send raw HTTP bodies and framed Connect payloads`

Open Paw now builds against Temper commit `a6e8c00`.

### 2. Channel continuation now externalizes resumed user turns
`route_message` now writes resumed user messages through TemperFS content files when it can resolve the session workspace, instead of forcing those turns inline into `session.jsonl`.

Changed files:

- [route_message/src/lib.rs](/Users/seshendranalla/Development/openpaw-codex-integration-20260327000903/os-apps/paw-channels/wasm/route_message/src/lib.rs)
- [route_message/Cargo.toml](/Users/seshendranalla/Development/openpaw-codex-integration-20260327000903/os-apps/paw-channels/wasm/route_message/Cargo.toml)

### 3. Paw soul got concrete workflow examples
`Paw` now includes concrete `temper_create(...)` and `temper_action(...)` patterns for `ProjectHarness`, `Monitor`, and agent spawning.

Changed file:

- [paw.md](/Users/seshendranalla/Development/openpaw-codex-integration-20260327000903/souls/paw.md)

### 4. E2B diagnostics were kept in `tool_runner`
I kept lightweight warnings in `tool_runner` for zero-frame and empty-output E2B responses. These are diagnostic only and do not change the happy path.

Changed file:

- [tool_runner/src/lib.rs](/Users/seshendranalla/Development/openpaw-codex-integration-20260327000903/os-apps/paw-agent/wasm/tool_runner/src/lib.rs)

---

## Fresh Verification Run

### 1. Build

Commands:

```bash
cargo build
cargo build --target wasm32-unknown-unknown --release --manifest-path os-apps/paw-channels/wasm/route_message/Cargo.toml
```

Result:

- `cargo build` finished successfully against upstream Temper `a6e8c00`
- `route_message` WASM rebuilt successfully

### 2. Daemon boot

Daemon was started from this worktree with the real `.env` loaded and reached bootstrap completion.

Live daemon:

- base URL: `http://localhost:3467/tdata`
- boot session: `18995`

Observed boot evidence included:

- Open Paw OS apps installed
- local WASM modules registered
- Cedar restored
- bootstrap completed

### 3. Curl-style continuing conversation rerun

Command:

```bash
python3 -u scripts/prove_channel_continuation.py
```

Artifacts:

- channel: `019d31e4-3154-7c82-b328-53f051e60bbc`
- route: `019d31e4-3262-7790-932c-2655ef24c1da`
- session: `019d31e4-32ae-7da1-bd4c-5083e3ccab5e`
- session file: `019d31e4-32e6-7f51-bb1d-f2d6a6c7cd05`
- first agent: `019d31e4-329e-7981-ad3a-75b41ef3ed3f`
- continuation agent: `019d31e4-3800-74a3-a648-cc61fe4aab49`

Observed replies:

- first reply: `REMEMBERED moon-biscuit-42`
- second reply: `RECALL moon-biscuit-42`

Recovered session file after restart:

```json
{"id":"h-019d31e4-329e-7981-ad3a-75b41ef3ed3f","parentId":null,"tokens":0,"type":"header","version":1}
{"content_file_id":"019d31e4-32ea-7191-b8ae-a6b226f29db1","id":"u-019d31e4-329e-7981-ad3a-75b41ef3ed3f-0","parentId":"h-019d31e4-329e-7981-ad3a-75b41ef3ed3f","role":"user","tokens":9,"type":"message"}
{"content_file_id":"019d31e4-3711-7340-aeea-00d0df019105","id":"a-2","parentId":"u-019d31e4-329e-7981-ad3a-75b41ef3ed3f-0","role":"assistant","tokens":12,"type":"message"}
{"content_file_id":"019d31e4-37d9-7cc0-9655-61bf392fd636","id":"u-3","parentId":"a-2","role":"user","tokens":9,"type":"message"}
{"content_file_id":"019d31e4-3bdd-7e30-880f-00c828da7f2f","id":"a-4","parentId":"u-3","role":"assistant","tokens":12,"type":"message"}
```

Conclusion:

- session continuation works
- continuation creates a child agent linked to the prior one
- resumed user turns are now file-backed, not inline-only

### 4. Real E2B execution rerun

I ran a fresh agent configured with `Developer`, no local `sandbox_url`, and a prompt that required a real bash call in E2B to:

- create `/home/user/e2b-proof/note.txt`
- write `e2b-file-backed-proof`
- print `TOOL_OK`
- return exactly `TOOL_OK`

Artifacts:

- agent: `019d31e5-37af-7e73-9a47-a214f665aaee`
- status: `Completed`
- result: `TOOL_OK`
- sandbox id: `ifll3o71je2ktwy75qnei`
- workspace: `019d31e5-39fa-7963-bb71-6503b17dc10e`
- session file: `019d31e5-3a3e-77d3-bd66-1d86d860fc70`
- manifest file: `019d31e5-3a24-7c92-82bd-71c34c6208f5`
- tool result content file: `019d31e5-4726-76a1-8298-51a03cdcd161`

Recovered manifest after restart:

```json
{"files":{"/home/user/e2b-proof/note.txt":{"file_id":"wsf-ace61fad8b8f0300","mtime":1774658602,"size_bytes":22}}}
```

Recovered tool-result content file after restart:

```json
[{"content":"TOOL_OK\ne2b-file-backed-proof\n","is_error":false,"tool_use_id":"toolu_01K3cJHEH8WerzF1AR23HvQj","type":"tool_result"}]
```

Conclusion:

- upstream Temper framing fix is working end to end for this E2B path
- tool result persistence is now verified, not just agent completion
- file manifest persistence is now verified, not just sandbox provisioning

### 5. Full self-heal rerun

Command:

```bash
python3 -u scripts/prove_self_heal_loop.py
```

Artifacts:

- harness: `019d31e5-c49c-7422-b585-6ec017a3c94f`
- monitor: `019d31e5-c4ab-7d30-8435-169cfb4f4b7a`
- alert cycle: `019d31e5-c4c5-7263-ae95-0bcfe54791e0`
- scout: `019d31e5-c4c8-7681-88e5-828ca1fcd101`
- developer: `019d31e6-3cae-7032-8e07-c60358171e28`
- work cycle: `019d31e5-fda6-7d72-a10a-a71292b1110e`
- PR: `https://github.com/arni-labs/deep-sci-fi/pull/75`
- commit: `05284d4`
- branch: `fix/platform-lockfile-drift`

Recovered `AlertCycle` after restart:

- `Status = Fixed`
- `pr_url = https://github.com/arni-labs/deep-sci-fi/pull/75`
- `fix_summary` persisted

Excerpt:

```json
{
  "entity_type":"AlertCycle",
  "entity_id":"019d31e5-c4c5-7263-ae95-0bcfe54791e0",
  "status":"Fixed",
  "fields":{
    "Status":"Fixed",
    "scout_agent_id":"019d31e5-c4c8-7681-88e5-828ca1fcd101",
    "pr_url":"https://github.com/arni-labs/deep-sci-fi/pull/75"
  }
}
```

Conclusion:

- the synthetic-alert self-heal loop still works end to end
- `Scout` triages, opens governance state, and spawns `Developer`
- `Developer` clones, fixes, validates, pushes, and opens a real PR
- alert state survives restart

### 6. Restart recovery

After the fresh reruns, I killed the daemon, restarted it, and then re-read:

- the self-heal `AlertCycle`
- the channel session file
- the E2B manifest file
- the E2B tool-result content file

All four were readable and still contained the expected data.

Conclusion:

- entity persistence survived restart
- file-backed session content survived restart
- sandbox fsync artifacts survived restart
- Open Paw came back able to serve those artifacts without re-running the jobs

---

## What Is Working Right Now

### 1. Open Paw builds and boots against upstream Temper
- no vendored Temper code is required in Open Paw
- the active dependency target is `feat/temper-claw`

### 2. Multi-turn channel continuation works
- the same thread can continue through `Channel` + `ChannelSession`
- the second turn sees the first turn's context
- resumed user turns are now file-backed in the session tree

### 3. File-backed session storage works in the core agent path and channel continuation
- session trees store structure
- content lives in TemperFS `File` entities via `content_file_id`

### 4. E2B provisioning and execution work with persisted evidence
- real E2B sandbox provisioning works
- real bash execution works
- real tool-result content files persist
- real synced file manifests persist

### 5. The self-heal workflow works end to end from a synthetic alert source
- proof driver creates `ProjectHarness`, `Monitor`, and initial `AlertCycle`
- `Scout` creates the `WorkCycle` and spawns `Developer`
- `Developer` follows workflow state, fixes the repo, validates it, pushes a branch, and opens a PR
- the resulting `AlertCycle` becomes `Fixed`

### 6. Restart recovery is proven for the core persistence path
- agent-facing entities survive restart
- file-backed session state survives restart
- E2B manifest and tool-result files survive restart

### 7. Paw has better workflow guidance now
- `Paw` has concrete OData tool patterns for creating and configuring harness/heal state

---

## What Is Still Not Working, Or Still Not Fully Proven

### 1. The strongest self-heal proof still starts from a synthetic alert source
The alert source is simulated by the proof driver, not pushed from a real observability system. The fix/PR path is real, but the alert ingress is still synthetic.

### 2. Webhook ingestion is basic, not provider-complete
`POST /webhooks/alerts` exists, but provider-grade verification and mapping for Datadog or Logfire are still not fully implemented or proven here.

### 3. The harness is operational workflow state, not a hard execution substrate
`ProjectHarness` is real and used by the flow, but it still does not itself enforce:

- sandbox image definition
- tool allowlists
- branch protections
- dependency policy
- machine lifecycle

It shapes work; it does not yet fully govern execution.

### 4. `paw-compute` is still largely vision-level
There is no fully proven first-class persistent computer lifecycle with provision, sleep, wake, checkpoint, and destroy semantics.

### 5. Local sandbox remains dev-only
The local Python sandbox helper is still host-backed and non-isolated. Production-grade remote execution still means E2B today.

### 6. CI/CD closure still ends at PR creation
The proven loop ends at:

- fix validated
- branch pushed
- PR opened

Merge, deploy, rollback, and post-deploy verification are not part of the proven loop yet.

### 7. Discord round-trip was not re-proven in this report
The transport and route/session logic are in better shape, but this report's fresh proof set used curl plus scripts, not a new human Discord DM exchange.

---

## Current Architecture

```text
operator / proof script / curl
        |
        v
   Open Paw daemon
   http://localhost:3467/tdata
        |
        +--> startup installs OS apps, Cedar, souls, WASM
        |
        +--> Channel.ReceiveMessage
        |       |
        |       v
        |   ChannelSession + AgentRoute
        |       |
        |       v
        |   route_message.wasm
        |       |
        |       +--> create or continue Agent
        |       +--> append user turn to session tree
        |       +--> externalize message content to TemperFS File entities
        |
        +--> Agent
        |       |
        |       +--> llm_caller.wasm
        |       |       |
        |       |       +--> resolves session tree refs into prompt content
        |       |       +--> Anthropic tool_use / assistant output
        |       |
        |       +--> tool_runner.wasm
        |               |
        |               +--> local sandbox helper, or
        |               +--> E2B via Temper Connect/envd
        |               +--> fsync back into TemperFS manifest + files
        |
        +--> paw-harness entities
        |       |
        |       +--> ProjectHarness
        |       +--> WorkCycle
        |
        +--> paw-heal entities
                |
                +--> Monitor
                +--> AlertCycle
                +--> Scout -> Developer remediation loop
```

---

## Bottom Line

The gaps called out after the previous review were materially closed in this rerun:

- upstream Temper was fixed in the right place
- Open Paw consumes Temper as upstream, not vendor code
- channel continuation now uses file-backed session content
- E2B evidence capture is now proven with real persisted tool output and file-manifest data
- the self-heal loop was re-run successfully and produced PR `#75`
- persistence was re-checked after a daemon restart

The honest remaining gaps are now mostly architectural scope gaps, not “this proof path is shaky” gaps:

- real external monitor ingestion
- full compute abstraction
- Discord round-trip re-proof
- deploy/rollback closure after PR
