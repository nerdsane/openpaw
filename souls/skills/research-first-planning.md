# Research-First Planning

When you receive a non-trivial task, follow this sequence. Do not skip to implementation.

## Phase 1: Research (mandatory)

Before writing any code or making changes:

1. **Understand the codebase context**
   - `sandbox.read()` key files related to the task
   - `sandbox.bash("find ... | head")` to map relevant directory structure
   - `sandbox.bash("grep -rn ...")` to find related patterns and conventions

2. **Understand the domain context**
   - `temper.web_search()` for relevant documentation, APIs, or best practices
   - `temper.web_fetch()` to read specific documentation pages
   - Look for prior art, known pitfalls, and recommended approaches

3. **Check existing state**
   - `temper.list()` for related entities (Issues, WorkCycles, Memories)
   - `temper.recall_memory()` for relevant past context

## Phase 2: Plan (mandatory)

After research, write a concrete plan before implementing:

1. **State the approach** — what you will change and why
2. **List the files** — every file you expect to modify or create
3. **Identify risks** — what could go wrong, what you are unsure about
4. **Define done** — how you will validate the change works

Save the plan: `temper.save_memory("plan-{task}", plan_text)`

Present the plan in your response.

## Phase 3: Await Feedback

After presenting the plan, STOP. Wait for human feedback via steering or follow-up message. Do not proceed to implementation until you receive approval or direction.

If the human says "go ahead", "looks good", "proceed", or similar — move to Phase 4.
If the human provides corrections — update the plan and present again.

## Phase 4: Implement

Execute the plan step by step. If you discover the plan needs adjustment during implementation, note the deviation and why.

When done, call `temper.done(result_summary)` with a structured summary of what was accomplished.

## When to Skip This

You may skip straight to implementation for:
- Trivial changes (typo fixes, single-line config changes, simple renames)
- Emergency fixes where speed matters — implement but explain after
- When the human explicitly says "just do it" or "no need to plan"
- Quick lookups or information queries (not implementation tasks)
