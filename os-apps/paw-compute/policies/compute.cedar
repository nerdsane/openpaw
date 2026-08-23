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

// Entity operations. The Computer row is the live agent<->sandbox mapping
// (no sidecar store), so attach = read-by-name. Read/list are open within
// the tenant (API access is already credential-gated); create requires an
// Agent principal. Note: `principal.authenticated` conditions are avoided —
// the deployed principal model does not carry that attribute, and Cedar
// treats a missing attribute as deny.
permit(
    principal,
    action in [
        Action::"read",
        Action::"list"
    ],
    resource is Computer
);

permit(
    principal is Agent,
    action == Action::"create",
    resource is Computer
);

// Exec: agents run commands on a computer's sandbox through the Exec entity —
// every invocation is a Cedar-gated, audited row (ADR-0002).
permit(
    principal is Agent,
    action in [
        Action::"create",
        Action::"read",
        Action::"list",
        Action::"Exec.Run"
    ],
    resource is Exec
);

// Exec callbacks (RunSucceeded, RunFailed) are dispatched by the
// computer_exec WASM integration running as admin.
permit(
    principal,
    action in [
        Action::"Exec.RunSucceeded",
        Action::"Exec.RunFailed"
    ],
    resource
)
when { principal.principalKind == "admin" };

// Allow computer_exec to make HTTP calls (Computer row loopback read,
// sandbox provider data plane).
permit(
    principal is Agent,
    action == Action::"http_call",
    resource is HttpEndpoint
) when {
    context.module == "computer_exec"
};

// Allow computer_exec to access secrets (temper_api_url, sandbox provider keys).
permit(
    principal is Agent,
    action == Action::"access_secret",
    resource is Secret
) when {
    context.module == "computer_exec"
};
