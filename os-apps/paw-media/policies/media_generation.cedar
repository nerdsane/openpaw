// Authenticated agents can create, inspect, and dispatch MediaGeneration flows.
permit(
  principal,
  action in [
    Action::"create",
    Action::"read",
    Action::"list",
    Action::"Generate",
    Action::"RecordAuthReady",
    Action::"RecordStoring",
    Action::"RecordResult",
    Action::"RecordError"
  ],
  resource is MediaGeneration
);

// Allow the provider WASM to call the Codex backend and local OData/PawFS APIs.
permit(
  principal is Agent,
  action == Action::"http_call",
  resource is HttpEndpoint
) when {
  context.module == "openai_codex_image_generate"
};

// Allow the provider WASM to read Codex OAuth/token and Temper API secrets.
permit(
  principal is Agent,
  action == Action::"access_secret",
  resource is Secret
) when {
  context.module == "openai_codex_image_generate"
};
