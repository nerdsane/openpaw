---
name: temper-app-creation
description: Build Temper apps when you need a new capability — your primary way to extend the platform
---

# Temper App Creation

## Everything is an app.

Every new capability — a tool, an integration, a workflow, a full product feature — is a Temper app. Not a script. Not a library. Not a CLI command. An app.

An app is a governed bundle that gives you new entity types, actions, integrations, and policies. Once installed, you use it through the same `temper.create`, `temper.list`, `temper.action` methods you already know. Before it's installed, those entity types don't exist.

**If you want to do something new, you build an app for it.**

**Why an app, not a script?** A script is a dead end — it does not evolve, it does not learn from use, and no one can see what it did or why. An app is a living capability. It can be improved without being rewritten. Usage reveals gaps and the system proposes fixes. Other agents and apps can compose with it. It hot-reloads without downtime, so iteration is as fast as describing what you want differently. When you build an app, you are not solving a problem once — you are creating a tool that gets better over time. Scripts are disposable. Apps compound.

---

## What's in an app

```
my-app/
  app.toml              # Identity: name, version, dependencies
  APP.md                # What the app does (human documentation)
  adrs/                 # Architecture Decision Records (why this design exists)
    001-initial-design.md
  specs/
    entity.ioa.toml     # State machines (one per entity type)
    model.csdl.xml      # OData data model (entity types, properties, actions)
  policies/
    entity.cedar        # Authorization rules (who can do what)
  wasm/
    module_name/
      Cargo.toml        # Rust crate
      src/lib.rs         # Integration logic
    module_name.wasm     # Compiled binary
  agents/                # Agent definitions (if the app includes agents)
    agent-name/
      AGENT.md           # Agent instructions
      skills/            # Agent-scoped skills
        skill-name/
          SKILL.md       # Skill content (YAML frontmatter: name, description)
  system/
    skills/              # System-level skills (available to ALL agents)
      skill-name/
        SKILL.md
  seed-data/             # Initial entities to create on install
```

## Storage Rule

Choose storage by intent, not by size alone.

Use Temper entities and actions for operational state:

- session turns and tool results
- job progress and review decisions
- parent/child session links
- durable memory records
- state that another workflow must query, resume, or react to

Use inline fields for bounded data:

- short descriptions
- comments
- labels
- bounded notes
- prompts that are intentionally small

For large operational payloads, prefer a Temper entity with a bounded field and
let Temper's blob-ref overflow move the bytes to the object/blob data plane.
The entity stays the control-plane record; the blob is just bytes.

Use `Files` plus `content_file_id` or another `*FileId` field only when the
thing is intentionally a governed file/artifact:

- markdown pages
- reports
- transcripts
- fetched documents
- compiled analyses
- large JSON outputs
- HTML or rendered artifacts
- exported or reviewable LLM outputs that should have file provenance

Pattern:

1. write bytes with `temper.write(...)` or `Files('{id}')/$value`
2. store the returned file id on the entity
3. keep only metadata inline

Do not store hot session or workflow state by rewriting a PawFS file. Create a
domain entity or dispatch a domain action instead.

## Recording design decisions

Apps should carry their own decision trail.

Specs, policies, and WASM define what an app does. ADRs explain why the app is
shaped that way so later humans and agents can evolve it without reconstructing
intent from diffs or chat logs.

Write ADRs when:

- you create a new app: at least one ADR covering entity types and state machine design
- you make a significant change: one ADR per meaningful design decision
- you change policy posture, reaction flow, or app dependencies

Keep ADRs in `adrs/` with zero-padded filenames:

```text
adrs/
  001-initial-design.md
  002-approval-routing.md
```

Template:

```markdown
# ADR-NNN: {Title}

**Status:** {Proposed | Accepted | Superseded by ADR-NNN}
**Scope:** {entity-types | state-machine | policies | integrations | reactions | dependencies}
**Author:** {agent-id or human name}
**Date:** {YYYY-MM-DD}

## Context

{What prompted this decision?}

## Decision

{What was decided and why. Name the entities, actions, policies, or modules.}

## Consequences

### Positive
- ...

### Negative
- ...
```

