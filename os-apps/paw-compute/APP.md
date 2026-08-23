# paw-compute

Manages persistent cloud VMs for developer agents. VMs survive agent restarts — code, dependencies, and state persist across sessions.

## Entity Types

### Computer
Long-lived Linux VM lifecycle.

- **States**: Created -> Provisioning -> Ready <-> Sleeping -> Destroyed (+ Checkpointing from Ready)
- **Key actions**: `Configure` (name, provider, cpu_cores, memory_gb, storage_gb, base_image, setup_script), `Provision`, `ProvisionComplete`, `Checkpoint`, `Sleep`, `Wake`, `Destroy`
- **Governance fields**: `tools_installed`, `credentials_scoped`, `network_allow`, `project_harness_id`

Computer provisioning is handled by the session's `sandbox_provisioner` module in paw-agent.

### Exec
One governed shell command on a Computer's sandbox (ADR-0002).

- **States**: Created -> Running -> Succeeded | Failed (both terminal)
- **Walk**: create an Exec, dispatch `Run(computer_id, command, created_by)`. The `computer_exec` WASM integration resolves the Computer row, executes the command on its sandbox via the shared provider abstraction, and reports back — `RunSucceeded(exit_code, stdout_tail, stderr_tail)` or `RunFailed(error)`.
- **Audit fields**: `computer_id`, `command`, `exit_code`, `stdout_tail`, `stderr_tail` (8 KB tails), `error`, `created_by`

Security posture: Run and Exec entity operations are permitted for Agent principals only; the RunSucceeded/RunFailed callbacks are admin-only (WASM dispatch path). Provider credentials come from the trigger config overlay at runtime — harnesses never hold them. Every command is a Cedar-gated entity with a durable audit trail. Long-running/interactive commands are out of scope (v1).

## WASM Modules

- **computer_exec** — executes an Exec's command on the target Computer's sandbox (`wasm/computer_exec/`, built via `wasm/build.sh`).

## Setup

No dependencies. Create a Computer entity with `Configure`, then `Provision` to start the VM. The `sandbox_url` and `ssh_host` are recorded on `ProvisionComplete`. To run a command on a Ready computer, create an Exec and dispatch `Run`.
