#!/usr/bin/env python3
"""Live proof for the WASM guest observability host API.

The proof harness only installs and drives the proof. The observable work is
Temper-native: an inline IOA spec dispatches guest_observability_probe, whose
callback dispatches the migrated monty_repl module on the same entity.
"""

from __future__ import annotations

import argparse
import base64
import datetime as dt
import json
import os
import pathlib
import subprocess
import sys
import time
import urllib.error
import urllib.parse
import urllib.request
from typing import Any


ROOT = pathlib.Path(__file__).resolve().parents[1]
TENANT = os.environ.get("TEMPERPAW_TENANT", "default")
DEFAULT_SERVER = os.environ.get("TEMPERPAW_SERVER", "http://127.0.0.1:3467")
PROBE_DIR = ROOT / "os-apps/paw-agent/wasm/guest_observability_probe"
PROBE_WASM = PROBE_DIR / "target/wasm32-unknown-unknown/release/guest_observability_probe.wasm"


def utc_now() -> dt.datetime:
    return dt.datetime.now(dt.timezone.utc).replace(microsecond=0)


def datadog_api_base(site: str) -> str:
    site = site.strip() or "datadoghq.com"
    return f"https://api.{site}"


def request_json(
    method: str,
    url: str,
    *,
    headers: dict[str, str] | None = None,
    body: Any | None = None,
    timeout: int = 120,
) -> Any:
    data = None
    req_headers = dict(headers or {})
    if body is not None:
        data = json.dumps(body).encode("utf-8")
        req_headers.setdefault("Content-Type", "application/json")
    req = urllib.request.Request(url, data=data, method=method, headers=req_headers)
    try:
        with urllib.request.urlopen(req, timeout=timeout) as resp:
            raw = resp.read().decode("utf-8")
    except urllib.error.HTTPError as exc:
        detail = exc.read().decode("utf-8", errors="replace")
        raise RuntimeError(f"{method} {url} failed: HTTP {exc.code}: {detail}") from exc
    if not raw:
        return {}
    try:
        return json.loads(raw)
    except json.JSONDecodeError:
        return raw


class TemperClient:
    def __init__(self, base_url: str, tenant: str, api_key: str | None = None) -> None:
        self.base_url = base_url.rstrip("/")
        self.tenant = tenant
        self.api_key = api_key

    def headers(self) -> dict[str, str]:
        headers = {
            "Accept": "application/json",
            "Content-Type": "application/json",
            "X-Tenant-Id": self.tenant,
            "X-Temper-Principal-Kind": "admin",
            "X-Temper-Principal-Id": "wasm-guest-observability-proof",
        }
        if self.api_key:
            headers["Authorization"] = f"Bearer {self.api_key}"
        return headers

    def request(self, method: str, path: str, body: Any | None = None) -> Any:
        return request_json(
            method,
            f"{self.base_url}{path}",
            headers=self.headers(),
            body=body,
            timeout=600,
        )

    def submit_specs(self, specs: dict[str, str]) -> Any:
        return self.request(
            "POST",
            "/api/specs/load-inline",
            {"tenant": self.tenant, "specs": specs},
        )

    def create_policy(self, policy_id: str, cedar_text: str) -> Any:
        return self.request(
            "POST",
            f"/api/tenants/{self.tenant}/policies/create",
            {"policy_id": policy_id, "cedar_text": cedar_text},
        )

    def upload_wasm(self, module_name: str, wasm_path: pathlib.Path) -> Any:
        wasm_b64 = base64.b64encode(wasm_path.read_bytes()).decode("ascii")
        return self.request(
            "POST",
            f"/api/wasm/modules/{module_name}",
            {"wasm_base64": wasm_b64},
        )

    def create(self, entity_set: str, body: dict[str, Any]) -> Any:
        return self.request("POST", f"/tdata/{entity_set}", body)

    def action(
        self,
        entity_set: str,
        entity_id: str,
        action: str,
        body: dict[str, Any],
        *,
        await_integration: bool = True,
    ) -> Any:
        suffix = "?await_integration=true" if await_integration else ""
        key = urllib.parse.quote(entity_id, safe="")
        return self.request(
            "POST",
            f"/tdata/{entity_set}('{key}')/{action}{suffix}",
            body,
        )

    def get(self, entity_set: str, entity_id: str) -> Any:
        key = urllib.parse.quote(entity_id, safe="")
        return self.request("GET", f"/tdata/{entity_set}('{key}')")

    def wait_for_status(
        self,
        entity_set: str,
        entity_id: str,
        expected: str,
        timeout_seconds: int = 120,
    ) -> Any:
        deadline = time.time() + timeout_seconds
        last = None
        while time.time() < deadline:
            last = self.get(entity_set, entity_id)
            fields = last.get("fields", last)
            status = fields.get("Status") or last.get("status") or last.get("Status")
            if status == expected:
                return last
            if status == "Failed":
                raise RuntimeError(f"{entity_set} {entity_id} failed: {json.dumps(last)[:2000]}")
            time.sleep(1)
        raise TimeoutError(f"timed out waiting for {entity_set} {entity_id} status={expected}: {last}")