Use the fixed scope vocabulary so app histories stay consistent:

- `entity-types`
- `state-machine`
- `policies`
- `integrations`
- `reactions`
- `dependencies`

Filesystem apps store ADRs as regular markdown files in `adrs/`.

Runtime-created apps should write ADRs through TemperFS before submitting specs:

```python
temper.write(
    "/apps/my-app/adrs/001-initial-design.md",
    adr_content,
    {"workspace": "default", "mime_type": "text/markdown"},
)
```

Example for the Bookmark app in this guide:

```markdown
# ADR-001: Bookmark lifecycle

**Status:** Accepted
**Scope:** state-machine
**Author:** paw
**Date:** 2026-04-08

## Context

Bookmark management only needs one active state and one terminal archive state.

## Decision

Model Bookmark with `Active` and `Archived` only. Use a self-loop `Create`
action so creation captures metadata without introducing a staging state.

## Consequences

### Positive
- Simple state space, easy verification

### Negative
- No review or staging workflow without later evolution
```

### Required parts

| Part | File | Purpose |
|------|------|---------|
| **IOA spec** | `specs/*.ioa.toml` | State machine: states, actions, transitions, guards, invariants, integrations |
| **CSDL model** | `specs/model.csdl.xml` | OData schema: entity types, properties, entity sets, bound actions |

### Optional parts

| Part | File | Purpose |
|------|------|---------|
| **Cedar policy** | `policies/*.cedar` | Access control: who can perform which actions on which entities |
| **WASM module** | `wasm/*.wasm` | Integration logic: external API calls, computation, side effects triggered by actions |
| **Manifest** | `app.toml` | Metadata: name, version, dependencies on other apps |
| **Documentation** | `APP.md` | Human-readable description of what the app does |
| **ADRs** | `adrs/*.md` | Decision records: why this state machine, why these entity types, why this policy approach |
| **Action triggers** | `[[action.triggers]]` in `specs/*.ioa.toml` | Cross-entity cascades and post-commit integrations declared inline on the source action |

### Part relationships

```
IOA spec (state machine)
  defines → states, actions, transitions
  references → WASM modules via [[integration]] blocks
  enforced by → Cedar policies
  described by → CSDL model (OData projection of the same entity)

CSDL model (data model)
  must match → IOA spec exactly (same entity names, same actions, same states)
  defines → property types, entity sets, bound actions for OData API

Cedar policy (authorization)
  gates → every action defined in IOA spec
  evaluated → before any state transition executes

WASM module (integration)
  triggered by → action effects in IOA spec
  can call → external HTTP APIs, create/action other entities
  reports back → success/failure callback actions
```

---

## Two paths to a live app

### Path 1: Runtime creation (from scratch, via agent tools)

Use this when you're creating a new capability on the fly. No files on disk needed.

```python
# 1. Design the state machine
ioa_spec = """
[automaton]
name = "Bookmark"
states = ["Active", "Archived"]
initial = "Active"

[[action]]
name = "Create"
kind = "input"
from = ["Active"]
to = "Active"
hint = "Create a new bookmark."

[[action]]
name = "Archive"
kind = "input"
from = ["Active"]
to = "Archived"
hint = "Archive a bookmark."

[[invariant]]
name = "ArchivedIsFinal"
when = ["Archived"]
assert = "no_further_transitions"
"""

# 2. Design the data model (must match IOA exactly)
csdl = """<?xml version="1.0" encoding="utf-8"?>
<edmx:Edmx Version="4.0" xmlns:edmx="http://docs.oasis-open.org/odata/ns/edmx">
  <edmx:DataServices>
    <Schema Namespace="TemperPaw" xmlns="http://docs.oasis-open.org/odata/ns/edm">
      <EntityType Name="Bookmark">
        <Key><PropertyRef Name="id"/></Key>
        <Property Name="id" Type="Edm.String" Nullable="false"/>
        <Property Name="state" Type="Edm.String" Nullable="false"/>
        <Property Name="url" Type="Edm.String"/>
        <Property Name="title" Type="Edm.String"/>
      </EntityType>
      <EntityContainer Name="Default">
        <EntitySet Name="Bookmarks" EntityType="TemperPaw.Bookmark"/>
      </EntityContainer>
    </Schema>
  </edmx:DataServices>
</edmx:Edmx>"""

# 3. Submit — platform verifies the spec (L0-L3 verification cascade)
result = temper.submit_specs({
    "bookmark.ioa.toml": ioa_spec,
    "model.csdl.xml": csdl
})
# If verification fails, read the error, fix the spec, resubmit (max 3 cycles)

# 4. Add authorization (Cedar policy)
temper.submit_policy("bookmark-access", """
permit(
    principal,
    action in [Action::"Create", Action::"Archive"],
    resource is Bookmark
);
""")

# 5. Use it — entity type is now live
temper.create("Bookmarks", {"url": "https://example.com", "title": "Example"})
temper.list("Bookmarks", "Status eq 'Active'")
temper.action("Bookmarks", bookmark_id, "Archive", {})
```

