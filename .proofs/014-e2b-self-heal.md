# Proof Report: 014 — E2B Self-Heal Loop

## Date

2026-03-27

## Branch / Commit

- Branch: `feat/openpaw-self-heal-loop-codex`
- Commit: working tree implementation

## Vision Target

This proof targets the `.vision` gap:

- `Scout → Developer → PR (self-heal) | ✅ Proven | Manually triggered with synthetic alert`

The missing proof is the sandbox environment itself: the loop needs to work in E2B, not just on the local sandbox.

## What Was Done

- Added an E2B-specific wrapper proof driver [`scripts/prove_e2b_self_heal.py`](/Users/seshendranalla/Development/openpaw-codex/scripts/prove_e2b_self_heal.py)
- Updated [`scripts/prove_self_heal_loop.py`](/Users/seshendranalla/Development/openpaw-codex/scripts/prove_self_heal_loop.py) so the same real self-heal flow can run in `local`, `auto`, or `e2b` sandbox modes

## Flow Diagram

```text
synthetic alert
    |
    v
Scout agent
    |
    v
Developer child agent
    |
    v
E2B sandbox provisioned
    |
    v
clone -> reproduce -> patch -> validate -> push -> PR
```

## What Is Intended To Be Proven

- E2B provisioning works for real remediation sessions.
- The Developer agent can use E2B as its governed computer rather than the local fallback.
- The self-heal loop can still reach a PR from E2B.

## Verification Flow

1. Start the daemon with `ANTHROPIC_API_KEY`, `E2B_API_KEY`, and `GITHUB_TOKEN`
2. Run `python3 scripts/prove_e2b_self_heal.py`
3. The wrapper invokes the existing self-heal proof with `--sandbox-mode e2b`

## Verification Results

- Executed against this branch with real `ANTHROPIC_API_KEY`, `E2B_API_KEY`, and `GITHUB_TOKEN` on `2026-03-28`.
- Observed governed state from the live E2B run:
  - `ProjectHarness`: `019d347d-0422-74d2-a5a7-cfbeb594262e`
  - `Monitor`: `019d347d-043d-7ec1-a7bd-11c44ff9210c`
  - `AlertCycle`: `019d347d-044e-7601-970f-37edeb479296`
  - `Scout` agent: `019d347d-0454-72d2-a6e1-66807cbe667c`
  - Scout sandbox URL: `https://49983-ir2lu0fg10p37ieaysloe.e2b.app`
  - `WorkCycle`: `019d347d-3f1f-7d13-9582-3f582144781e`
  - `Developer` child agent: `019d347d-7cde-7102-b862-a7916b0a2303`
- The `WorkCycle` wrote a concrete remediation plan and moved to `InProgress`.
- The child `Developer` agent inherited the same E2B-backed sandbox URL and executed real repo steps inside that governed sandbox:
  - `git clone https://github.com/arni-labs/deep-sci-fi.git`
  - `cd deep-sci-fi/platform && npm ci`
  - lockfile inspection and bounded remediation commands such as `rm -rf node_modules`
- This proves the remediation loop is using a real E2B computer rather than falling back to the local sandbox.
- An additional earlier E2B attempt on this branch reached a terminal failure state:
  - `AlertCycle`: `019d346a-9d1b-7010-bbab-0d26ec599583`
  - terminal status: `Failed`
  - recorded reason: Developer exhausted the 20-turn budget during remediation
- That earlier terminal run proves the E2B path can also converge into governed failure escalation rather than hanging forever.

## Honest Assessment Against Vision

- Proven by implementation:
  - The self-heal proof can now be forced into E2B mode instead of only local mode.
- Proven by execution:
  - E2B provisioning succeeds with real credentials on this branch.
  - Both Scout and Developer can execute against real E2B sandbox URLs while the `AlertCycle` and `WorkCycle` move through governed state.
  - Failure handling in E2B is real: at least one run escalated with a recorded remediation failure instead of silently stalling.
- Not proven by this report:
  - That a fresh PR was opened from an E2B-backed remediation run on the current branch tip.
  - That the full E2B loop closed from alert to successful `AlertCycle.Fixed` during this specific proof run.
- Still below vision:
  - The branch now proves governed E2B execution, but not yet a clean end-to-end E2B success case with PR creation and post-fix closure.
  - There is still no proof here for long-lived governed computers beyond the remediation session or for post-PR deploy verification.

## Artifacts

- [`scripts/prove_self_heal_loop.py`](/Users/seshendranalla/Development/openpaw-codex/scripts/prove_self_heal_loop.py)
- [`scripts/prove_e2b_self_heal.py`](/Users/seshendranalla/Development/openpaw-codex/scripts/prove_e2b_self_heal.py)
