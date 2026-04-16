#!/usr/bin/env python3
"""Managed agents proof runner.

This script is intentionally strict and doubles as the red/green end-to-end
test target for the app implementation.
"""

from __future__ import annotations

import json
import os
import sys
import time
import urllib.error
import urllib.parse
import urllib.request


SERVER = os.environ.get("OPENPAW_SERVER", "http://127.0.0.1:3000")
TENANT = os.environ.get("OPENPAW_TENANT", "default")
API_KEY = os.environ.get("OPENPAW_API_KEY", "")
HEADERS = {
    "content-type": "application/json",
    "x-tenant-id": TENANT,
    "x-temper-principal-kind": "admin",
    "x-temper-principal-id": "paw-managed-agents-proof",
}
if API_KEY:
    HEADERS["authorization"] = f"Bearer {API_KEY}"


def request(method: str, path: str, body: dict | None = None) -> dict | list | str:
    data = None if body is None else json.dumps(body).encode()
    req = urllib.request.Request(f"{SERVER}{path}", data=data, method=method)
    for key, value in HEADERS.items():
        req.add_header(key, value)
    try:
        with urllib.request.urlopen(req, timeout=30) as response:
            raw = response.read().decode()
            if not raw:
                return {}
            if response.headers.get("content-type", "").startswith("application/json"):
                return json.loads(raw)
            return raw
    except urllib.error.HTTPError as exc:
        detail = exc.read().decode()
        raise RuntimeError(f"{method} {path} failed: HTTP {exc.code}: {detail}") from exc


def entity_fields(payload: dict) -> dict:
    return payload.get("fields", payload)


def field_value(fields: dict, *names: str):
    for name in names:
        if name in fields:
            return fields[name]
    return None


