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
            "NetworkingType": "Unrestricted",
            "AllowedHostsJson": "[]",
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

    print("Adding a built-in tool row...")
    request(
        "POST",
        "/tdata/AgentTools",
        {
            "AgentId": agent_id,
            "Kind": "agent_toolset",
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
        {"stop_reason": "error"},
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

    archive_target = None
    for action in terminated.get("@odata.actions", []):
        name = action.get("name")
        if name in {"ArchiveManagedSession", "ArchivedRequiresTerminatedState", "Archive"}:
            archive_target = action.get("target")
            break

    if not archive_target:
        raise RuntimeError("archive action target missing from terminated session metadata")

    print("Archiving session...")
    request(
        "POST",
        f"/tdata/{archive_target}",
        {"archived_at": "2026-04-15T22:30:00Z"},
    )

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
                "Path": "/tmp/blocked.txt",
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
