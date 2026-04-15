// paw-consilium WASM Module Authorization Policies

permit(
    principal is Agent,
    action == Action::"http_call",
    resource is HttpEndpoint
) when {
    context.module == "spawn_perspectives" ||
    context.module == "check_and_synthesize"
};

permit(
    principal is Agent,
    action == Action::"access_secret",
    resource is Secret
) when {
    context.module == "spawn_perspectives" ||
    context.module == "check_and_synthesize"
};