def proof_csdl() -> str:
    return """<?xml version="1.0" encoding="utf-8"?>
<edmx:Edmx Version="4.0" xmlns:edmx="http://docs.oasis-open.org/odata/ns/edmx">
  <edmx:DataServices>
    <Schema Namespace="TemperPaw.Proof" xmlns="http://docs.oasis-open.org/odata/ns/edm">
      <EntityType Name="WasmObservabilityProof">
        <Key><PropertyRef Name="Id"/></Key>
        <Property Name="Id" Type="Edm.String" Nullable="false"/>
        <Property Name="Status" Type="Edm.String"/>
        <Property Name="run_id" Type="Edm.String"/>
        <Property Name="pending_tool_calls" Type="Edm.String"/>
        <Property Name="conversation" Type="Edm.String"/>
        <Property Name="temper_api_url" Type="Edm.String"/>
        <Property Name="workdir" Type="Edm.String"/>
        <Property Name="tools_enabled" Type="Edm.String"/>
        <Property Name="normal_repl_state_max_bytes" Type="Edm.String"/>
        <Property Name="persist_tool_spans_file" Type="Edm.String"/>
        <Property Name="repl_file_id" Type="Edm.String"/>
        <Property Name="tool_spans_file_id" Type="Edm.String"/>
        <Property Name="pending_tool_context" Type="Edm.String"/>
        <Property Name="pending_decision_id" Type="Edm.String"/>
        <Property Name="last_progress_at" Type="Edm.String"/>
        <Property Name="error" Type="Edm.String"/>
        <Property Name="error_message" Type="Edm.String"/>
      </EntityType>

      <Action Name="RunProbe" IsBound="true">
        <Parameter Name="bindingParameter" Type="TemperPaw.Proof.WasmObservabilityProof"/>
        <Parameter Name="run_id" Type="Edm.String" Nullable="false"/>
        <ReturnType Type="TemperPaw.Proof.WasmObservabilityProof"/>
      </Action>

      <Action Name="RunMigratedToolPath" IsBound="true">
        <Parameter Name="bindingParameter" Type="TemperPaw.Proof.WasmObservabilityProof"/>
        <Parameter Name="run_id" Type="Edm.String" Nullable="false"/>
        <Parameter Name="pending_tool_calls" Type="Edm.String" Nullable="false"/>
        <Parameter Name="conversation" Type="Edm.String" Nullable="true"/>
        <Parameter Name="temper_api_url" Type="Edm.String" Nullable="true"/>
        <Parameter Name="workdir" Type="Edm.String" Nullable="true"/>
        <Parameter Name="tools_enabled" Type="Edm.String" Nullable="true"/>
        <Parameter Name="normal_repl_state_max_bytes" Type="Edm.String" Nullable="true"/>
        <Parameter Name="persist_tool_spans_file" Type="Edm.String" Nullable="true"/>
        <ReturnType Type="TemperPaw.Proof.WasmObservabilityProof"/>
      </Action>

      <Action Name="HandleToolResults" IsBound="true">
        <Parameter Name="bindingParameter" Type="TemperPaw.Proof.WasmObservabilityProof"/>
        <Parameter Name="pending_tool_calls" Type="Edm.String" Nullable="true"/>
        <Parameter Name="conversation" Type="Edm.String" Nullable="true"/>
        <Parameter Name="repl_file_id" Type="Edm.String" Nullable="true"/>
        <Parameter Name="tool_spans_file_id" Type="Edm.String" Nullable="true"/>
        <Parameter Name="sandbox_url" Type="Edm.String" Nullable="true"/>
        <Parameter Name="sandbox_id" Type="Edm.String" Nullable="true"/>
        <Parameter Name="sandbox_provider" Type="Edm.String" Nullable="true"/>
        <Parameter Name="system_prompt_hash" Type="Edm.String" Nullable="true"/>
        <Parameter Name="system_prompt_file_id" Type="Edm.String" Nullable="true"/>
        <Parameter Name="pending_tool_context" Type="Edm.String" Nullable="true"/>
        <Parameter Name="pending_decision_id" Type="Edm.String" Nullable="true"/>
        <ReturnType Type="TemperPaw.Proof.WasmObservabilityProof"/>
      </Action>

      <Action Name="RecordResult" IsBound="true">
        <Parameter Name="bindingParameter" Type="TemperPaw.Proof.WasmObservabilityProof"/>
        <Parameter Name="result" Type="Edm.String" Nullable="true"/>
        <Parameter Name="pending_tool_calls" Type="Edm.String" Nullable="true"/>
        <Parameter Name="conversation" Type="Edm.String" Nullable="true"/>
        <ReturnType Type="TemperPaw.Proof.WasmObservabilityProof"/>
      </Action>

      <Action Name="CheckpointToolBatch" IsBound="true">
        <Parameter Name="bindingParameter" Type="TemperPaw.Proof.WasmObservabilityProof"/>
        <Parameter Name="pending_tool_calls" Type="Edm.String" Nullable="true"/>
        <Parameter Name="pending_tool_context" Type="Edm.String" Nullable="true"/>
        <Parameter Name="repl_file_id" Type="Edm.String" Nullable="true"/>
        <ReturnType Type="TemperPaw.Proof.WasmObservabilityProof"/>
      </Action>

      <Action Name="ProgressMade" IsBound="true">
        <Parameter Name="bindingParameter" Type="TemperPaw.Proof.WasmObservabilityProof"/>
        <Parameter Name="last_progress_at" Type="Edm.String" Nullable="true"/>
        <ReturnType Type="TemperPaw.Proof.WasmObservabilityProof"/>
      </Action>

      <Action Name="Fail" IsBound="true">
        <Parameter Name="bindingParameter" Type="TemperPaw.Proof.WasmObservabilityProof"/>
        <Parameter Name="error" Type="Edm.String" Nullable="true"/>
        <Parameter Name="error_message" Type="Edm.String" Nullable="true"/>
        <ReturnType Type="TemperPaw.Proof.WasmObservabilityProof"/>
      </Action>

      <EntityContainer Name="ProofService">
        <EntitySet Name="WasmObservabilityProofs" EntityType="TemperPaw.Proof.WasmObservabilityProof"/>
      </EntityContainer>
    </Schema>
  </edmx:DataServices>
</edmx:Edmx>
"""


