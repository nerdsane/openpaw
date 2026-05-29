# ADR 0064: Directed Evolution Proof Diffs And Telemetry

## Status

Accepted

## Context

Directed Evolution proof runs must be reviewable from Temper/Genesis state without relying on chat narration. Variant generators currently commit app changes, but mutation entities only receive changed filenames and a diff reference. Telemetry evaluators also need stable Datadog join fields that line up with the runtime headers sent by simulated users and evaluators.

## Decision

- Variant-generator finalization stages the repo, captures the staged unified patch, commits, and returns `diff_patch` with `changed_files` and `diff_ref`.
- Worker evidence summaries include Datadog context derived from the WorkItem correlation graph, not only the WorkItem id.
- Worker prompts keep simulated users observational: they use the app and report journeys, while evaluator roles decide pass/fail.
- Default human episode setup treats Datadog runtime errors as a hard elimination metric alongside state/spec regressions.

## Consequences

- Genesis can materialize GitHub-like code diffs from mutation entities.
- Datadog queries are reproducible from evidence records and can be joined by tenant, episode, generation, variant, stage, trial, and work item.
- Missing Datadog evidence can be failed by the app router instead of remaining an opaque link.
