// paw-wiki WASM Module Authorization Policies

// Allow WASM modules to make HTTP calls (create sessions via OData)
permit(
    principal is Agent,
    action == Action::"http_call",
    resource is HttpEndpoint
) when {
    context.module == "build_session_message" ||
    context.module == "finalize_spawned_session"
};

// Allow WASM modules to access secrets (temper_api_url)
permit(
    principal is Agent,
    action == Action::"access_secret",
    resource is Secret
) when {
    context.module == "build_session_message" ||
    context.module == "finalize_spawned_session"
};
