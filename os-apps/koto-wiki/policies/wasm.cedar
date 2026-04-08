// Koto-Wiki WASM Module Authorization Policies

// Allow build_session_message to make HTTP calls (create sessions via OData)
permit(
    principal is Agent,
    action == Action::"http_call",
    resource is HttpEndpoint
) when {
    context.module == "build_session_message"
};

// Allow build_session_message to access secrets (temper_api_url)
permit(
    principal is Agent,
    action == Action::"access_secret",
    resource is Secret
) when {
    context.module == "build_session_message"
};
