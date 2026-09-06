# Implementation plan

The expected end state is the complete DSF software factory defined in spec.md. Each unit ends in a verifiable result; passing an early unit does not complete the effort.

1. Establish isolated current-main worktrees, private Foundry fork, committed contract/decisions and the Temper Intent/Effort chain. Inspect Genesis pins before modifying existing apps. Resolve provider/subscription access with actual probes and record any required human consent immediately.
2. Define the operational schema, formal model and executable invariant checks, and enforce agreement between them. Add stable resource/flow/participant identities and seeded DSF bindings. Prove invalid transitions and stale/replayed observations fail before implementing integrations.
3. Implement narrow provider, Datadog and DSF operational collection adapters and the governed recurring sync. Prove real reads, redaction, coverage, freshness and drift-to-Ask behavior. Preserve prior observations when sources fail.
4. Implement resource-owned operations and adapt Effort deployment selection. Preserve existing TemperPaw deployment behavior while moving callers to explicit targets. Prove idempotency, exact-revision verification and rollback/recovery under injected failures.
5. Add necessary DSF startup, operational telemetry and scoped repair corrections in its own worktree. Run migration/restart, route, DST and browser checks against disposable data, then use the resource operation to ship and verify the affected flow.
6. Adapt private Foundry hosting, machine auth, subscription connection, Temper MCP bootstrap, effort/run linkage, decision bridge and operational UI. Boot its real API/web/Postgres shape and run an agent inside Tensorlake. Exercise direct chat, decisions, files, suspension and recovery.
7. Connect observation-driven investigation/repair and isolated experiment lifecycles. Demonstrate one repair, two isolated variants, selection through delivery and cleanup. Verify automatic ongoing reconciliation after the interactive session ends.
8. Run independent reviews and required checks for every changed repository. Fix every finding, merge in dependency order, publish app deltas to Genesis, install pinned revisions, deploy private services and perform browser/provider/Datadog verification. Attach final evidence and costs to ARN-467 and close the verified Effort.

Keep operational notes current while working. Stop dependent production actions on a real denied authorization or provider consent requirement; continue independent implementation and tests. Never weaken gates to meet the morning target.