def main() -> int:
    print("== paw-managed-agents proof ==")

    print("Installing app bundle...")
    request(
        "POST",
        "/api/os-apps/paw-managed-agents/install",
        {"tenant": TENANT},
    )

    print("Creating managed environment...")
    env = request(
        "POST",
        "/tdata/ManagedEnvironments",
        {
            "Name": "proof-env",
            "Description": "managed agents proof environment",
            "ConfigType": "Cloud",
            "NetworkingType": "Limited",
            "AllowedHostsJson": "[\"github.com\"]",
            "AllowMcpServers": True,
            "AllowPackageManagers": False,
        },
    )
    env_id = env["entity_id"]

    print("Creating managed agent...")
    agent = request(
        "POST",
        "/tdata/ManagedAgents",
        {
            "Name": "proof-agent",
            "Description": "managed agents proof agent",
            "System": "You are a concise proof assistant.",
            "ModelId": "claude-sonnet-4-6",
            "ModelSpeed": "standard",
            "Version": 1,
        },
    )
    agent_id = agent["entity_id"]

    print("Updating managed agent...")
    request(
        "POST",
        f"/tdata/ManagedAgents('{agent_id}')/ManagedAgents.UpdateManagedAgent?await_integration=true",
        {
            "Description": "managed agents proof agent updated",
            "System": "You are a concise proof assistant. Keep answers to one word.",
            "ModelId": "claude-sonnet-4-6",
            "ModelSpeed": "standard",
        },
    )
    updated_agent = request("GET", f"/tdata/ManagedAgents('{agent_id}')")
    updated_agent_fields = entity_fields(updated_agent)
    if updated_agent_fields.get("Version") != 2:
        raise RuntimeError("managed agent version did not increment after update")
    if updated_agent_fields.get("Description") != "managed agents proof agent updated":
        raise RuntimeError("managed agent description was not updated")

    print("Adding a built-in tool row...")
    tool = request(
        "POST",
        "/tdata/AgentTools",
        {
            "AgentId": agent_id,
            "Kind": "agent_toolset_20260401",
        },
    )
    tool_id = tool["entity_id"]

    print("Adding explicit tool config rows...")
    request(
        "POST",
        "/tdata/AgentToolConfigs",
        {
            "ToolId": tool_id,
            "ToolName": "bash",
            "PermissionPolicy": "always_allow",
        },
    )
    request(
        "POST",
        "/tdata/AgentToolConfigs",
        {
            "ToolId": tool_id,
            "ToolName": "temper_get",
            "PermissionPolicy": "always_ask",
        },
    )

    print("Creating managed session...")
    session = request(
        "POST",
        "/tdata/ManagedSessions",
        {
            "Title": "proof-session",
            "AgentId": agent_id,
            "EnvironmentId": env_id,
            "VaultIds": "[\"vault-proof\"]",
        },
    )
    session_id = session["entity_id"]

    print("Posting initial user event...")
    request(
        "POST",
        "/tdata/SessionEvents",
        {
            "SessionId": session_id,
            "Sequence": 1,
            "Kind": "user.message",
            "Content": json.dumps([{"type": "text", "text": "Reply with the word proof."}]),
        },
    )

    print("Starting session...")
    request(
        "POST",
        f"/tdata/ManagedSessions('{session_id}')/ManagedAgents.StartSession",
        {},
    )

    deadline = time.time() + 90
    idle = None
    while time.time() < deadline:
        current = request("GET", f"/tdata/ManagedSessions('{session_id}')")
        if current.get("status") == "Idle":
            idle = current
            break
        if current.get("status") == "Terminated":
            raise RuntimeError("session terminated before idling")
        time.sleep(2)

    if idle is None:
        raise RuntimeError("managed session did not reach Idle within timeout")

    idle_fields = entity_fields(idle)
    if field_value(idle_fields, "StopReason", "stop_reason") != "user_input_required":
        raise RuntimeError("managed session idle stop reason was not user_input_required")
    if field_value(idle_fields, "VaultIds", "vault_ids") != "[\"vault-proof\"]":
        raise RuntimeError("managed session did not retain VaultIds")

    print("Checking bound computer and inner agent state...")
    env_current = request("GET", f"/tdata/ManagedEnvironments('{env_id}')")
    env_fields = entity_fields(env_current)
    computer_id = env_fields.get("ComputerId")
    if not computer_id:
        raise RuntimeError("managed environment did not bind a ComputerId")

    computer = request("GET", f"/tdata/Computers('{computer_id}')")
    computer_fields = entity_fields(computer)
    if field_value(computer_fields, "Status", "status") != "Provisioning":
        raise RuntimeError("managed environment computer should remain in Provisioning")
    network_allow_raw = field_value(computer_fields, "NetworkAllow", "network_allow") or ""
    network_allow = json.loads(network_allow_raw)
    if network_allow != {
        "allowed_hosts": ["github.com"],
        "allow_mcp_servers": True,
        "allow_package_managers": False,
    }:
        raise RuntimeError("computer network_allow did not reflect managed environment settings")

    managed_agent_current = request("GET", f"/tdata/ManagedAgents('{agent_id}')")
    managed_agent_fields = entity_fields(managed_agent_current)
    inner_agent_id = managed_agent_fields.get("InnerAgentId")
    if not inner_agent_id:
        raise RuntimeError("managed agent did not bind an InnerAgentId")
    inner_agent = request("GET", f"/tdata/Agents('{inner_agent_id}')")
    inner_agent_fields = entity_fields(inner_agent)
    enabled_tools_raw = field_value(inner_agent_fields, "ToolsEnabled", "tools_enabled") or ""
    enabled_tools = set(filter(None, enabled_tools_raw.split(",")))
    if enabled_tools != {"bash", "temper_get"}:
        raise RuntimeError(f"inner agent tools did not match config rows: {enabled_tools}")

    print("Fetching emitted events...")
    filter_expr = urllib.parse.quote(f"SessionId eq '{session_id}'", safe="'()")
    orderby = urllib.parse.quote("Sequence asc")
    events = request(
        "GET",
        f"/tdata/SessionEvents?$filter={filter_expr}&$orderby={orderby}",
    )
    values = events.get("value", [])
    kinds = [item.get("fields", {}).get("Kind") for item in values]
    print("Event kinds:", kinds)

    if "session.status_running" not in kinds:
        raise RuntimeError("running event missing")
    if "session.status_idle" not in kinds:
        raise RuntimeError("idle event missing")
    if "agent.message" not in kinds:
        raise RuntimeError("agent.message missing")

    print("Posting follow-up user event...")
    request(
        "POST",
        "/tdata/SessionEvents",
        {
            "SessionId": session_id,
            "Sequence": len(values) + 1,
            "Kind": "user.message",
            "Content": json.dumps(
                [{"type": "text", "text": "Reply again with the word proof."}]
            ),
        },
    )

    print("Resuming session...")
    request(
        "POST",
        f"/tdata/ManagedSessions('{session_id}')/ManagedAgents.ResumeSession",
        {},
    )

    deadline = time.time() + 90
    resumed_idle = None
    while time.time() < deadline:
        current = request("GET", f"/tdata/ManagedSessions('{session_id}')")
        if current.get("status") == "Idle":
            resumed_idle = current
            break
        if current.get("status") == "Terminated":
            raise RuntimeError("session terminated before second idle")
        time.sleep(2)

    if resumed_idle is None:
        raise RuntimeError("managed session did not reach Idle after resume")

    print("Fetching resumed events...")
    resumed_events = request(
        "GET",
        f"/tdata/SessionEvents?$filter={filter_expr}&$orderby={orderby}",
    )
    resumed_values = resumed_events.get("value", [])
    resumed_kinds = [item.get("fields", {}).get("Kind") for item in resumed_values]
    print("Resumed event kinds:", resumed_kinds)

    if resumed_kinds.count("session.status_running") < 2:
        raise RuntimeError("second running event missing")
    if resumed_kinds.count("session.status_idle") < 2:
        raise RuntimeError("second idle event missing")

    print("Terminating session...")
    request(
        "POST",
        f"/tdata/ManagedSessions('{session_id}')/ManagedAgents.TerminateSession",
        {"TerminationReason": "cancelled"},
    )

    deadline = time.time() + 60
    terminated = None
    while time.time() < deadline:
        current = request("GET", f"/tdata/ManagedSessions('{session_id}')")
        if current.get("status") == "Terminated":
            terminated = current
            break
        time.sleep(2)

    if terminated is None:
        raise RuntimeError("managed session did not terminate within timeout")

    terminated_fields = entity_fields(terminated)
    if field_value(terminated_fields, "TerminationReason", "termination_reason") != "cancelled":
        raise RuntimeError("managed session termination reason was not recorded")

    action_names = [action.get("name") for action in terminated.get("@odata.actions", [])]
    if "Archive" not in action_names:
        raise RuntimeError(f"terminated managed session did not advertise Archive action: {action_names}")
    if "ArchivedAtRequiresTerminatedStatus" in action_names:
        raise RuntimeError("field invariant leaked into @odata.actions")

    archive_target = None
    for action in terminated.get("@odata.actions", []):
        name = action.get("name")
        if name == "Archive":
            archive_target = action.get("target")
            break

    if not archive_target:
        archive_target = f"ManagedSessions('{session_id}')/Temper.Archive"

    print("Archiving session...")
    request(
        "POST",
        f"/tdata/{archive_target}",
        {"ArchivedAt": "2026-04-15T22:30:00Z"},
    )

    print("Checking terminated event semantics...")
    terminated_status_events = []
    event_deadline = time.time() + 30
    while time.time() < event_deadline:
        terminated_events = request(
            "GET",
            f"/tdata/SessionEvents?$filter={filter_expr}&$orderby={orderby}",
        )
        terminated_values = terminated_events.get("value", [])
        terminated_status_events = [
            entity_fields(item)
            for item in terminated_values
            if entity_fields(item).get("Kind") == "session.status_terminated"
        ]
        if len(terminated_status_events) == 1:
            break
        time.sleep(1)

    if len(terminated_status_events) != 1:
        raise RuntimeError(
            f"expected exactly one session.status_terminated event, got {len(terminated_status_events)}"
        )
    if (
        field_value(
            terminated_status_events[0],
            "TerminationReason",
            "termination_reason",
        )
        != "cancelled"
    ):
        raise RuntimeError("terminated event did not store TerminationReason")

    print("Negative check: bogus event kind should fail...")
    try:
        request(
            "POST",
            "/tdata/SessionEvents",
            {"SessionId": session_id, "Sequence": 999, "Kind": "bogus.event"},
        )
    except RuntimeError as exc:
        if "409" not in str(exc):
            raise
        print("Constraint rejection observed as expected.")
    else:
        raise RuntimeError("invalid event kind unexpectedly succeeded")

    print("Negative check: archived session should block child rows...")
    try:
        request(
            "POST",
            "/tdata/SessionResources",
            {
                "SessionId": session_id,
                "Kind": "file",
                "Name": "blocked.txt",
                "MountPath": "/tmp/blocked.txt",
            },
        )
    except RuntimeError as exc:
        if "409" not in str(exc):
            raise
        print("Archive gate rejection observed as expected.")
    else:
        raise RuntimeError("archived session unexpectedly accepted a child row")

    print("Proof completed successfully.")
    return 0


if __name__ == "__main__":
    sys.exit(main())