def proof_ioa(base_url: str) -> str:
    escaped_base_url = base_url.replace("\\", "\\\\").replace('"', '\\"')
    return f'''# Live proof flow for the WASM guest observability host API.

[automaton]
name = "WasmObservabilityProof"
states = ["Created", "RunningProbe", "RunningMigratedToolPath", "Complete", "Failed"]
initial = "Created"
allow_indefinite_states = ["Created", "Complete", "Failed"]

[[state]]
name = "run_id"
type = "string"
initial = ""

[[state]]
name = "pending_tool_calls"
type = "string"
initial = "[]"
overflow_inline_max_bytes = "131072"

[[state]]
name = "conversation"
type = "string"
initial = "[]"
overflow_inline_max_bytes = "131072"

[[state]]
name = "temper_api_url"
type = "string"
initial = "{escaped_base_url}"

[[state]]
name = "workdir"
type = "string"
initial = "/workspace"

[[state]]
name = "tools_enabled"
type = "string"
initial = "temper_specs"

[[state]]
name = "normal_repl_state_max_bytes"
type = "string"
initial = "0"

[[state]]
name = "persist_tool_spans_file"
type = "string"
initial = "false"

[[state]]
name = "repl_file_id"
type = "string"
initial = ""

[[state]]
name = "tool_spans_file_id"
type = "string"
initial = ""

[[state]]
name = "pending_tool_context"
type = "string"
initial = ""

[[state]]
name = "pending_decision_id"
type = "string"
initial = ""

[[state]]
name = "last_progress_at"
type = "string"
initial = ""

[[state]]
name = "error"
type = "string"
initial = ""

[[state]]
name = "error_message"
type = "string"
initial = ""

[[action]]
name = "RunProbe"
kind = "input"
from = ["Created"]
to = "RunningProbe"
params = ["run_id"]
hint = "Start the direct guest observability API proof."
effect = [{{ type = "trigger", name = "run_guest_observability_probe" }}]

[[action.triggers]]
name = "run_guest_observability_probe"
kind = "wasm"
module = "guest_observability_probe"
on_failure = "Fail"

[action.triggers.config]
temper_api_url = "{escaped_base_url}"

[[action]]
name = "RunMigratedToolPath"
kind = "input"
from = ["RunningProbe"]
to = "RunningMigratedToolPath"
params = ["run_id", "pending_tool_calls", "conversation", "temper_api_url", "workdir", "tools_enabled", "normal_repl_state_max_bytes", "persist_tool_spans_file"]
hint = "Callback from the proof probe. Run the migrated Monty tool path on the same entity."
effect = [{{ type = "trigger", name = "run_migrated_monty_tool_path" }}]

[[action.triggers]]
name = "run_migrated_monty_tool_path"
kind = "wasm"
module = "monty_repl"
on_failure = "Fail"

[action.triggers.config]
temper_api_url = "{escaped_base_url}"
normal_repl_state_max_bytes = "0"
persist_tool_spans_file = "false"
tool_progress_dispatch_enabled = "false"
heartbeat_dispatch_enabled = "false"

[[action]]
name = "HandleToolResults"
kind = "input"
from = ["RunningMigratedToolPath"]
to = "Complete"
params = ["pending_tool_calls", "conversation", "repl_file_id", "tool_spans_file_id", "sandbox_url", "sandbox_id", "sandbox_provider", "system_prompt_hash", "system_prompt_file_id", "pending_tool_context", "pending_decision_id"]
hint = "Monty completed the migrated tool span path."

[[action]]
name = "RecordResult"
kind = "input"
from = ["RunningMigratedToolPath"]
to = "Complete"
params = ["result", "pending_tool_calls", "conversation"]
hint = "Monty completed by calling temper.done."

[[action]]
name = "CheckpointToolBatch"
kind = "input"
from = ["RunningMigratedToolPath"]
to = "RunningMigratedToolPath"
params = ["pending_tool_calls", "pending_tool_context", "repl_file_id"]
effect = [{{ type = "trigger", name = "run_migrated_monty_tool_path" }}]
hint = "Checkpoint continuation for proof tool batches."

[[action]]
name = "ProgressMade"
kind = "input"
from = ["RunningMigratedToolPath"]
to = "RunningMigratedToolPath"
params = ["last_progress_at"]
hint = "Optional migrated-tool progress signal."

[[action]]
name = "Fail"
kind = "input"
from = ["Created", "RunningProbe", "RunningMigratedToolPath"]
to = "Failed"
params = ["error", "error_message"]
hint = "The live observability proof failed."
'''


