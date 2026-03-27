// paw-compute Cedar policies
//
// Computers are provisioned by agents (Paw) and used by developer agents.
// Humans/supervisors can manage any computer.

// Any authenticated principal can create and configure computers.
permit(
    principal,
    action in [
        Action::"Computer.Configure",
        Action::"Computer.Provision"
    ],
    resource
)
when { principal.authenticated == true };

// Callback actions (ProvisionComplete, ProvisionFailed, CheckpointComplete)
// are dispatched by WASM integrations running as admin.
permit(
    principal,
    action in [
        Action::"Computer.ProvisionComplete",
        Action::"Computer.ProvisionFailed",
        Action::"Computer.CheckpointComplete"
    ],
    resource
)
when { principal.principalKind == "admin" };

// Checkpoint, Sleep, Wake, Destroy — agents or supervisors.
permit(
    principal,
    action in [
        Action::"Computer.Checkpoint",
        Action::"Computer.Sleep",
        Action::"Computer.Wake",
        Action::"Computer.Destroy"
    ],
    resource
)
when { principal.authenticated == true };
