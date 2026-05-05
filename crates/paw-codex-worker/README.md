# paw-codex-worker

Local Mac mini executor for `paw-patrol` WorkerRuns.

The worker connects outbound to the TemperPaw/Temper control plane, watches
Temper event streams, claims configured `local_codex` WorkerRuns, runs work in a
local checkout or worktree, and self-reports through Temper actions. It uses the
local Codex CLI and ChatGPT/Codex auth, not a raw OpenAI API key.

## Safe Local Test

Run these commands from the TemperPaw repo/worktree root.

Run against a local TemperPaw server with execution disabled:

```sh
TEMPER_URL=http://127.0.0.1:3497 \
TEMPER_TENANT=default \
WORKER_ID=mac-mini-codex-prod \
WORKER_TOKEN="$TEMPER_API_KEY" \
REPO_ROOT="$(pwd)" \
WORKSPACE_ROOT=/Users/seshendranalla/Development/temperpaw-worktrees \
PAW_CODEX_ENABLE_EXECUTION=0 \
cargo run -p paw-codex-worker
```

Then create a `RepoGraphSnapshot` and dispatch `RepoGraphSnapshot.StartScan`.
The worker should claim the generated WorkerRun, run the repo-health sweep,
dispatch `RepoGraphSnapshot.ScanComplete`, self-report `WorkerRun.ReportDone`,
auto-review/evaluate that repo-sweep run only, and leave a visual ProofPacket.

Before starting the long-running worker, run the doctor. It checks the repo
paths, Codex binary, OData API, and event stream using the same environment:

```sh
TEMPER_URL=http://127.0.0.1:3497 \
TEMPER_TENANT=default \
WORKER_ID=mac-mini-codex-prod \
WORKER_TOKEN="$TEMPER_API_KEY" \
REPO_ROOT="$(pwd)" \
WORKSPACE_ROOT=/Users/seshendranalla/Development/temperpaw-worktrees \
cargo run -p paw-codex-worker -- doctor
```

Set `PAW_CODEX_ENABLE_EXECUTION=1` only when you want the worker to invoke
`codex exec` for non-sweep WorkerRuns and their independent ReviewRuns. The
reviewer invocation is a fresh Codex prompt and must return one explicit marker:
`VERDICT: approve`, `VERDICT: request_changes`, or `VERDICT: escalate`.

With execution enabled, non-sweep EvaluationRuns run local shell commands from
`PAW_CODEX_EVAL_COMMANDS`, one command per line. If unset, the worker runs:

```sh
cargo test -p temperpaw --test paw_patrol_foundation -- --nocapture
```

The Codex CLI must already be signed in with ChatGPT/Codex auth on the Mac mini.
`WORKER_ID` must match Patrol's configured `local_codex_worker_id`; the default
for local smoke and production setup is `mac-mini-codex-prod`.

## Deterministic End-to-End Smoke

This exercises the non-sweep implementation, review, evaluation, and proof loop
without API billing by using the checked-in fake Codex fixture.

Run from the TemperPaw repo/worktree root. The examples use `jq` to read OData
entity IDs from JSON responses.

The one-command version is:

```sh
crates/paw-codex-worker/scripts/deterministic-smoke.sh
```

It rebuilds the current `paw-patrol` WASM modules before booting the local
control plane so the smoke cannot accidentally use stale ignored `.wasm`
artifacts.

On success, the script prints the entity IDs and writes a proof bundle containing
`summary.json`, `proof.json`, `proof.md`, and `proof.svg`. By default that proof
bundle goes to `/tmp/paw-patrol-smoke-proof-*`; set `PROOF_DIR=/path/to/dir` to
choose a stable location.

The first cold run may take several minutes because
`TEMPERPAW_WASM_STARTUP_POLICY=build` compiles bundled WASM modules before the
local server starts accepting OData requests.

In one terminal, boot a local control plane:

```sh
TEMPERPAW_WASM_STARTUP_POLICY=build \
PORT=3551 \
TEMPER_API_KEY=patrol-smoke \
PAW_TENANT=patrol_smoke \
TURSO_URL=file:/tmp/paw-patrol-smoke.db \
cargo run -p temperpaw
```

Create and submit a PatrolRequest:

