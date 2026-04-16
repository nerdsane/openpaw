# Run 005 Changelog

## Changed File
`os-apps/paw-foresight/wasm/spawn_orchestrator/src/lib.rs`

## What Changed

The orchestrator Session was previously configured with `provider` and `model` fields
derived from the ForesightModel's `seed_provider` and `seed_model` — i.e. the same
provider that authored the knowledge graph was also used to run orchestration. For the
DSE v2 ForesightModel, those values were `"openai"` / `"gpt-5.4"`, yielding
`openai_codex` for the orchestrator.

The orchestrator now hardcodes `anthropic_codex` + `claude-sonnet-4-6`, decoupled from
the ForesightModel entirely. The ForesightModel is still fetched for its `name` field
(used in the prompt) but its seed_provider/seed_model are ignored for orchestration.

This is an architectural fix, not a prompt edit. The rationale is that seeding and
orchestration are unrelated tasks: seeding is a single-shot authoring call from an
essay; orchestration is a long multi-turn coordinator with many tool calls. Tying
them together conflated two independent provider choices and made the orchestrator
inherit whatever reliability characteristics the seed provider happened to have —
which, for openai_codex, meant session timeouts in Runs 002, 003, and 004.

## Diff (key lines)

Before:
```rust
let (fm_name, seed_model, seed_provider) = if fm_resp.status >= 200 && ... {
    let name = f.get("name")...;
    let model = f.get("seed_model").unwrap_or("gpt-5.4").to_string();
    let provider = f.get("seed_provider").unwrap_or("openai").to_string();
    (name, model, provider)
} else { ("unknown", "gpt-5.4", "openai") };
...
let provider_codex = format!("{seed_provider}_codex");
let configure_body = json!({
    "model": seed_model,
    "provider": provider_codex,
    ...
});
```

After:
```rust
let fm_name = if fm_resp.status >= 200 && ... {
    f.get("name").unwrap_or("unknown").to_string()
} else { "unknown".to_string() };

// Orchestrator model/provider are hardcoded for reliability.
let orchestrator_model = "claude-sonnet-4-6";
let orchestrator_provider = "anthropic_codex";
...
let configure_body = json!({
    "model": orchestrator_model,
    "provider": orchestrator_provider,
    ...
});
```

## Rebuild & Reload

```
cargo build --target wasm32-unknown-unknown --release
cp .../target/.../release/spawn_orchestrator.wasm .../spawn_orchestrator.wasm
# Then uploaded directly:
POST /api/wasm/modules/spawn_orchestrator
  -> sha256: 998cea22a0b578e333456476c75842a2d57462a988f92bbd3d97cfff71af73e0
  -> size: 228772 bytes
# Plus reinstalled the full app:
POST /api/os-apps/paw-foresight/install  -> status: installed
```

## Verification

- WASM build succeeded with no errors (warnings are about unused patch crates,
  unrelated to this module).
- Module upload returned 200 with the new SHA-256 hash.
- Full app reinstall returned `status: installed` with all entities present.
- Orchestrator provider decision is now a first-class orchestrator concern, not a
  side-effect of ForesightModel seed choice.
