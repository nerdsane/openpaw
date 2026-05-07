# Proof Report: 067 - Paw Patrol Evaluation Failure Rework Hot-load

## Date
2026-05-07

## Scope
Paw Patrol only. No Railway image redeploy, no `paw-agent` hot-load, and no Discord/channel module changes.

## Trigger
A live WorkCycle rework was independently approved, but its automated EvaluationRun failed on `cargo fmt --check`. The failure was useful: it exposed that `EvaluationRun.Fail` dead-ended the WorkCycle as `Failed` instead of sending normal low-risk code-quality failures back through the implementer/reviewer/evaluator loop.

## Change
Updated `review_gate_lifecycle` so `EvaluationRun.Fail` behaves like reviewer feedback when the WorkCycle is still in `Reviewing`:

```text
EvaluationRun.Fail
        |
        v
WorkCycle.RequestChanges
        |
        v
work_cycle_lifecycle queues a new WorkerRun
        |
        v
implementer -> reviewer -> evaluator -> ProofPacket
```

Unexpected evaluation failures outside `Reviewing` still fail visibly via `WorkCycle.Fail`.

## Verification
Local checks:

```text
cargo fmt --check
git diff --check
cargo test -p paw-codex-worker -- --nocapture
cargo test -p temperpaw --test paw_patrol_foundation -- --nocapture
cargo test -p temperpaw --test datadog_monitor_config -- --nocapture
npm --prefix dashboard run check
npm --prefix dashboard run build
os-apps/paw-patrol/wasm/build.sh
```

Key regression:

```text
evaluation_failures_requeue_rework_instead_of_dead_ending_the_cycle ... ok
```

Production hot-load:

```text
POST /api/wasm/modules/review_gate_lifecycle
module_name=review_gate_lifecycle
sha256_hash=7a4336be6dea28af2b8bbf4e66c4c3484f8058d8ca9298ec7cb8143664505909
cached=true
```

Production health after hot-load:

```json
{
  "status": "ready",
  "discord": {
    "status": "connected",
    "configured": true,
    "connected": true,
    "desired_state": "connected",
    "connection_state": "Connected",
    "last_error": null,
    "next_retry_at": null
  }
}
```

## Residual Risk
The already-failed live WorkCycle remains failed because the old module handled that event before this fix was hot-loaded. The next fresh WorkCycle uses the new behavior.