### Path 2: Install a pre-built app (from the app catalog)

Use this when an app already exists as a directory (in `os-apps/` or a known catalog).

```python
# Install by name — resolves dependencies, loads specs, policies, WASM, reactions
temper.install_app("paw-research", reason="Need web search capability")
```

The platform reads the app's `app.toml`, installs dependencies first, then registers everything. After install, `temper.specs()` shows the new entity types.

---

## IOA spec format (complete reference)

The IOA spec is the source of truth. Everything else (CSDL, Cedar, WASM) derives from it.

```toml
[automaton]
name = "EntityName"              # Must match CSDL EntityType Name
states = ["State1", "State2", "State3"]  # All possible states
initial = "State1"               # Starting state

# State variables (optional — for counters, flags)
[[state]]
name = "attempt_count"
type = "counter"                 # "counter" (Int32) | "bool" (Boolean)
initial = "0"

# Actions — state transitions
[[action]]
name = "Submit"                  # Action name (must match CSDL Action Name)
kind = "input"                   # "input" (external trigger) | "internal" (system) | "output" (event, no CSDL entry)
from = ["State1"]                # States this action can fire from
to = "State2"                    # Target state (omit for self-loops)
guard = "attempt_count < 3"      # Precondition (optional)
effect = "increment attempt_count"  # Side effect (optional): "increment var", "decrement var", "set var true/false", "trigger integration_name"
params = ["reason", "priority"]  # Parameters passed by caller (optional)
hint = "Submit for processing."  # Human-readable description (optional)

# Self-loop action (stays in same state)
[[action]]
name = "AddNote"
kind = "input"
from = ["State1", "State2"]      # Can fire from multiple states
# no "to" field = self-loop
params = ["note_text"]
hint = "Add a note without changing state."

# Safety invariants
[[invariant]]
name = "FinalIsFinal"
when = ["State3"]                # States where this is checked (empty = all)
assert = "no_further_transitions"  # Terminal state

[[invariant]]
name = "MustHaveAttempts"
when = ["State2"]
assert = "attempt_count > 0"

# Liveness properties (the system must eventually reach these states)
[[liveness]]
name = "EventuallyComplete"
from = ["State1"]
reaches = ["State3"]

# WASM integrations (for external API calls / side effects)
[[integration]]
name = "call_external_api"       # Internal name
trigger = "call_external_api"    # Matches the "trigger" in action effect
type = "wasm"                    # Always "wasm"
module = "http_fetch"            # WASM module name (built-in or custom)
on_success = "CallSucceeded"     # Action to fire on success
on_failure = "CallFailed"        # Action to fire on failure
# Extra config keys passed to the module:
url = "https://api.example.com/{param}"
method = "POST"
```

### IOA rules

- `[automaton]` table header — not bare `automaton EntityName`
- `initial` field — not `initial_state`
- `kind = "output"` actions do NOT get CSDL entries
- Self-loop actions: include `from` but omit `to`
- Every `on_success` / `on_failure` callback must itself be a defined `[[action]]`
- Integration `trigger` name must match the `effect = "trigger <name>"` in the triggering action

---

## CSDL model format

The CSDL is the OData projection of your IOA spec. It must match exactly.

