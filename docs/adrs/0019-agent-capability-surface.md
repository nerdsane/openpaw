# ADR-0019: Agent Capability Surface and Discord Delivery Hardening

## Status

Accepted

## Context

Paw's Discord behavior exposed a set of platform mismatches that blocked the self-extension loop:

- New routed sessions defaulted to an incomplete tool set, so Paw could not reliably call `temper.specs`, `temper.submit_specs`, `temper.write`, or other core capabilities.
- Existing seeded `AgentRoute` configs still contained stale tool tokens and legacy workdir assumptions, so even previously created routes could boot into a degraded capability surface.
- The CSDL model had drifted behind the IOA specs for `Session`, `Skill`, and `Memory`, leaving important fields and actions invisible at the OData boundary.
- Child sessions looked up Discord bindings only by their own `agent_entity_id`, so spawned agents could lose reply delivery, typing indicators, and approval prompts.
- Discord rich-content delivery assumed content-bearing webhook payloads and did not degrade gracefully when embeds or component payloads exceeded Discord limits.

These failures were independent of Cedar authorization. Paw could be denied by policy when appropriate, but it also routinely failed before reaching the policy layer.

## Decision

### 1. Canonical session defaults

OpenPaw now defines a canonical default tool set and workdir for routed sessions:

- `DEFAULT_TOOLS_ENABLED` contains the supported Temper and sandbox capabilities Paw is expected to use.
- `DEFAULT_WORKDIR` is `/workspace`.

The route-message WASM uses these defaults both for new sessions and for resumed sessions when prior values are missing.

### 2. Repair seeded and legacy AgentRoute config

Startup now repairs global and legacy `AgentRoute.agent_config` payloads:

- missing `model`, `provider`, and `temper_api_url` are filled in
- `/tmp/workspace` is normalized to `/workspace`
- stale tool tokens are rewritten to their current names
- obsolete tokens are removed

This keeps previously seeded routes usable without requiring manual database surgery.

### 3. Align the OData model with runtime reality

The OpenPaw CSDL now matches the IOA/runtime surface for the agent app:

- `Session` includes the project, decision, REPL, and provisioning fields used at runtime
- `Session.Configure` includes the resume and project parameters the WASM code already passes
- `SwitchProvider` and `DeliveryFailed` are exposed in the model
- `Skill` and `Memory` expose `project_id`

This makes the state machine readable and writable through the same contract the runtime already expects.

### 4. Make spec discovery part of the default agent surface

`read_specs` on `Spec` is permitted for authenticated agents. Capability discovery is read-only metadata and is necessary for dynamic, Temper-native extension.

### 5. Bootstrap foundational skills

The startup path now bootstraps the foundational OpenPaw skills needed for self-extension:

- `Temper App Creation`
- `Platform Awareness`
- `OpenPaw Agent`

These are registered as globally available skills so agents can discover and apply them without relying on Claude-only skill directories.

### 6. Preserve parent-channel continuity for child sessions

Reply delivery, approval prompts, and typing indicators now fall back to the parent session binding when a child session does not own its own `ChannelSession`.

This keeps spawned agents visible in the same Discord conversation instead of silently skipping delivery.

### 7. Harden Discord delivery

Discord transport now:

- accepts embed-only and component-only webhook payloads
- retries on HTTP 429 using `Retry-After`
- detects oversized payload failures
- falls back from rich content to chunked plain text plus component follow-up
- records reply delivery failures on the `Session` entity

The agent-reply WASM also reads fresh reply text from recent event params before falling back to persisted fields, which avoids losing live output when entity fields are truncated upstream.

## Consequences

### Positive

- Paw can discover platform capabilities and attempt self-extension from Discord.
- Existing seeded routes are repaired in place instead of continuing to fail with stale config.
- Child sessions remain visible in the parent Discord thread.
- Long and rich replies degrade gracefully instead of disappearing silently.

### Negative

- The default capability surface is broader, so Cedar remains the primary enforcement layer for sensitive actions.
- Message delivery logic is more complex because it now explicitly handles Discord transport constraints.

## Follow-up

- Temper still needs a first-class large-field overflow solution so oversized entity fields are preserved at the platform layer, not only mitigated in the OpenPaw delivery path.
