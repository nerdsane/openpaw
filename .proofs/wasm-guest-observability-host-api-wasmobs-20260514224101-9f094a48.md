# WASM Guest Observability Host API Live Proof

- run_id: `wasmobs-20260514224101-9f094a48`
- proof_started_at: `2026-05-14T22:39:02Z`
- TemperPaw server: `http://127.0.0.1:3467`
- tenant: `default`
- proof entity: `WasmObservabilityProofs('proof-wasmobs-20260514224101-9f094a48')`
- final status: `Complete`

## Temper-Native Flow

The proof was run as an IOA entity and WASM integration chain:

1. `WasmObservabilityProof.RunProbe` transitioned `Created -> RunningProbe` and dispatched `guest_observability_probe`.
2. `guest_observability_probe` called `Context::start_span`, `WasmSpan::add_event`, `WasmSpan::set_attributes`, `WasmSpan::end_ok`, `Context::log_structured`, `Context::emit_metric`, `Context::emit_progress`, and an internal host HTTP call.
3. The probe callback dispatched `RunMigratedToolPath`, which transitioned `RunningProbe -> RunningMigratedToolPath` and invoked the migrated `monty_repl` module.
4. `monty_repl` executed `temper.specs()` from a `python` tool call, creating a migrated `tool.python` guest span with host-boundary HTTP child work.
5. `HandleToolResults` transitioned `RunningMigratedToolPath -> Complete`.

## Datadog Queries

- spans: `env:dev-wasm-observability @entity_id:proof-wasmobs-20260514224101-9f094a48`
- logs: `env:dev-wasm-observability @entity_id:proof-wasmobs-20260514224101-9f094a48`
- metrics: `sum:temperpaw.wasm_guest_observability.proof{*}.as_count()`

## Sanitized Evidence

- span events returned: `28`
- log events returned: `24`
- metric series returned: `1`
- trace ids: `019e28a7-0cb7-74d2-a85c-3bd4eb32bc10, 9ad6e60b2a054c3431d4d5a86c17bbac, c3b440852ebb64dfdeb9f0de08be0af2, d2a81c218794259c3c5f62673fe6f765, f070e8de945a0c31d1cf0863ac4a0ebc`
- log trace ids: `d2a81c218794259c3c5f62673fe6f765, f070e8de945a0c31d1cf0863ac4a0ebc`
- span names/resources sampled: `9c89174c537a, GET, Internal, WasmObservabilityProof.HandleToolResults, WasmObservabilityProof.RunMigratedToolPath, WasmObservabilityProof.RunMigratedToolPath.integrations, WasmObservabilityProof.RunProbe, WasmObservabilityProof.RunProbe.integrations, dev-wasm-observability, dispatch.background_adapter_integrations, dispatch.background_wasm_integrations, dispatch.dispatch_adapter_integrations_internal, dispatch.phase.actor_spawn, dispatch.phase.persist_wasm_invocation, dispatch.phase.query_projection, proof.guest_observability, proof.nested_guest_span, python, temper, temper-wasm-guest`
- metric names: `temperpaw.wasm_guest_observability.proof`

## OData State