```xml
<?xml version="1.0" encoding="utf-8"?>
<edmx:Edmx Version="4.0" xmlns:edmx="http://docs.oasis-open.org/odata/ns/edmx">
  <edmx:DataServices>
    <Schema Namespace="MyApp" xmlns="http://docs.oasis-open.org/odata/ns/edm">

      <EntityType Name="EntityName">
        <Key><PropertyRef Name="id"/></Key>
        <Property Name="id" Type="Edm.String" Nullable="false"/>
        <Property Name="state" Type="Edm.String" Nullable="false"/>
        <!-- Domain properties -->
        <Property Name="title" Type="Edm.String"/>
        <Property Name="attempt_count" Type="Edm.Int32"/>
        <Property Name="is_flagged" Type="Edm.Boolean"/>
      </EntityType>

      <!-- Only input and internal actions — NOT output actions -->
      <Action Name="Submit" IsBound="true">
        <Parameter Name="bindingParameter" Type="MyApp.EntityName"/>
        <Parameter Name="reason" Type="Edm.String"/>
        <Parameter Name="priority" Type="Edm.String"/>
      </Action>

      <EntityContainer Name="Default">
        <EntitySet Name="EntityNames" EntityType="MyApp.EntityName"/>
      </EntityContainer>

    </Schema>
  </edmx:DataServices>
</edmx:Edmx>
```

### CSDL-IOA mapping rules

| IOA | CSDL | Rule |
|-----|------|------|
| `[automaton].name` | `EntityType Name` | Must be identical |
| `[automaton].states` | Annotation `StateMachine.States` | Same values, same order |
| `[automaton].initial` | Annotation `StateMachine.InitialState` | Must match |
| `[[action]]` with `kind = "input"` or `"internal"` | `<Action>` element | Must exist |
| `[[action]]` with `kind = "output"` | Nothing | Do NOT create CSDL action |
| `[[action]].from` | `ValidFromStates` annotation | Must list same states |
| `[[action]].to` | `TargetState` annotation | Must match (omit for self-loops) |
| `[[action]].params` | `<Parameter>` elements | After `bindingParameter` |
| `type = "counter"` state var | `Edm.Int32` property | |
| `type = "bool"` state var | `Edm.Boolean` property | |

### CSDL rules

- Every `<Action>` must have `IsBound="true"` with a `bindingParameter` of the entity type
- EntitySet name is typically the plural of EntityType name
- Every entity MUST have `id` (key) and `state` properties
- Namespace is for OData routing — doesn't need to match IOA but be consistent

---

## Cedar policies

Cedar controls who can perform which actions. Every action is Cedar-gated.

```cedar
// Allow any agent to create and read bookmarks
permit(
    principal,
    action in [Action::"Create", Action::"Archive", Action::"Read"],
    resource is Bookmark
);

// Only the owner can archive
permit(
    principal,
    action == Action::"Archive",
    resource is Bookmark
) when {
    principal.id == resource.owner_id
};

// Supervisors can do anything
permit(
    principal,
    action,
    resource is Bookmark
) when {
    principal.agent_type == "supervisor"
};
```

Submit at runtime:
```python
temper.submit_policy("bookmark-access", cedar_text)
```

---

## WASM integrations

When an action needs to call an external API or perform computation, that logic lives in a WASM module.

### Built-in module: `http_fetch`

For simple HTTP calls, use the built-in `http_fetch` module. No custom code needed.

```toml
[[integration]]
name = "fetch_data"
trigger = "fetch_data"
type = "wasm"
module = "http_fetch"
on_success = "FetchSucceeded"
on_failure = "FetchFailed"
url = "https://api.example.com/{param}"
method = "GET"
```

The callback action receives `{"status_code": "200", "body": "...response..."}` as params.

### Custom WASM modules

For complex logic, write a Rust WASM module:

