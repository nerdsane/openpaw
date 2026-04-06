# paw-compute

Manages persistent cloud VMs for developer agents. VMs survive agent restarts — code, dependencies, and state persist across sessions.

## Entity Types

### Computer
Long-lived Linux VM lifecycle.

- **States**: Created -> Provisioning -> Ready <-> Sleeping -> Destroyed (+ Checkpointing from Ready)
- **Key actions**: `Configure` (name, provider, cpu_cores, memory_gb, storage_gb, base_image, setup_script), `Provision`, `ProvisionComplete`, `Checkpoint`, `Sleep`, `Wake`, `Destroy`
- **Governance fields**: `tools_installed`, `credentials_scoped`, `network_allow`, `project_harness_id`

No WASM integrations in the spec — provisioning is handled by the session's `sandbox_provisioner` module in paw-agent.

## Setup

No dependencies. Create a Computer entity with `Configure`, then `Provision` to start the VM. The `sandbox_url` and `ssh_host` are recorded on `ProvisionComplete`.
