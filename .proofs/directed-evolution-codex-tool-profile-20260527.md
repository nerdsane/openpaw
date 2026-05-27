# Directed Evolution Codex Tool Profile Proof

Date: 2026-05-27

## Change Under Test

`PAW_CODEX_ENABLE_DATADOG_MCP=1` lets a trusted local TemperPaw worker launch
child Codex sessions with a Datadog MCP server definition while preserving
`--ignore-user-config`, so Directed Evolution brain roles can inspect Datadog
without inheriting arbitrary user config.

Default worker execution still includes `--ignore-user-config`.

## Static And Unit Verification

- `cargo fmt -p paw-codex-worker`
- `cargo test -p paw-codex-worker codex_ --no-default-features`
- `cargo test -p paw-codex-worker directed_evolution_ --no-default-features`
- `cargo test -p paw-codex-worker launchd_plist_renders_concrete_worker_environment --no-default-features`
- `cargo test -p paw-codex-worker --no-default-features`
- `git diff --check`

Result: all passed.

## Child Codex Tool Smoke

Command shape:

```bash
codex exec --ignore-user-config --json \
  -c 'mcp_servers.datadog.url="https://mcp.datadoghq.com/api/unstable/mcp-server/mcp?toolsets=all"' \
  -c model_reasoning_effort='"low"' \
  --ephemeral --sandbox read-only \
  --cd /Users/seshendranalla/Development/temperpaw-worktrees/directed-evolution-hotload-variants \
  --skip-git-repo-check \
  'Do not edit files. Use tool discovery if available to check for Datadog MCP tools...'
```

Result: child Codex returned `datadog_tools_visible=true` and listed Datadog
tools including `load_datadog_skill`, `list_datadog_skills`,
`search_datadog_services`, and `search_datadog_monitors`.

## Live Directed Evolution Worker Smoke

Control tenant: `de-control-agent-answers-20260527001135`

Created smoke evidence and signal:

- EvidenceArtifact: `en-019e6821-5d08-7720-9768-da65678cb637`
- Signal: `en-019e6821-8a52-7363-9373-7ad4b8d7ea8e`

The hot-loaded Directed Evolution `signal_observer` queued:

- WorkItem: `en-019e6821-a9a7-7e61-9faf-4e049e91cc56`

Ran the local worker with:

```bash
PAW_CODEX_ENABLE_DATADOG_MCP=1 \
PAW_CODEX_ENABLE_EXECUTION=1 \
PAW_CODEX_POLL_ON_START=1 \
cargo run -p paw-codex-worker -- run
```

Resulting live entities:

- BrainRun `en-019e6821-ffc6-7870-bfa8-b1f62cb2bdc0`: `Succeeded`
- WorkItem `en-019e6821-a9a7-7e61-9faf-4e049e91cc56`: `Succeeded`
- EvidenceArtifact `en-019e6822-8a71-7092-9c59-a15381d3d3b7`: `Linked`
- Signal `en-019e6821-8a52-7363-9373-7ad4b8d7ea8e`: `Ignored`

The brain output included Datadog-backed `evidence_scope` entries:

- `Datadog MCP tool discovery and datadog/logs skill load`
- `service:temperpaw env:local, last 24h`

The linked evidence URI was a Datadog logs URL:

`https://app.datadoghq.com/logs?query=service%3Atemperpaw%20env%3Alocal`

The observer marked the smoke signal `actionable=false`; querying Directions for
the smoke signal id returned `[]`, so no product direction was created.
