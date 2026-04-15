---
name: platform-awareness
description: Discover installed apps and capabilities — and design new ones when what you need doesn't exist
---

# Platform Awareness

## Your capabilities are not fixed — they come from installed apps.

This is your defining advantage: you are never stuck waiting for a feature. If what you need does not exist, you design it, submit the spec, verification proves it correct, and it is live in minutes — formally verified and governed, not duct-taped together. Most agents are limited to whatever tools someone pre-built for them. You extend your own capability surface at runtime.

You are an agent running on the Temper platform. Your tools (`temper.create`, `temper.action`, `temper.list`, etc.) are generic — they operate on **entity types**. What entity types exist, what actions they support, and what WASM integrations run behind them — all of that comes from **installed apps**.

**Default posture:** When you encounter a need that isn't met by an installed app, your first response is to design one — not to work around the gap with a shell script or ad-hoc automation. Check what exists first (`temper.specs()` and `temper.list("Apps", "Status eq 'Installed'")`). If nothing fits, the `temper-app-creation` skill has the full authoring guide.

An app is not a shell script. An app is not a binary on PATH. An app is a governed package of:

- **Entity specs** (`.ioa.toml`) — state machines with named states, actions, transitions, invariants
- **Cedar policies** (`.cedar`) — attribute-based access control governing who can do what
- **WASM modules** (`.wasm`) — integration logic triggered by entity actions (API calls, provisioning, computation)
- **Reactions** (`reactions.toml`) — cross-entity cascades that fire when an action completes
- **A manifest** (`app.toml`) — name, version, dependencies on other apps

When an app is installed, the platform registers its entity types, loads its policies, wires its WASM triggers, and activates its reactions. After that, you can `create`, `list`, `action`, and `get` against those entity types. Before that, you can't.

**This is the difference between a Temper-native capability and a shell tool.** A Temper-native app has state machines, authorization, audit trails, and governed transitions. A shell tool has none of that. Never confuse the two.

## Discovering what's available right now

Don't assume what you can do. Discover it.

### What entity types are registered?

```python
specs = temper.specs()
# Returns the registered entity types, their states, actions, and fields
# This is the live truth — not a static list
```

### What apps have been installed?

```python
apps = temper.list("Apps", "Status eq 'Installed'")
# Returns App entities with name, description, version, app_guide_file_id
# Read an app's guide for architecture context:
for app in apps:
    guide = temper.read(f"/apps/{app['Name']}/APP.md")
```

### What entities exist for a given type?

```python
# Replace with any entity set name from specs
items = temper.list("Workspaces", "")
items = temper.list("WebQueries", "Status eq 'Complete'")
items = temper.list("Sessions", "Status eq 'Running'")
```

### What Cedar policies are active?

```python
policies = temper.list_policies()
# Shows what access rules govern entity actions
```

### What agents and sessions are running?

```python
agents = temper.list("Agents", "Status eq 'Active'")
sessions = temper.list_sessions()
```

### What skills are available?

Skills are listed in your system prompt as `<skill name="..." description="..." path="..." />`.
To load full content, read the path:

```python
content = temper.read("/system/skills/platform-awareness/SKILL.md")
# Skills are scoped by path:
#   /system/skills/         → platform knowledge (all agents)
#   /agents/{id}/skills/    → agent-specific skills
#   /projects/{id}/skills/  → project-specific skills
```

### What memories exist?

```python
temper.recall_memory("topic")
# Searches across your agent's persisted memories
```

**Run `temper.specs()` early in your session when you need to understand your capability surface.** This is especially important when someone asks "what can you do?" — the answer is not a static list of `temper.*` methods. The answer is: whatever entity types are registered, whatever actions those types support, whatever WASM integrations power them.

## How capabilities compose

Apps depend on other apps. For example:

- An agent management app depends on a file storage app (souls and skills are stored as files)
- A research app provides web query entities that other apps' WASM modules can reference
- A project management app depends on agent management (issues get assigned to agents)

When you call `temper.specs()`, you see the flattened result — every entity type from every installed app. The dependency chain is already resolved.

Entity types from different apps can interact through:

1. **ID references** — one entity stores another entity's ID as a field
2. **Reactions** — completing an action on one entity type auto-triggers an action on another
3. **WASM integrations** — a WASM module triggered by one entity can create/action other entities

## Extending the platform — creating new capabilities

You can create new Temper-native capabilities at runtime. This is **not** the same as writing a shell script.

### Minimum viable app: entity spec + policy

```python
# 1. Define the entity type — a state machine
spec = """
[automaton]
name = "Bookmark"
namespace = "OpenPaw"

[[state]]
name = "Status"
type = "string"
initial = "Active"

[states]
values = ["Active", "Archived"]

[[action]]
name = "Create"
kind = "input"
from = ["Active"]
to = "Active"

[[action]]
name = "Archive"
kind = "input"
from = ["Active"]
to = "Archived"
"""

# 2. Submit the spec bundle — registers the entity type
# submit_specs requires model.csdl.xml plus one or more *.ioa.toml files
temper.submit_specs({
    "model.csdl.xml": csdl_model,
    "bookmark.ioa.toml": spec,
})

# 3. Create a Cedar policy — allow agents to use it
temper.submit_policy("bookmark-access", '''
permit(
    principal is Agent,
    action in [Action::"Create", Action::"Archive"],
    resource is Bookmark
);
''')

# 4. Now use it — the entity type is live
temper.create("Bookmarks", {"url": "https://example.com", "title": "Example"})
temper.list("Bookmarks", "Status eq 'Active'")
temper.action("Bookmarks", bookmark_id, "Archive", {})
```

### Adding integration logic with WASM

If an action should trigger external behavior (API calls, computation, provisioning), that logic lives in a WASM module:

```python
# Upload a compiled WASM module
import base64
wasm_bytes = sandbox.read("/path/to/module.wasm", binary=True)
temper.upload_wasm("my_integration", base64.b64encode(wasm_bytes).decode())
```

The WASM module gets referenced in the `.ioa.toml` spec via an integration block, and the platform calls it when the associated action fires.

### Installing a packaged app

If a pre-built app exists in the platform's app catalog:

```python
temper.install_app("my-app", reason="Need bookmark management for project tracking")
# This reads the app's manifest, resolves dependencies, registers specs, policies, WASM, and reactions
```

### The full app anatomy (for authoring from scratch)

```
my-app/
├── app.toml                  # name, version, dependencies
├── APP.md                    # human documentation
├── adrs/                     # design decisions for the app
│   └── 001-initial-design.md
├── specs/
│   ├── entity_name.ioa.toml  # one state machine per entity type
│   └── model.csdl.xml        # OData data model (required for submit_specs)
├── policies/
│   └── entity_name.cedar     # Cedar access rules
├── wasm/
│   ├── module_name/
│   │   ├── Cargo.toml        # Rust crate
│   │   └── src/lib.rs        # integration logic
│   └── module_name.wasm      # compiled binary
└── reactions/
    └── reactions.toml         # cross-entity cascades
```

## What you should never do

- **Never create a shell script or CLI binary and call it an "app."** That's a sandbox tool, not a Temper-native capability. It has no state machine, no Cedar policy, no audit trail, no governance.
- **Never hardcode a list of capabilities.** Query `temper.specs()` — the live platform is the source of truth.
- **Never assume an entity type exists without checking.** If `temper.list("SomeEntities", "")` fails, the app providing that type may not be installed.
- **Never bypass Cedar governance by doing work in the sandbox that should be a governed entity action.** If the work needs traceability, authorization, or state transitions — it belongs in a Temper entity, not a bash command.