```sh
REQUEST_ID=$(curl -sS -X POST \
  -H 'Authorization: Bearer patrol-smoke' \
  -H 'Content-Type: application/json' \
  http://127.0.0.1:3551/tdata/PatrolRequests \
  -d '{}' | jq -r '.entity_id')

curl -sS -X POST \
  -H 'Authorization: Bearer patrol-smoke' \
  -H 'Content-Type: application/json' \
  "http://127.0.0.1:3551/tdata/PatrolRequests('${REQUEST_ID}')/TemperPaw.Patrol.Submit" \
  -d '{"source":"codex-smoke","request_text":"Produce a visual proof packet after the worker completes.","requester_id":"codex-smoke"}'
```

Then run the fake local worker:

```sh
TEMPER_URL=http://127.0.0.1:3551 \
TEMPER_TENANT=patrol_smoke \
WORKER_ID=mac-mini-codex-prod \
WORKER_TOKEN=patrol-smoke \
REPO_ROOT="$(pwd)" \
WORKSPACE_ROOT=/Users/seshendranalla/Development/temperpaw-worktrees \
CODEX_BIN="$(pwd)/crates/paw-codex-worker/fixtures/fake-codex.sh" \
PAW_CODEX_ENABLE_EXECUTION=1 \
PAW_CODEX_POLL_ON_START=1 \
PAW_CODEX_EVAL_COMMANDS='test -f .paw-fake-codex-implementation' \
cargo run -p paw-codex-worker
```

Expected result: the PatrolRequest reaches `Linked`, WorkerRun reaches `Done`,
ReviewRun reaches `Approved`, EvaluationRun reaches `Passed`, ProofPacket
reaches `Ready`, and WorkCycle plus FactoryCase reach `Complete`.

Stop the worker with `Ctrl-C` after the proof is ready. The worker creates a
temporary git worktree under `WORKSPACE_ROOT`; remove it after collecting any
evidence you need.

## Webhook Intake Smoke

This exercises the external trigger boundary. The HTTP webhook listener creates
`WebhookEvent` entities and dispatches only `TemperPaw.Ingest.Received`; the
rest of the flow is handled by `paw-ingest` and `paw-patrol` WASM.

Run from the TemperPaw repo/worktree root:

```sh
crates/paw-codex-worker/scripts/webhook-intake-smoke.sh
```

The script rebuilds `paw-ingest` and `paw-patrol` WASM, boots a local
TemperPaw control plane, registers Patrol webhook routes, posts to
`/triggers/webhook/patrol-request`, `/triggers/webhook/patrol-datadog`,
`/triggers/webhook/patrol-github`, and `/triggers/webhook/patrol-discord`, then
waits for the resulting PatrolRequest, Datadog Signal, GitHub Signal, and
Discord Signal to reach `Linked`.

On success, it writes a proof bundle with `summary.json`,
`request-webhook-event.json`, `datadog-webhook-event.json`,
`github-webhook-event.json`, `discord-webhook-event.json`,
`patrol-request.json`, `datadog-signal.json`, `github-signal.json`,
`discord-signal.json`, `proof.md`, and `webhook-intake.svg`. By default that
proof bundle goes to `/tmp/paw-patrol-webhook-smoke-proof-*`; set
`PROOF_DIR=/path/to/dir` to choose a stable location.

## Repo Sweep And Brief Smoke

This exercises the maintenance side of Patrol: RepoGraphSnapshot, local worker
repo-health scan, QualityFinding/SecurityFinding fan-out, automatic repo-sweep
review/evaluation, final ProofPacket, and a DailyBrief visual rollup.
Fresh Patrol installs also seed `patrol-default-daily-maintenance`, an active
daily PatrolSchedule that creates the same RepoGraphSnapshot and DailyBrief
entities through Temper `schedule_at` transitions.

Run from the TemperPaw repo/worktree root:

```sh
crates/paw-codex-worker/scripts/repo-sweep-brief-smoke.sh
```

On success, the script writes a proof bundle with `summary.json`,
`repo-graph.json`, `proof.json`, `proof.md`, `proof.svg`, and
`daily-brief.svg`. By default that proof bundle goes to
`/tmp/paw-patrol-repo-smoke-proof-*`; set `PROOF_DIR=/path/to/dir` to choose a
stable location.

Expected result: RepoGraphSnapshot reaches `Ready`, WorkCycle reaches
`Complete`, ReviewRun reaches `Approved`, EvaluationRun reaches `Passed`,
ProofPacket reaches `Ready`, and DailyBrief reaches `Ready`.

