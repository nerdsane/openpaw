// paw-autoreason WASM Module Authorization Policies

permit(
    principal is Agent,
    action == Action::"http_call",
    resource is HttpEndpoint
) when {
    context.module == "initialize_tournament" ||
    context.module == "run_round" ||
    context.module == "tally_votes"
};

permit(
    principal is Agent,
    action == Action::"access_secret",
    resource is Secret
) when {
    context.module == "initialize_tournament" ||
    context.module == "run_round" ||
    context.module == "tally_votes"
};
