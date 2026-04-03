// All authenticated principals can create, read, and execute web queries.
permit(
  principal,
  action in [
    Action::"create",
    Action::"read",
    Action::"list",
    Action::"ExecuteSearch",
    Action::"ExecuteFetch",
    Action::"RecordResults",
    Action::"RecordError"
  ],
  resource is WebQuery
);

// Allow WASM modules to make outbound HTTP calls for search and fetch.
permit(
  principal is Agent,
  action == Action::"http_call",
  resource is HttpEndpoint
) when {
  ["web_search", "web_fetch"].contains(context.module)
};