## Acceptance Harness

Use this when you want one command that leaves a single acceptance proof bundle
for a human or another agent to review:

```sh
crates/paw-codex-worker/scripts/paw-patrol-acceptance.sh quick
```

`quick` runs syntax checks, a CI action runtime smoke, formatting, diff
whitespace checks, `cargo check`, the Patrol foundation suite, worker tests,
production preflight, Railway discovery preflight, and preflight diff smoke.
It also proves the GitHub cutover gate that blocks an unmerged clean/green
TemperPaw PR until `CONFIRM_TEMPERPAW_PR_OK=1` is explicitly set. It writes
`index.html`, `summary.json`, `proof.md`, and `acceptance.log` under
`/tmp/paw-patrol-acceptance-*`.

For a full local acceptance pass that also runs the live E2E smokes:

```sh
crates/paw-codex-worker/scripts/paw-patrol-acceptance.sh live
```

`live` adds deterministic implementation, webhook intake, repo-sweep/brief, and
production-readiness smokes, each in a stable subdirectory of the acceptance
proof bundle. Open `index.html` in that bundle for a browser-readable visual
review surface with links to logs, JSON, proof markdown, and generated SVGs.

## Production Readiness

First run the non-mutating preflight. It writes a small proof bundle with
`summary.json`, `proof.md`, `operator-handoff.md`, `preflight.svg`,
`railway-candidates.json`, and `human_blockers` so the operator can see which
Railway project/service, launchd, webhook, and token gates still need human
input. It does not mutate Railway, launchd, or Temper:

```sh
crates/paw-codex-worker/scripts/production-preflight.sh
```

When comparing an earlier preflight to the current one, use the read-only diff
helper to produce `summary.json`, `proof.md`, and `preflight-diff.svg` showing
resolved blockers, new blockers, unchanged blockers, gate drift, and Railway
candidate drift:

```sh
crates/paw-codex-worker/scripts/production-preflight-diff.sh \
  /tmp/previous-preflight/summary.json \
  /tmp/current-preflight/summary.json
```

Use `STRICT=1` when blocked gates should make the command fail, for example in
a final cutover checklist:

```sh
STRICT=1 \
TEMPER_URL=https://your-railway-temperpaw.example \
WORKER_TOKEN="$TEMPER_WORKER_TOKEN" \
PATROL_OPERATOR_TOKEN="$TEMPER_OPERATOR_TOKEN" \
CONFIRM_LOCAL_CODEX_WORKER_ID=mac-mini-codex-prod \
CONFIRM_TEMPER_PIN_OK=1 \
CONFIRM_TEMPERPAW_PR_OK=1 \
crates/paw-codex-worker/scripts/production-preflight.sh
```

Use the guarded readiness script before loading the Mac mini daemon. It builds
the release worker, runs `paw-codex-worker doctor`, and can render or install
the launchd plist only when explicitly requested. It does not print `WORKER_TOKEN`.
For the full production cutover sequence, required human inputs, evidence gates,
webhook-secret setup, and rollback path, use
`docs/runbooks/paw-patrol-production-cutover.md`.

To dry-run the full guarded path against a live local TemperPaw control plane,
without touching launchd or production services:

```sh
crates/paw-codex-worker/scripts/production-readiness-smoke.sh
```

On success, the smoke writes `summary.json`, `proof.md`,
`production-readiness.log`, and a rendered launchd plist under
`/tmp/paw-patrol-production-readiness-proof-*`. It checks OData, the event
stream, fake Codex availability, a guarded `codex exec` doctor smoke,
`PAW_CODEX_ENABLE_EXECUTION=0`, plist rendering, and that the worker token was
not printed to the readiness log.

```sh
TEMPER_URL=https://your-railway-temperpaw.example \
TEMPER_TENANT=default \
WORKER_ID=mac-mini-codex-prod \
WORKER_TOKEN="$TEMPER_WORKER_TOKEN" \
REPO_ROOT=/Users/seshendranalla/Development/temperpaw \
WORKSPACE_ROOT=/Users/seshendranalla/Development/temperpaw-worktrees \
CODEX_BIN=/Users/seshendranalla/.local/bin/codex \
PAW_CODEX_ENABLE_EXECUTION=0 \
PAW_CODEX_DOCTOR_EXEC_SMOKE=1 \
crates/paw-codex-worker/scripts/production-readiness.sh
```