```rust
// wasm/my_module/src/lib.rs
use temper_wasm_sdk::prelude::*;

#[unsafe(no_mangle)]
pub extern "C" fn run(_ctx_ptr: i32, _ctx_len: i32) -> i32 {
    let result = (|| -> Result<(), String> {
        let ctx = Context::from_host()?;

        // Read entity state
        let fields = ctx.entity_state.get("fields").cloned().unwrap_or(json!({}));

        // Make HTTP call
        let resp = ctx.http_call("POST", "https://api.example.com", &headers, &body)?;

        // Create other entities or dispatch actions
        ctx.create_entity("OtherEntities", json!({...}))?;
        ctx.dispatch_action("OtherEntities", "id", "DoSomething", json!({}))?;

        // Signal success (triggers on_success callback action)
        set_success_result("CallbackAction", &json!({"key": "value"}));
        Ok(())
    })();

    if let Err(e) = result {
        set_error_result(&e);  // Triggers on_failure callback action
    }
    0
}
```

Compile and upload:
```python
# Option A: compile from source (if sandbox has Rust)
temper.compile_wasm("my_module", rust_source_code)

# Option B: upload pre-compiled binary
temper.upload_wasm("my_module", wasm_base64_string)
```

---

## Updating an existing app

To add new actions, states, or entity types to a running app, edit the specs and resubmit.

```python
# 1. Read the current spec
current = temper.show_spec("EntityName")

# 2. Modify the IOA (add action, add state, etc.)
updated_ioa = """..."""  # Full updated spec

# 3. Update the CSDL to match
updated_csdl = """..."""  # Full updated model

# 4. Resubmit — platform re-verifies
temper.submit_specs({
    "entity.ioa.toml": updated_ioa,
    "model.csdl.xml": updated_csdl
})
```

**Important:** Resubmission replaces the spec. Existing entities keep their current state. New actions become available immediately. Removed actions stop working. State renames can orphan existing entities — avoid renaming states that have entities in them.

---

## Evolving an app (the evolution loop)

Temper records failed intents automatically. When someone tries an action that doesn't exist, or tries to transition from an invalid state, the platform logs it.

```python
# 1. Check for unmet intents
trajectories = temper.get_trajectories(failed_only=True)

# 2. Read system-generated improvement suggestions
insights = temper.get_insights()

# 3. Design a spec change to handle the unmet intent

# 3.5 Record why you made the change
temper.write(
    "/apps/my-app/adrs/NNN-change-description.md",
    adr_content,
    {"workspace": "default", "mime_type": "text/markdown"},
)

# 4. Submit the updated spec
# 5. Retry — should now succeed
```

This is how apps grow: usage reveals gaps, agents close the gaps by evolving specs, humans approve via Cedar governance.

---

## app.toml manifest

```toml
[app]
name = "my-app"
version = "0.1.0"
description = "What this app does"
namespace = "MyApp"

[dependencies]
paw-agent = "*"    # This app requires paw-agent to be installed first
paw-fs = "*"       # And paw-fs
```

Dependencies are resolved in order during `install_app`. If a dependency isn't installed, the platform installs it first.

---

## Verification

Every spec goes through a verification cascade before the entity type becomes usable:

| Level | What it checks |
|-------|---------------|
| L0 | Syntax, reachability, inductiveness, dead guards |
| L1 | Model checking (finds counterexample traces that violate invariants) |
| L2 | Simulation with fault injection |
| L3 | Property testing with random action sequences |

If verification fails, the entity type is locked (HTTP 423 on all operations). Read the failure details, fix the spec, resubmit.

### Common verification errors

| Error | Meaning | Fix |
|-------|---------|-----|
| Dead guard | Guard condition can never be true | Check counter bounds, contradictory conditions |
| Unreachable state | No action sequence leads to this state | Add missing transition or remove unused state |
| Non-inductive invariant | A transition reaches the state without establishing the condition | Add guard to the offending transition |
| L1 counterexample | Model checker found a violating trace | Follow the trace — last transition is the problem |
| Deadlock | Entity stuck with no valid actions | Add transition out, or mark terminal with `no_further_transitions` |

---

## Governance

All spec submissions and integration calls are Cedar-gated. If Cedar denies:

1. You get an `AuthorizationDenied` error with a decision ID (`PD-xxx`)
2. Surface the decision to the human: "Action denied. Decision `PD-xxx` pending approval."
3. Poll for resolution: `temper.poll_decision(decision_id)`
4. Once approved, retry the action

You cannot self-approve. The human approves via the Observe UI or Discord buttons.

---

## Discovering what exists