def build_probe_wasm() -> None:
    subprocess.run(
        ["cargo", "build", "--release", "--target", "wasm32-unknown-unknown"],
        cwd=PROBE_DIR,
        check=True,
    )
    if not PROBE_WASM.exists():
        raise RuntimeError(f"proof WASM was not built at {PROBE_WASM}")


def safe_policy_run_id(run_id: str) -> str:
    return "".join(ch if ch.isalnum() or ch in "-_" else "-" for ch in run_id)


def proof_submit_specs_policy(run_id: str) -> tuple[str, str]:
    safe_run_id = safe_policy_run_id(run_id)
    return (
        f"wasm-guest-observability-proof-submit-specs-{safe_run_id}",
        """permit(
  principal is Admin,
  action == Action::"submit_specs",
  resource is SpecRegistry
);""",
    )


def proof_runtime_policy(run_id: str) -> tuple[str, str]:
    safe_run_id = safe_policy_run_id(run_id)
    return (
        f"wasm-guest-observability-proof-runtime-{safe_run_id}",
        """permit(
  principal,
  action,
  resource is WasmObservabilityProof
);

permit(
  principal is Agent,
  action == Action::"http_call",
  resource is HttpEndpoint
) when {
  context.module == "guest_observability_probe"
};""",
    )


