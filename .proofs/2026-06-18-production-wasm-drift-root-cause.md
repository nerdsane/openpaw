# Production WASM Drift Root Cause

Date: 2026-06-18

Linear: ARN-61

## Summary

Production did not randomly lose the Paw DM image generation fix. The prior live
proof passed because corrected WASM modules were hot-uploaded into production.
The durable Genesis app refs used by production still point at bundles that
contain the stale WASM artifacts, so restart/reconcile can converge production
back to those stale hashes.

## Evidence

Railway production `openpaw` is running image tag `sha-d7ce726`, but startup is
governed by `TEMPERPAW_GENESIS_BOOTSTRAP_REFS`. The production variable is pinned
to Genesis app refs including:

- `temperpaw/paw-agent@18f58e340795e66015e428534679222eb2afaced`
- `temperpaw/paw-media@7098fc6c3ba726880af3d2a9a005429dd90f7df0`
- `temperpaw/paw-channels@3dd250da9777e1e623edbfed94df665012f96b3b`

Production `tenant_installed_apps` confirms the default tenant is installed from
Genesis and pinned to those refs:

- `paw-agent`: `source_kind=genesis`, `follow_policy=pinned`,
  `app_ref=temperpaw/paw-agent@18f58e340795e66015e428534679222eb2afaced`,
  installed at `2026-04-14 16:35:09+00`
- `paw-media`: `source_kind=genesis`, `follow_policy=pinned`,
  `app_ref=temperpaw/paw-media@7098fc6c3ba726880af3d2a9a005429dd90f7df0`,
  installed at `2026-06-17 18:22:49.788969+00`
- `paw-channels`: `source_kind=genesis`, `follow_policy=pinned`,
  `app_ref=temperpaw/paw-channels@3dd250da9777e1e623edbfed94df665012f96b3b`,
  installed at `2026-04-14 16:36:21+00`

The live Genesis bundle endpoint for those pinned refs contains the stale hashes
observed during the drift:

```text
paw-agent@18f58e340795e66015e428534679222eb2afaced
wasm/agent_reply/agent_reply.wasm         6af95d432bed49c23a0b145c67ef7ab782f59dbfab2388d89d26a11d15225bfa
wasm/monty_repl/monty_repl.wasm           a32adbc638414343dbcfad68d72eb3308d1b643930c9b49c6f76f0ab4ce232e2

paw-media@7098fc6c3ba726880af3d2a9a005429dd90f7df0
wasm/openai_codex_image_generate/openai_codex_image_generate.wasm
                                           9c5520c2bacf3600380760b2e07c5794949a2779476cef964ac1fa197ff1bb3c

paw-channels@3dd250da9777e1e623edbfed94df665012f96b3b
wasm/route_message/route_message.wasm      9f679aba98010b87a777224555e74521cc3e27b7e8b4719399c835ec6a4df408
wasm/send_reply/send_reply.wasm            aa92dcd9e1dc06e8fd2c928b1ad6b08845b2325230e5c8d5057ae02f3076ddfd
```

Current production rows show the re-hotloaded fixed modules as `source=upload`,
which explains why the live path works again now:

```text
default | agent_reply                 | e36d025b52bf420b3e5a8fd1c6fb10a1a9eca157f253c683f492b7e80fe6495f | upload
default | monty_repl                  | af3293660446b6ff5de831d4d02f149d49a0a8f253ce620696ecf364a41f1676 | upload
default | openai_codex_image_generate | f5ba13c268f0cf639114922080502f5bda715c206c2188f1fb5f0baa91cdc67f | upload
```

## Code Path

TemperPaw startup reads `TEMPERPAW_GENESIS_BOOTSTRAP_REFS` in
`crates/temperpaw/src/startup.rs`. When that variable is non-empty,
`startup_os_apps()` returns an empty local startup list, so production does not
treat the checked-out `os-apps/*` tree as the startup source of truth.

For each pinned Genesis ref, `bootstrap_configured_genesis_apps()` checks the
durable installed app record. If runtime recovery finds drift, it runs
`install_genesis_app_from_registry(...)`, which materializes the pinned Genesis
bundle and adds that cache root as the preferred OS-app catalog source.

The pinned Temper installer treats manifest-declared `app-required` WASM modules
as bundle-owned. `paw-agent`, `paw-media`, and `paw-channels` all declare the
affected modules as `app-required`, so a later Genesis reconcile can replace a
differing hot-uploaded module with the bundled hash.

## Conclusion

The root cause is a split deployment source of truth:

1. The bugfix landed in code and was hot-uploaded to production.
2. Production was still pinned to old Genesis app refs.
3. Those old Genesis app refs contain the exact stale WASM artifacts.
4. Startup/reconcile prefers Genesis-pinned bundles and required WASM modules are
   owned by those bundles.
5. Production can therefore revert from the hot-uploaded fixed hashes back to the
   stale bundled hashes on restart/reconcile.

## Durable Fix

The durable path is:

1. Merge/deploy PR #413 for the source fixes.
2. Publish refreshed Genesis app bundles for `paw-agent`, `paw-media`, and
   `paw-channels` containing the corrected packaged WASM artifacts.
3. Update production `TEMPERPAW_GENESIS_BOOTSTRAP_REFS` to the new pinned refs.
4. Restart/redeploy production and verify:
   - `/observe/wasm/modules/*` does not return the stale hashes.
   - `tenant_installed_apps` points at the new Genesis refs.
   - A normal Discord DM image request creates a real PawFS PNG and sends it back
     as a Discord attachment without a post-restart hot upload.

This matches the warning in `os-apps/paw-agent/adrs/033-temporary-media-route-hot-upload.md`:
the hot-upload bridge can be overwritten until Genesis publishing and pinned refs
are updated.