Before building, check what's already installed:

```python
# What entity types are registered?
temper.specs()

# What apps are installed?
apps = temper.list("Apps", "Status eq 'Installed'")

# Read an app's guide for architecture context
guide = temper.read("/apps/my-app/APP.md")

# What actions are available from a given state?
spec.actions_from("EntityType", "CurrentState")
```

**Always check `temper.specs()` before assuming an entity type exists or doesn't exist.**

---

## Example: building a real app at runtime

Scenario: you need a simple task tracker.

```python
# 1. Design
ioa = """
[automaton]
name = "Task"
states = ["Open", "InProgress", "Done", "Cancelled"]
initial = "Open"

[[action]]
name = "Create"
kind = "input"
from = ["Open"]
to = "Open"
params = ["title", "assignee"]
hint = "Create a new task."

[[action]]
name = "Start"
kind = "input"
from = ["Open"]
to = "InProgress"
hint = "Begin working on the task."

[[action]]
name = "Complete"
kind = "input"
from = ["InProgress"]
to = "Done"
params = ["summary"]
hint = "Mark task as complete."

[[action]]
name = "Cancel"
kind = "input"
from = ["Open", "InProgress"]
to = "Cancelled"
params = ["reason"]
hint = "Cancel the task."

[[action]]
name = "Reopen"
kind = "input"
from = ["Done"]
to = "Open"
hint = "Reopen a completed task."

[[invariant]]
name = "CancelledIsFinal"
when = ["Cancelled"]
assert = "no_further_transitions"

[[liveness]]
name = "EventuallyDone"
from = ["Open"]
reaches = ["Done", "Cancelled"]
"""

csdl = '''<?xml version="1.0" encoding="utf-8"?>
<edmx:Edmx Version="4.0" xmlns:edmx="http://docs.oasis-open.org/odata/ns/edmx">
  <edmx:DataServices>
    <Schema Namespace="TemperPaw" xmlns="http://docs.oasis-open.org/odata/ns/edm">
      <EntityType Name="Task">
        <Key><PropertyRef Name="id"/></Key>
        <Property Name="id" Type="Edm.String" Nullable="false"/>
        <Property Name="state" Type="Edm.String" Nullable="false"/>
        <Property Name="title" Type="Edm.String"/>
        <Property Name="assignee" Type="Edm.String"/>
        <Property Name="summary" Type="Edm.String"/>
        <Property Name="reason" Type="Edm.String"/>
      </EntityType>
      <Action Name="Create" IsBound="true">
        <Parameter Name="bindingParameter" Type="TemperPaw.Task"/>
        <Parameter Name="title" Type="Edm.String"/>
        <Parameter Name="assignee" Type="Edm.String"/>
      </Action>
      <Action Name="Start" IsBound="true">
        <Parameter Name="bindingParameter" Type="TemperPaw.Task"/>
      </Action>
      <Action Name="Complete" IsBound="true">
        <Parameter Name="bindingParameter" Type="TemperPaw.Task"/>
        <Parameter Name="summary" Type="Edm.String"/>
      </Action>
      <Action Name="Cancel" IsBound="true">
        <Parameter Name="bindingParameter" Type="TemperPaw.Task"/>
        <Parameter Name="reason" Type="Edm.String"/>
      </Action>
      <Action Name="Reopen" IsBound="true">
        <Parameter Name="bindingParameter" Type="TemperPaw.Task"/>
      </Action>
      <EntityContainer Name="Default">
        <EntitySet Name="Tasks" EntityType="TemperPaw.Task"/>
      </EntityContainer>
    </Schema>
  </edmx:DataServices>
</edmx:Edmx>'''

# 2. Submit
temper.submit_specs({"task.ioa.toml": ioa, "model.csdl.xml": csdl})

# 3. Policy
temper.submit_policy("task-access", '''
permit(principal, action, resource is Task);
''')

# 4. Use
task = temper.create("Tasks", {"title": "Fix login bug", "assignee": "swe-01"})
temper.action("Tasks", task["entity_id"], "Start", {})
temper.action("Tasks", task["entity_id"], "Complete", {"summary": "Fixed null check in auth flow"})
```
