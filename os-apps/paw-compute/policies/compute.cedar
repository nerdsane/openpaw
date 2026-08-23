// paw-compute Cedar policies
//
// Computers are provisioned by agents (Paw) and used by developer agents.
// Third-party harness sessions attach the existing computer by reading the
// Computer row by name; every Exec invocation is a gated, audited row.
//
// Action ids are listed in both namespaced (Entity.Action) and bare forms
// until the deployed engine's naming is pinned down by live dispatch.
// Shape notes (2026-08-23): permits use only predicates that match on the
// deployed principal model — `principal is Agent` / `principal is Admin`
// and `principal.agent_type`. Conditions on `principal.authenticated` or
// `principal.principalKind` never match (the attributes are absent and
// Cedar treats a missing attribute as deny), which is why the original
// lifecycle permits denied every live principal.

// Admins can do anything with computers.
permit(
    principal is Admin,
    action,
    resource is Computer
);

// Entity operations. The Computer row is the live agent<->sandbox mapping
// (no sidecar store), so attach = read-by-name. Read/list are open within
// the tenant (API access is already credential-gated); create requires an
// Agent principal.
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

// Lifecycle: any Agent on this tenant may drive the computer.
permit(
    principal is Agent,
    action in [
        Action::"Computer.Configure", Action::"Configure",
        Action::"Computer.Provision", Action::"Provision",
        Action::"Computer.Checkpoint", Action::"Checkpoint",
        Action::"Computer.Sleep", Action::"Sleep",
        Action::"Computer.Wake", Action::"Wake",
        Action::"Computer.Destroy", Action::"Destroy"
    ],
    resource is Computer
);

// Callback actions (ProvisionComplete, ProvisionFailed, CheckpointComplete)
// are dispatched by WASM integrations running as the system service, or by
// an admin recording existing metal.
permit(
    principal,
    action in [
        Action::"Computer.ProvisionComplete", Action::"ProvisionComplete",
        Action::"Computer.ProvisionFailed", Action::"ProvisionFailed",
        Action::"Computer.CheckpointComplete", Action::"CheckpointComplete"
    ],
    resource is Computer
) when {
    principal.agent_type == "system"
};

permit(
    principal is Admin,
    action in [
        Action::"Computer.ProvisionComplete", Action::"ProvisionComplete",
        Action::"Computer.ProvisionFailed", Action::"ProvisionFailed",
        Action::"Computer.CheckpointComplete", Action::"CheckpointComplete"
    ],
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
        Action::"Exec.Run", Action::"Run"
    ],
    resource is Exec
);

// Exec callbacks (RunSucceeded, RunFailed) are dispatched by the
// computer_exec WASM integration running as the system service.
permit(
    principal,
    action in [
        Action::"Exec.RunSucceeded", Action::"RunSucceeded",
        Action::"Exec.RunFailed", Action::"RunFailed"
    ],
    resource is Exec
) when {
    principal.agent_type == "system"
};

permit(
    principal is Admin,
    action in [
        Action::"Exec.RunSucceeded", Action::"RunSucceeded",
        Action::"Exec.RunFailed", Action::"RunFailed"
    ],
    resource is Exec
);

// LatencyDiag: a learned, constrained diagnostic that runs one canned
// read-only command on a pinned computer through the same computer_exec
// trigger. Same governance shape as Exec — agents may create/read/list and
// fire RunScan; the callbacks are system-dispatched.
permit(
    principal is Agent,
    action in [
        Action::"create",
        Action::"read",
        Action::"list",
        Action::"LatencyDiag.RunScan", Action::"RunScan"
    ],
    resource is LatencyDiag
);

// LatencyDiag callbacks (RunSucceeded, RunFailed) are dispatched by the
// computer_exec WASM integration running as the system service.
permit(
    principal,
    action in [
        Action::"LatencyDiag.RunSucceeded", Action::"RunSucceeded",
        Action::"LatencyDiag.RunFailed", Action::"RunFailed"
    ],
    resource is LatencyDiag
) when {
    principal.agent_type == "system"
};

permit(
    principal is Admin,
    action in [
        Action::"LatencyDiag.RunScan", Action::"RunScan",
        Action::"LatencyDiag.RunSucceeded", Action::"RunSucceeded",
        Action::"LatencyDiag.RunFailed", Action::"RunFailed"
    ],
    resource is LatencyDiag
);

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
