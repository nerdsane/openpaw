# ADR-0003 — Copies as governed child Computers

Status: accepted (ARN-443 C)

## Context

The review panel copies the arni-big box and tears the copy down with raw
`tl sbx copy` / `tl sbx terminate` CLI calls. Those copies are ungoverned: no
Computer row, no audit trail, no automatic reaping — an orphaned copy just keeps
running. We want a copy to be a first-class governed entity whose lifecycle
(creation, teardown, reaping) is visible in the state machine.

## Decision

A copy IS a Computer whose lifecycle differs by STATE, not a second entity type
and not a flag.

- `Computer.Copy` (on a source, a `Ready` self-loop) uses the kernel's `spawn`
  effect to create a child Computer, carrying the source's machine_id + spec into
  the child's `ProvisionFromCopy` params via `copy_fields`.
- The child runs its OWN copy, ASYNCHRONOUSLY: `ProvisionFromCopy →
  computer_copy_start → CopyStarted → Copying → (poll loop) → CopyComplete →
  Leased`. A live-copy of a real box takes minutes — past the ~120s WASM invocation
  cap — so `computer_copy_start` only KICKS OFF the copy (a short-timeout POST that
  returns the new sandbox id) and `computer_copy_poll` health-checks readiness from
  a `Copying` `state_timeout` (`reset_on=["CopyPoll"]`) across invocations, the same
  pattern as the async Exec (ARN-443 D). A copy that never becomes ready
  (`CopyExpired`) is torn down through `Terminating` (its machine_id is the copy's).
  `CopyStarted` records `source_machine_id` (the parent reference; the parent's
  entity id is derivable by query, not precomputed) and a distinct `copy-…` name —
  never the source's name, which is an attach/resolution key.
- Children land in a distinct **Leased** state, never `Ready`. A lease
  `state_timeout` lives ONLY on `Leased`, so it reaps copies and can NEVER touch a
  source (sources stay `Ready`, `allow_indefinite`). `Heartbeat` renews the lease.
- `Destroy → Terminating → computer_terminate → Destroyed` actually tears the
  sandbox down (previously `Destroy` was a bare transition). `Destroy` is NOT
  allowed from `Provisioning`, where a child's machine_id is still the SOURCE's —
  destroying there would kill the source.

Each module is one concern on its own entity; no WASM dispatches transitions on
another machine. The choice of Option A (spawn + child's own copy) over a WASM
that materializes the child inline, and of a state over a flag/second-entity, is
what keeps this entity-first and the no-orphans guarantee structural.

## Consequences

- The panel's copies become auditable Computer rows; teardown and reaping are
  declared transitions, not CLI side effects.
- A copy is exec-able only once `Ready`; a `Leased` copy is not (a forward pointer
  for ADR-0004/D: the exec Ready-gate will widen to accept `Leased`).
- A copy sandbox leaked by a partial copy failure is caught by the panel's
  stale-copy reaper (an accepted residual; C5 in the effort decision log defers
  the real-provider proof to the Genesis-publish verification).
