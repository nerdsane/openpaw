# paw-heal

Self-healing monitoring loop. Integrates with Datadog to detect incidents, spawns SRE agents for triage, tracks fix through CI/CD merge and deployment, and verifies alert resolution.

## Entity Types

### Monitor
Datadog-backed watch for a self-healing target.

- **States**: Created -> Active <-> Paused -> Archived
- **Key actions**: `Configure` (dd_query, threshold, dd_monitor_id), `Activate`, `AlertFired`, `Tune` (adjust query/threshold), `Pause`, `Archive`
- **Counter**: `alert_count`

### AlertCycle
One self-healing attempt opened from a monitor alert. Full lifecycle from triage through deploy verification.

- **States**: Created -> Triaging -> Fixed -> Merging -> Deploying -> Verifying -> Resolved / Tuned / Failed
- **Key actions**: `Open` (spawns SRE agent), `HealComplete` (records PR), `BeginMerge`, `MergeComplete`, `DeployDetected`, `AlertResolved`, `TuneComplete`, `Escalate`
- **WASM modules**: `alert_opener` (spawns SRE), `cicd_initiator`, `cicd_merger` (polls PR merge readiness), `deployment_tracker` (polls GitHub deployments), `alert_verifier` (polls Datadog for resolution), `heal_reporter` (reports outcome)
- Polling actions (`CheckMergeReady`, `CheckDeployment`, `CheckAlertResolution`) self-loop until conditions are met.

### MonitorScan
Tracks a Datadog monitor bootstrap or PR-delta scan.

- **States**: Created -> Scanning -> Complete / Failed
- **Key actions**: `Configure` (project_harness_id, scan_type, commit_sha), `StartScan`, `ScanComplete`, `ScanFailed`
- **Counters**: `monitors_created`, `monitors_updated`

## Setup

Depends on `paw-agent` for SRE agent sessions. Requires Datadog secrets (`dd_api_key`, `dd_app_key`, `dd_site`) and GitHub token for CI/CD tracking.
