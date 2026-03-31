# Proof Report: 007 — SRE -> Developer Self-Heal Loop

## Date
2026-03-27

## Branch
`feat/openpaw-self-heal-loop-codex`

## What Was Proven
The full deep-sci-fi self-heal path completed end to end through the real Open Paw entities:

`ProjectHarness -> Monitor -> AlertCycle -> SRE agent -> Developer child agent -> repo fix -> validation -> push -> PR -> WorkCycle Complete -> AlertCycle Fixed`

This proof used the local sandbox path for both SRE and Developer (`http://127.0.0.1:3477`) and did not require Discord.

## Verification Flow
1. Created a `ProjectHarness` for `https://github.com/arni-labs/deep-sci-fi.git`.
2. Created a `Monitor` carrying a synthetic alert payload for the real `platform/npm ci` lockfile drift issue.
3. Created an `AlertCycle` from that alert.
4. Provisioned a `SRE` agent with the self-heal instructions and a local sandbox URL.
5. The SRE created a `WorkCycle`, spawned a `Developer` child in the same local sandbox, and waited for completion.
6. The Developer cloned the repo, reproduced the issue, repaired the lockfile, validated the fix, pushed a branch, and opened a PR.
7. The SRE marked the `WorkCycle` complete and the `AlertCycle` fixed.

## Verification Results
| Step | Expected | Actual | Status |
|------|----------|--------|--------|
| Project harness setup | Harness exists for deep-sci-fi | `ProjectHarnesses('019d2cc5-a901-7ab0-bd8e-34173c91a011')` is `Active` with `repo_url=https://github.com/arni-labs/deep-sci-fi.git` | PASS |
| Alert intake | Monitor and alert cycle exist with concrete reproduction details | `Monitors('019d2cc5-a915-7543-9e2c-fbcac1a508c8')` recorded the synthetic `npm ci` failure payload; `AlertCycles('019d2cc5-a927-7af2-896f-52b252c66f4a')` was created from it | PASS |
| SRE completion | SRE should finish with fixed status and PR URL | `Agents('019d2cc5-a92b-7db1-b0ec-7072f5b32a61')` reached `Completed` and returned `ALERT_CYCLE_STATUS=Fixed`, `WORK_CYCLE_STATUS=Complete`, `PR_URL=https://github.com/arni-labs/deep-sci-fi/pull/68` | PASS |
| Developer execution | Developer should repair the repo in a sandbox and produce a PR | `Agents('019d2cc6-35b2-7710-adad-21260111a8fd')` reached `Completed` in local sandbox `http://127.0.0.1:3477`, produced commit `6a1de00dee433d8e6ca5d078495aa0f2d4c9e5af`, and opened PR `#68` | PASS |
| Validation | WorkCycle should record successful checks | `WorkCycles('019d2cc5-f6df-7ba0-bc56-f7ce1ae72de5')` is `Complete` with `tests_passed=true` and validation results for `npm ci --no-fund --no-audit` and `npm run typecheck` | PASS |
| Final alert status | AlertCycle should be fixed and linked to PR | `AlertCycles('019d2cc5-a927-7af2-896f-52b252c66f4a')` is `Fixed` with `pr_url=https://github.com/arni-labs/deep-sci-fi/pull/68` and `commit_sha=6a1de00` | PASS |

## Key Artifacts
- ProjectHarness: `019d2cc5-a901-7ab0-bd8e-34173c91a011`
- Monitor: `019d2cc5-a915-7543-9e2c-fbcac1a508c8`
- AlertCycle: `019d2cc5-a927-7af2-896f-52b252c66f4a`
- SRE agent: `019d2cc5-a92b-7db1-b0ec-7072f5b32a61`
- Developer agent: `019d2cc6-35b2-7710-adad-21260111a8fd`
- WorkCycle: `019d2cc5-f6df-7ba0-bc56-f7ce1ae72de5`
- PR URL: `https://github.com/arni-labs/deep-sci-fi/pull/68`
- Commit SHA: `6a1de00dee433d8e6ca5d078495aa0f2d4c9e5af`

## What Worked
- SRE and Developer both ran in the local sandbox path instead of forcing a fresh E2B sandbox.
- The Developer reproduced the failure and used the bounded lockfile repair path rather than looping on a heavy install.
- Validation completed inside the repo with real commands.
- The GitHub workflow completed: branch push plus upstream PR creation.
- The `WorkCycle` and `AlertCycle` state machines reflected the successful outcome.

## Notes
- The Developer result text still mentioned `WorkCycle` "Reviewing" in one summary block, but the authoritative entity state and SRE result show `WorkCycle` is `Complete`.
- This proof covers the real self-heal loop and supersedes the earlier clone-only milestone.
