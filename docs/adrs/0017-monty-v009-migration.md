# ADR-0017: Monty v0.0.9 Migration

## Status

Accepted

## Context

The `monty_repl` WASM module embeds Pydantic's Monty Python sandbox (v0.0.7, rev `bf7c7ef`) to execute agent-authored Python code. GPT-5 agents write standard Python patterns — `enumerate(x, start=1)`, `import json`, `max(items, key=...)` — that Monty v0.0.7 cannot interpret, blocking koto-wiki and other agent sessions.

Monty v0.0.9 (rev `c9802b5`, 2026-03-28) adds:
- Named single-kwarg support (fixes `enumerate(x, start=1)`)
- `json` module (fixes `import json`)
- `datetime` module
- `max()` kwargs/default
- PEP 448 generalised unpacking

The upgrade introduces breaking API changes to `MontyRepl` initialization, `PrintWriter` ownership semantics, and error return types.

## Decision

Upgrade the monty dependency from v0.0.7 to v0.0.9 and adapt the integration layer to the new API:

### 1. REPL Initialization

**Before:** `MontyRepl::new(code, filename, input_names, inputs, tracker, &mut print)` created a REPL and executed init code in a single call.

**After:** `MontyRepl::new(script_name, tracker)` creates a bare REPL. Globals (`temper`, `sandbox`) are injected via `feed_start("pass", inputs, print)` which returns `ReplProgress`. A new `drive_init_to_completion()` helper drives the progress to `Complete` to recover the initialized REPL.

**Rationale:** v0.0.9 separates construction from execution, providing a cleaner API for incremental code feeding. The inputs parameter moves from constructor to `feed_start`, using `Vec<(String, MontyObject)>` instead of separate `input_names`/`inputs` vectors.

### 2. Code Execution

**Before:** `repl.start(&code, &mut print)` began execution of a code snippet.

**After:** `repl.feed_start(&code, vec![], print)` — renamed method, inputs parameter added (empty for subsequent calls since globals are already set), `PrintWriter` passed by value instead of `&mut` reference.

### 3. PrintWriter Ownership

All `resume()` methods on `ReplFunctionCall`, `ReplOsCall`, `ReplNameLookup`, and `ReplResolveFutures` now take `PrintWriter<'_>` by value instead of `&mut PrintWriter`. The `BoundedOutputCollector` is still borrowed mutably to construct a fresh `PrintWriter::Callback(&mut collector)` at each call site.

### 4. Error Boxing

`ReplStartError<T>` is now returned inside a `Box` (`Err(Box<ReplStartError<T>>)`). Error paths that move out `.repl` unbox with `let e = *e;` before field access.

## Consequences

### Positive
- Unblocks standard Python patterns used by GPT-5 agents (`import json`, named kwargs, datetime)
- Tracks upstream Monty improvements — json module, datetime, PEP 448 unpacking
- `feed_start` inputs parameter is a cleaner API for global injection (single vec of tuples vs. two parallel vecs)

### Negative
- Fresh REPL setup now requires a mini event loop (`drive_init_to_completion`) since `feed_start` returns `ReplProgress` rather than a direct result
- Serialized REPL state from v0.0.7 sessions is incompatible — existing sessions will need a fresh REPL on next execution (gracefully handled by the existing fallback-to-fresh-REPL logic)

### Risks
- If Monty's `dump()`/`load()` format changed between versions, in-flight sessions with persisted state will fail to deserialize. The existing error handling creates a fresh REPL in this case, so sessions recover but lose accumulated state.