```json
{
  "@odata.actions": [],
  "@odata.children": {},
  "@odata.context": "$metadata#WasmObservabilityProofs/$entity",
  "@odata.id": "WasmObservabilityProofs('proof-wasmobs-20260514224101-9f094a48')",
  "booleans": {},
  "counters": {},
  "entity_id": "proof-wasmobs-20260514224101-9f094a48",
  "entity_type": "WasmObservabilityProof",
  "events": [
    {
      "action": "Created",
      "from_status": "",
      "params": {
        "Id": "proof-wasmobs-20260514224101-9f094a48",
        "normal_repl_state_max_bytes": "0",
        "persist_tool_spans_file": "false",
        "run_id": "wasmobs-20260514224101-9f094a48",
        "temper_api_url": "http://127.0.0.1:3467",
        "tools_enabled": "temper_specs"
      },
      "timestamp": "2026-05-14T22:41:36.972363Z",
      "to_status": "Created"
    },
    {
      "action": "RunProbe",
      "from_status": "Created",
      "params": {
        "run_id": "wasmobs-20260514224101-9f094a48"
      },
      "timestamp": "2026-05-14T22:41:37.142111Z",
      "to_status": "RunningProbe"
    },
    {
      "action": "RunMigratedToolPath",
      "from_status": "RunningProbe",
      "params": {
        "conversation": "[]",
        "normal_repl_state_max_bytes": "0",
        "pending_tool_calls": "[{\"id\":\"toolu_wasmobs-20260514224101-9f094a48\",\"input\":{\"code\":\"print('wasm guest observability migrated Monty path run_id=wasmobs-20260514224101-9f094a48')\\ntemper.specs()\"},\"name\":\"python\",\"type\":\"tool_use\"}]",
        "persist_tool_spans_file": "false",
        "run_id": "wasmobs-20260514224101-9f094a48",
        "temper_api_url": "http://127.0.0.1:3467",
        "tools_enabled": "temper_specs",
        "workdir": "/workspace"
      },
      "timestamp": "2026-05-14T22:41:37.481474Z",
      "to_status": "RunningMigratedToolPath"
    },
    {
      "action": "HandleToolResults",
      "from_status": "RunningMigratedToolPath",
      "params": {
        "conversation": "[{\"content\":[{\"content\":\"wasm guest observability migrated Monty path run_id=wasmobs-20260514224101-9f094a48\\n\\n{\\\"specs\\\":[{\\\"actions\\\":[\\\"Configure\\\",\\\"Update\\\",\\\"Archive\\\"],\\\"entity_type\\\":\\\"Agent\\\",\\\"initial_state\\\":\\\"Created\\\",\\\"levels_passed\\\":1,\\\"levels_total\\\":1,\\\"states\\\":[\\\"Created\\\",\\\"Active\\\",\\\"Archived\\\"],\\\"tenant\\\":\\\"default\\\",\\\"verification_status\\\":\\\"passed\\\"},{\\\"actions\\\":[\\\"Register\\\",\\\"Update\\\",\\\"Disable\\\",\\\"Enable\\\"],\\\"entity_type\\\":\\\"AgentRoute\\\",\\\"initial_state\\\":\\\"Active\\\",\\\"levels_passed\\\":1,\\\"levels_total\\\":1,\\\"states\\\":[\\\"Active\\\",\\\"Disabled\\\"],\\\"tenant\\\":\\\"default\\\",\\\"verification_status\\\":\\\"passed\\\"},{\\\"actions\\\":[\\\"Install\\\",\\\"Archive\\\"],\\\"entity_type\\\":\\\"App\\\",\\\"initial_state\\\":\\\"Installed\\\",\\\"levels_passed\\\":1,\\\"levels_total\\\":1,\\\"states\\\":[\\\"Installed\\\",\\\"Archived\\\"],\\\"tenant\\\":\\\"default\\\",\\\"verification_status\\\":\\\"passed\\\"},{\\\"actions\\\":[\\\"Approve\\\",\\\"Reject\\\",\\\"InstallComplete\\\",\\\"InstallFailed\\\"],\\\"entity_type\\\":\\\"CapabilityRequest\\\",\\\"initial_state\\\":\\\"Requested\\\",\\\"levels_passed\\\":1,\\\"levels_total\\\":1,\\\"states\\\":[\\\"Requested\\\",\\\"Approved\\\",\\\"Installing\\\",\\\"Installed\\\",\\\"Rejected\\\",\\\"Failed\\\"],\\\"tenant\\\":\\\"default\\\",\\\"verification_status\\\":\\\"passed\\\"},{\\\"actions\\\":[\\\"Configure\\\",\\\"Connect\\\",\\\"Ready\\\",\\\"ReceiveMessage\\\",\\\"SendReply\\\",\\\"ReplyDelivered\\\",\\\"UpdateConfig\\\",\\\"UpdateCursor\\\",\\\"Disconnect\\\",\\\"Reconnect\\\",\\\"Archive\\\",\\\"ConnectFailed\\\",\\\"RouteFailed\\\",\\\"ReplyFailed\\\"],\\\"entity_type\\\":\\\"Channel\\\",\\\"initial_state\\\":\\\"Created\\\",\\\"levels_passed\\\":1,\\\"levels_total\\\":1,\\\"states\\\":[\\\"Created\\\",\\\"Connecting\\\",\\\"Connected\\\",\\\"Disconnected\\\",\\\"Archived\\\"],\\\"tenant\\\":\\\"default\\\",\\\"verification_status\\\":\\\"passed\\\"},{\\\"actions\\\":[\\\"Create\\\",\\\"Resume\\\",\\\"UpdateSession\\\",\\\"Expire\\\"],\\\"entity_type\\\":\\\"ChannelSession\\\",\\\"initial_state\\\":\\\"Active\\\",\\\"levels_passed\\\":1,\\\"levels_total\\\":1,\\\"states\\\":[\\\"Active\\\",\\\"Expired\\\"],\\\"tenant\\\":\\\"default\\\",\\\"verification_status\\\":\\\"passed\\\"},{\\\"actions\\\":[\\\"Create\\\",\\\"Edit\\\",\\\"Delete\\\",\\\"React\\\"],\\\"entity_type\\\":\\\"Comment\\\",\\\"initial_state\\\":\\\"Active\\\",\\\"levels_passed\\\":1,\\\"levels_total\\\":1,\\\"states\\\":[\\\"Active\\\",\\\"Edited\\\",\\\"Deleted\\\"],\\\"tenant\\\":\\\"default\\\",\\\"verification_status\\\":\\\"passed\\\"},{\\\"actions\\\":[\\\"Configure\\\",\\\"Activate\\\",\\\"ActivateComplete\\\",\\\"ActivateFailed\\\",\\\"Pause\\\",\\\"Resume\\\",\\\"Trigger\\\",\\\"TriggerComplete\\\",\\\"TriggerFailed\\\",\\\"Expire\\\"],\\\"entity_type\\\":\\\"CronJob\\\",\\\"initial_state\\\":\\\"Created\\\",\\\"levels_passed\\\":1,\\\"levels_total\\\":1,\\\"states\\\":[\\\"Created\\\",\\\"Active\\\",\\\"Paused\\\",\\\"Expired\\\"],\\\"tenant\\\":\\\"default\\\",\\\"verification_status\\\":\\\"passed\\\"},{\\\"actions\\\":[\\\"Configure\\\",\\\"QueueSynthesis\\\",\\\"Complete\\\",\\\"Fail\\\"],\\\"entity_type\\\":\\\"CurationDirection\\\",\\\"initial_state\\\":\\\"Discovered\\\",\\\"levels_passed\\\":1,\\\"levels_total\\\":1,\\\"states\\\":[\\\"Discovered\\\",\\\"Synthesizing\\\",\\\"Completed\\\",\\\"Failed\\\"],\\\"tenant\\\":\\\"default\\\",\\\"verification_status\\\":\\\"passed\\\"},{\\\"actions\\\":[\\\"Configure\\\",\\\"ConfigureAndSubmit\\\",\\\"Submit\\\",\\\"Start\\\",\\\"SessionSpawned\\\",\\\"RecordProgress\\\",\\\"Complete\\\",\\\"CompleteResearch\\\",\\\"CompleteSynthesis\\\",\\\"CompleteQualityReview\\\",\\\"CompleteOrganization\\\",\\\"CompleteRegeneration\\\",\\\"CompleteEvolution\\\",\\\"PublishResearchCompletion\\\",\\\"PublishSynthesisCompletion\\\",\\\"PublishOrganizationCompletion\\\",\\\"FinalizeCompletion\\\",\\\"Fail\\\",\\\"Retry\\\"],\\\"entity_type\\\":\\\"CurationJob\\\",\\\"initial_state\\\":\\\"Queued\\\",\\\"levels_passed\\\":1,\\\"levels_total\\\":1,\\\"states\\\":[\\\"Queued\\\",\\\"Ready\\\",\\\"Running\\\",\\\"Finalizing\\\",\\\"Completed\\\",\\\"Failed\\\"],\\\"tenant\\\":\\\"default\\\",\\\"verification_status\\\":\\\"passed\\\"},{\\\"actions\\\":[\\\"Configure\\\",\\\"Activate\\\",\\\"Supersede\\\"],\\\"entity_type\\\":\\\"CurationJobTemplate\\\",\\\"initial_state\\\":\\\"Draft\\\",\\\"levels_passed\\\":1,\\\"levels_total\\\":1,\\\"states\\\":[\\\"Draft\\\",\\\"Active\\\",\\\"Superseded\\\"],\\\"tenant\\\":\\\"default\\\",\\\"verification_status\\\":\\\"passed\\\"},{\\\"actions\\\":[\\\"Configure\\\",\\\"Submit\\\",\\\"ResearchComplete\\\",\\\"SynthesisComplete\\\",\\\"OrganizationComplete\\\",\\\"Fail\\\"],\\\"entity_type\\\":\\\"CurationQuery\\\",\\\"initial_state\\\":\\\"Submitted\\\",\\\"levels_passed\\\":1,\\\"levels_total\\\":1,\\\"states\\\":[\\\"Submitted\\\",\\\"Researching\\\",\\\"Synthesizing\\\",\\\"Organizing\\\",\\\"Completed\\\",\\\"Failed\\\"],\\\"tenant\\\":\\\"default\\\",\\\"verification_status\\\":\\\"passed\\\"},{\\\"actions\\\":[\\\"SetProject\\\",\\\"AddIssueToCycle\\\",\\\"Start\\\",\\\"MarkIssueComplete\\\",\\\"Complete\\\",\\\"RemoveIssueFromCycle\\\"],\\\"entity_type\\\":\\\"Cycle\\\",\\\"initial_state\\\":\\\"Planning\\\",\\\"levels_passed\\\":1,\\\"levels_total\\\":1,\\\"states\\\":[\\\"Planning\\\",\\\"Active\\\",\\\"Completed\\\"],\\\"tenant\\\":\\\"default\\\",\\\"verification_status\\\":\\\"passed\\\"},{\\\"actions\\\":[\\\"Start\\\",\\\"AttachSession\\\",\\\"AttachWorkerRun\\\",\\\"Render\\\",\\\"Publish\\\",\\\"Fail\\\"],\\\"entity_type\\\":\\\"DailyBrief\\\",\\\"initial_state\\\":\\\"Collecting\\\",\\\"levels_passed\\\":1,\\\"levels_total\\\":1,\\\"states\\\":[\\\"Collecting\\\",\\\"Ready\\\",\\\"Published\\\",\\\"Failed\\\"],\\\"tenant\\\":\\\"default\\\",\\\"verification_st
```
