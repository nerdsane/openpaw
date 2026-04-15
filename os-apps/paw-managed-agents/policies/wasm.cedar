// paw-managed-agents WASM authorization.

permit(
  principal is Agent,
  action == Action::"http_call",
  resource is HttpEndpoint
) when {
  ["session_orchestrator", "event_emitter", "environment_provisioner", "session_terminator"].contains(context.module)
};

permit(
  principal is Agent,
  action == Action::"access_secret",
  resource is Secret
) when {
  ["session_orchestrator", "event_emitter", "environment_provisioner", "session_terminator"].contains(context.module)
};