`PAW_CODEX_DOCTOR_EXEC_SMOKE=1` runs a tiny
`codex exec --skip-git-repo-check` prompt in a temporary directory. Use it
before launchd so the doctor proves the Mac mini account has a working Codex
auth/session, not just a binary on `PATH`.

When the doctor passes, render the exact plist:

```sh
WRITE_LAUNCHD_PLIST=1 \
TEMPER_URL=https://your-railway-temperpaw.example \
WORKER_TOKEN="$TEMPER_WORKER_TOKEN" \
PAW_CODEX_DOCTOR_EXEC_SMOKE=1 \
crates/paw-codex-worker/scripts/production-readiness.sh
```

Load it only after reviewing the rendered plist:

```sh
WRITE_LAUNCHD_PLIST=1 \
INSTALL_LAUNCHD=1 \
TEMPER_URL=https://your-railway-temperpaw.example \
WORKER_TOKEN="$TEMPER_WORKER_TOKEN" \
PAW_CODEX_DOCTOR_EXEC_SMOKE=1 \
crates/paw-codex-worker/scripts/production-readiness.sh
```

## launchd

Build and place the binary where the plist expects it:

```sh
cargo build -p paw-codex-worker --release
mkdir -p /Users/seshendranalla/.local/bin
install -m 755 target/release/paw-codex-worker /Users/seshendranalla/.local/bin/paw-codex-worker
```

Generate a concrete plist from the same environment you used for `doctor`:

```sh
TEMPER_URL=https://your-railway-temperpaw.example \
TEMPER_TENANT=default \
WORKER_ID=mac-mini-codex-prod \
WORKER_TOKEN="$TEMPER_WORKER_TOKEN" \
REPO_ROOT=/Users/seshendranalla/Development/temperpaw \
WORKSPACE_ROOT=/Users/seshendranalla/Development/temperpaw-worktrees \
CODEX_BIN=/Users/seshendranalla/.local/bin/codex \
PAW_CODEX_ENABLE_EXECUTION=0 \
PAW_CODEX_DOCTOR_EXEC_SMOKE=1 \
PAW_CODEX_POLL_ON_START=1 \
./target/release/paw-codex-worker launchd-plist > ~/Library/LaunchAgents/com.temperpaw.paw-codex-worker.plist
```

Then load it:

```sh
launchctl bootstrap gui/$(id -u) ~/Library/LaunchAgents/com.temperpaw.paw-codex-worker.plist
launchctl kickstart -k gui/$(id -u)/com.temperpaw.paw-codex-worker
```

Use `paw-codex-worker doctor` with the same plist environment before
`launchctl bootstrap`; any `fail` line should be fixed before the worker is
allowed to claim production WorkerRuns.

After launchd is loaded with `PAW_CODEX_ENABLE_EXECUTION=0`, run the guarded
observe-only proof. This creates a low-risk `RepoGraphSnapshot`, waits for the
Mac mini worker, independent reviewer, evaluation gate, final `ProofPacket`, and
`DailyBrief`, and writes `summary.json`, `proof.md`, and `observe-only.svg`.
The script refuses to write anything unless `ALLOW_PRODUCTION_WRITE=1` and the
operator confirms the launchd worker is still observe-only:

```sh
ALLOW_PRODUCTION_WRITE=1 \
CONFIRM_PAW_CODEX_ENABLE_EXECUTION_0=1 \
TEMPER_URL=https://your-railway-temperpaw.example \
TEMPER_TENANT=default \
PATROL_OPERATOR_TOKEN="$TEMPER_OPERATOR_TOKEN" \
EXPECTED_WORKER_ID=mac-mini-codex-prod \
crates/paw-codex-worker/scripts/production-observe-only.sh
```

To prove the same gate locally with fake Codex and no production writes:

```sh
crates/paw-codex-worker/scripts/production-observe-only-smoke.sh
```

The checked-in `launchd/com.temperpaw.paw-codex-worker.plist` remains a static
template, but `launchd-plist` is the safer production path because it renders
the actual `TEMPER_URL`, worker identity, repo paths, execution toggle, and
optional `PAW_CODEX_EVAL_COMMANDS` in one place.

Logs go to `/tmp/paw-codex-worker.out.log` and
`/tmp/paw-codex-worker.err.log`.