def dd_headers(api_key: str, app_key: str) -> dict[str, str]:
    return {
        "Accept": "application/json",
        "Content-Type": "application/json",
        "DD-API-KEY": api_key,
        "DD-APPLICATION-KEY": app_key,
    }


def search_datadog_events(
    *,
    api_base: str,
    api_key: str,
    app_key: str,
    endpoint: str,
    query: str,
    start: dt.datetime,
    limit: int = 100,
) -> dict[str, Any]:
    filter_body = {
        "from": start.isoformat().replace("+00:00", "Z"),
        "to": "now",
        "query": query,
    }
    if endpoint.startswith("/api/v2/spans/"):
        body = {
            "data": {
                "type": "search_request",
                "attributes": {
                    "filter": filter_body,
                    "page": {"limit": limit},
                },
            }
        }
    else:
        body = {
            "filter": filter_body,
            "page": {"limit": limit},
        }
    return request_json(
        "POST",
        f"{api_base}{endpoint}",
        headers=dd_headers(api_key, app_key),
        body=body,
        timeout=60,
    )


def query_datadog_metric(
    *,
    api_base: str,
    api_key: str,
    app_key: str,
    query: str,
    start: dt.datetime,
) -> dict[str, Any]:
    now = int(time.time())
    start_ts = int(start.timestamp())
    encoded = urllib.parse.urlencode({"from": start_ts, "to": now, "query": query})
    return request_json(
        "GET",
        f"{api_base}/api/v1/query?{encoded}",
        headers=dd_headers(api_key, app_key),
        timeout=60,
    )


def event_count(payload: Any) -> int:
    if isinstance(payload, dict):
        data = payload.get("data")
        if isinstance(data, list):
            return len(data)
        series = payload.get("series")
        if isinstance(series, list):
            return len(series)
    return 0


def payload_contains(payload: Any, needle: str) -> bool:
    return needle in json.dumps(payload, sort_keys=True, default=str)


def extract_strings(value: Any, keys: set[str]) -> list[str]:
    found: list[str] = []
    if isinstance(value, dict):
        for key, child in value.items():
            if key in keys and isinstance(child, (str, int, float)):
                found.append(str(child))
            found.extend(extract_strings(child, keys))
    elif isinstance(value, list):
        for child in value:
            found.extend(extract_strings(child, keys))
    return found


def wait_for_datadog(
    *,
    run_id: str,
    dd_env: str,
    start: dt.datetime,
    api_base: str,
    api_key: str,
    app_key: str,
    timeout_seconds: int,
) -> dict[str, Any]:
    proof_entity_id = f"proof-{run_id}"
    span_query = f"env:{dd_env} @entity_id:{proof_entity_id}"
    log_query = f"env:{dd_env} @entity_id:{proof_entity_id}"
    metric_query = f"sum:temperpaw.wasm_guest_observability.proof{{env:{dd_env}}}.as_count()"
    deadline = time.time() + timeout_seconds
    last: dict[str, Any] = {}
    while time.time() < deadline:
        try:
            spans = search_datadog_events(
                api_base=api_base,
                api_key=api_key,
                app_key=app_key,
                endpoint="/api/v2/spans/events/search",
                query=span_query,
                start=start,
            )
            logs = search_datadog_events(
                api_base=api_base,
                api_key=api_key,
                app_key=app_key,
                endpoint="/api/v2/logs/events/search",
                query=log_query,
                start=start,
            )
            metrics = query_datadog_metric(
                api_base=api_base,
                api_key=api_key,
                app_key=app_key,
                query=metric_query,
                start=start,
            )
        except RuntimeError as exc:
            last = {
                "error": str(exc),
                "queries": {
                    "spans": span_query,
                    "logs": log_query,
                    "metrics": metric_query,
                },
            }
            time.sleep(20)
            continue
        last = {
            "spans": spans,
            "logs": logs,
            "metrics": metrics,
            "queries": {
                "spans": span_query,
                "logs": log_query,
                "metrics": metric_query,
            },
        }
        has_core_spans = all(
            payload_contains(spans, span_name)
            for span_name in [
                "WasmObservabilityProof.RunProbe.integrations",
                "proof.guest_observability",
                "proof.nested_guest_span",
                "tool.python",
                "GET",
                "WasmObservabilityProof.RunMigratedToolPath.integrations",
            ]
        )
        has_guest_log = payload_contains(
            logs, "wasm guest observability proof structured log"
        )
        if (
            event_count(spans) > 0
            and event_count(logs) > 0
            and event_count(metrics) > 0
            and has_core_spans
            and has_guest_log
        ):
            return last
        time.sleep(20)
    raise TimeoutError(
        "Datadog proof evidence did not appear before timeout: "
        f"spans={event_count(last.get('spans'))} "
        f"logs={event_count(last.get('logs'))} "
        f"metrics={event_count(last.get('metrics'))} "
        f"last_error={last.get('error', '')}"
    )


