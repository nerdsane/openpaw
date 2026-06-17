// Authenticated agents can create, inspect, and start MediaGeneration flows.
permit(
  principal,
  action in [
    Action::"create",
    Action::"read",
    Action::"list",
    Action::"Generate"
  ],
  resource is MediaGeneration
);

// Provider auth can only report auth readiness or auth failure from its WASM module.
permit(
  principal is Agent,
  action in [
    Action::"RecordAuthReady",
    Action::"RecordError"
  ],
  resource is MediaGeneration
) when {
  context.module == "provider_auth_gate"
};

// The Codex renderer is the only module that can store or finalize image results.
permit(
  principal is Agent,
  action in [
    Action::"RecordStoring",
    Action::"RecordResult",
    Action::"RecordError"
  ],
  resource is MediaGeneration
) when {
  context.module == "openai_codex_image_generate"
};

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
