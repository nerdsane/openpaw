// TemperFS WASM Module Authorization Policies

// Allow blob_adapter to make HTTP calls (blob storage upload/download)
permit(
    principal is Agent,
    action == Action::"http_call",
    resource is HttpEndpoint
) when {
    context.module == "blob_adapter"
};

// Allow workspace_fs to make HTTP calls (create directories/files via OData)
permit(
    principal is Agent,
    action == Action::"http_call",
    resource is HttpEndpoint
) when {
    context.module == "workspace_fs"
};

// Allow workspace_fs to access secrets (temper_api_url)
permit(
    principal is Agent,
    action == Action::"access_secret",
    resource is Secret
) when {
    context.module == "workspace_fs"
};

// Allow blob_adapter to access secrets (blob_endpoint, blob_bucket)
permit(
    principal is Agent,
    action == Action::"access_secret",
    resource is Secret
) when {
    context.module == "blob_adapter"
};