def write_report(
    *,
    run_id: str,
    server: str,
    tenant: str,
    proof_entity_id: str,
    final_entity: Any,
    datadog: dict[str, Any],
    proof_start: dt.datetime,
) -> pathlib.Path:
    proof_dir = ROOT / ".proofs"
    proof_dir.mkdir(exist_ok=True)
    report_path = proof_dir / f"wasm-guest-observability-host-api-{run_id}.md"
    span_names = sorted(
        set(
            extract_strings(
                datadog.get("spans"),
                {
                    "name",
                    "resource_name",
                    "operation_name",
                    "span.name",
                    "span_name",
                },
            )
        )
    )
    trace_ids = sorted(set(extract_strings(datadog.get("spans"), {"trace_id", "traceId"})))
    log_trace_ids = sorted(set(extract_strings(datadog.get("logs"), {"dd.trace_id", "trace_id"})))
    metric_names = sorted(
        set(extract_strings(datadog.get("metrics"), {"metric", "metric_name"}))
        | {"temperpaw.wasm_guest_observability.proof"}
    )
    fields = final_entity.get("fields", final_entity) if isinstance(final_entity, dict) else {}
    status = fields.get("Status") or final_entity.get("status", "")

    report = f"""# WASM Guest Observability Host API Live Proof

- run_id: `{run_id}`
- proof_started_at: `{proof_start.isoformat().replace("+00:00", "Z")}`
- TemperPaw server: `{server}`
- tenant: `{tenant}`
- proof entity: `WasmObservabilityProofs('{proof_entity_id}')`
- final status: `{status}`

## Temper-Native Flow

The proof was run as an IOA entity and WASM integration chain:

1. `WasmObservabilityProof.RunProbe` transitioned `Created -> RunningProbe` and dispatched `guest_observability_probe`.
2. `guest_observability_probe` called `Context::start_span`, `WasmSpan::add_event`, `WasmSpan::set_attributes`, `WasmSpan::end_ok`, `Context::log_structured`, `Context::emit_metric`, `Context::emit_progress`, and an internal host HTTP call.
3. The probe callback dispatched `RunMigratedToolPath`, which transitioned `RunningProbe -> RunningMigratedToolPath` and invoked the migrated `monty_repl` module.
4. `monty_repl` executed `temper.specs()` from a `python` tool call, creating a migrated `tool.python` guest span with host-boundary HTTP child work.
5. `HandleToolResults` transitioned `RunningMigratedToolPath -> Complete`.

## Datadog Queries

- spans: `{datadog["queries"]["spans"]}`
- logs: `{datadog["queries"]["logs"]}`
- metrics: `{datadog["queries"]["metrics"]}`

## Sanitized Evidence

- span events returned: `{event_count(datadog.get("spans"))}`
- log events returned: `{event_count(datadog.get("logs"))}`
- metric series returned: `{event_count(datadog.get("metrics"))}`
- trace ids: `{", ".join(trace_ids[:8]) or "not extracted from response shape"}`
- log trace ids: `{", ".join(log_trace_ids[:8]) or "not extracted from response shape"}`
- span names/resources sampled: `{", ".join(span_names[:20]) or "not extracted from response shape"}`
- metric names: `{", ".join(metric_names[:10])}`

## OData State

```json
{json.dumps(final_entity, indent=2, sort_keys=True)[:8000]}
```
"""
    report_path.write_text(report)

    summary_path = proof_dir / f"wasm-guest-observability-host-api-{run_id}.json"
    summary_path.write_text(
        json.dumps(
            {
                "run_id": run_id,
                "proof_entity_id": proof_entity_id,
                "status": status,
                "span_event_count": event_count(datadog.get("spans")),
                "log_event_count": event_count(datadog.get("logs")),
                "metric_series_count": event_count(datadog.get("metrics")),
                "trace_ids": trace_ids[:8],
                "log_trace_ids": log_trace_ids[:8],
                "span_names": span_names[:20],
                "metric_names": metric_names[:10],
                "report": str(report_path),
            },
            indent=2,
            sort_keys=True,
        )
    )
    return report_path


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--server", default=DEFAULT_SERVER)
    parser.add_argument("--tenant", default=TENANT)
    parser.add_argument("--run-id", default=f"wasmobs-{utc_now().strftime('%Y%m%d%H%M%S')}")
    parser.add_argument("--skip-build", action="store_true")
    parser.add_argument(
        "--datadog-env",
        default=os.environ.get("DD_ENV", "dev-wasm-observability"),
    )
    parser.add_argument("--datadog-timeout-seconds", type=int, default=420)
    args = parser.parse_args()

    api_key = os.environ.get("DD_API_KEY", "")
    app_key = os.environ.get("DD_APP_KEY", "")
    site = os.environ.get("DD_SITE", "datadoghq.com")
    temper_api_key = os.environ.get("TEMPERPAW_API_KEY", "")
    if not api_key or not app_key:
        raise RuntimeError("DD_API_KEY and DD_APP_KEY are required for live Datadog proof queries")

    proof_start = utc_now() - dt.timedelta(minutes=2)
    if not args.skip_build:
        print("Building proof WASM module...")
        build_probe_wasm()

    client = TemperClient(args.server, args.tenant, temper_api_key or None)
    policy_id, cedar_text = proof_submit_specs_policy(args.run_id)
    print(f"Installing proof-scoped submit_specs policy {policy_id}...")
    client.create_policy(policy_id, cedar_text)

    print("Submitting proof specs...")
    client.submit_specs(
        {
            "model.csdl.xml": proof_csdl(),
            "wasm_observability_proof.ioa.toml": proof_ioa(args.server.rstrip("/")),
        }
    )
    runtime_policy_id, runtime_cedar_text = proof_runtime_policy(args.run_id)
    print(f"Installing proof runtime policy {runtime_policy_id}...")
    client.create_policy(runtime_policy_id, runtime_cedar_text)

    print("Uploading proof WASM module...")
    client.upload_wasm("guest_observability_probe", PROBE_WASM)

    proof_entity_id = f"proof-{args.run_id}"
    print(f"Creating proof entity {proof_entity_id}...")
    client.create(
        "WasmObservabilityProofs",
        {
            "Id": proof_entity_id,
            "run_id": args.run_id,
            "temper_api_url": args.server.rstrip("/"),
            "tools_enabled": "temper_specs",
            "normal_repl_state_max_bytes": "0",
            "persist_tool_spans_file": "false",
        },
    )
    print("Dispatching RunProbe with await_integration=true...")
    client.action(
        "WasmObservabilityProofs",
        proof_entity_id,
        "TemperPaw.Proof.RunProbe",
        {"run_id": args.run_id},
    )
    entity_timeout_seconds = max(300, args.datadog_timeout_seconds)
    final_entity = client.wait_for_status(
        "WasmObservabilityProofs",
        proof_entity_id,
        "Complete",
        timeout_seconds=entity_timeout_seconds,
    )

    print("Querying Datadog for trace/log/metric evidence...")
    dd = wait_for_datadog(
        run_id=args.run_id,
        dd_env=args.datadog_env,
        start=proof_start,
        api_base=datadog_api_base(site),
        api_key=api_key,
        app_key=app_key,
        timeout_seconds=args.datadog_timeout_seconds,
    )
    report_path = write_report(
        run_id=args.run_id,
        server=args.server,
        tenant=args.tenant,
        proof_entity_id=proof_entity_id,
        final_entity=final_entity,
        datadog=dd,
        proof_start=proof_start,
    )
    print(f"Proof complete: {report_path}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
