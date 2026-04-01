#!/usr/bin/env python3
"""Prove Paw can orchestrate managed-project setup through a Channel."""

from __future__ import annotations

import argparse
import json
import os
import time

from openpaw_proof_support import (
    DEFAULT_BASE_URL,
    DEFAULT_MODEL,
    DEFAULT_REPO_URL,
    DEFAULT_TENANT,
    ODataClient,
    ReplyCollector,
    derive_local_sandbox_url,
    entity_id,
    nested_str,
    require,
)


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--base-url", default=os.getenv("OPENPAW_BASE_URL", DEFAULT_BASE_URL))
    parser.add_argument("--tenant", default=os.getenv("PAW_TENANT", DEFAULT_TENANT))
    parser.add_argument("--model", default=DEFAULT_MODEL)
    parser.add_argument("--repo-url", default=DEFAULT_REPO_URL)
    parser.add_argument("--sandbox-url", default=os.getenv("OPENPAW_SANDBOX_URL"))
    parser.add_argument("--timeout-secs", type=float, default=300.0)
    args = parser.parse_args()

    sandbox_url = args.sandbox_url or derive_local_sandbox_url(args.base_url)
    client = ODataClient(
        base_url=args.base_url,
        tenant=args.tenant,
        api_key=os.getenv("TEMPER_API_KEY") or None,
    )

    collector = ReplyCollector()
    collector.start()
    before_harnesses = len(client.list("Harnesses"))
    before_monitors = len(client.list("Monitors"))
    before_agents = len(client.list("Agents"))
    channel_name = f"paw-orchestration-{int(time.time())}"
    thread_id = f"thread-{int(time.time())}"

    try:
        channel = client.create("Channels")
        channel_id = entity_id(channel)
        require(channel_id, "failed to create Channel")
        client.action(
            "Channels",
            channel_id,
            "Paw.Channel.Configure",
            {
                "channel_type": "webhook-proof",
                "channel_id": channel_name,
                "guild_id": "",
                "default_agent_config": "",
                "webhook_secret": "",
                "webhook_url": collector.url,
            },
        )
        client.action("Channels", channel_id, "Paw.Channel.Connect", {})
        client.wait_for_status("Channels", channel_id, "Connected", timeout_secs=15.0)

        route = client.create("AgentRoutes")
        route_id = entity_id(route)
        require(route_id, "failed to create AgentRoute")
        route_config = {
            "model": args.model,
            "provider": "anthropic",
            "tools_enabled": "temper_create,temper_get,temper_list,temper_action,spawn_agent,read_entity,save_memory",
            "max_turns": "24",
            "workdir": "/tmp/paw-orchestration-proof",
            "sandbox_url": sandbox_url,
            "temper_api_url": args.base_url.rstrip("/"),
            "max_follow_ups": "8",
        }
        client.action(
            "AgentRoutes",
            route_id,
            "Paw.Channel.Register",
            {
                "binding_tier": "channel",
                "channel_id": channel_name,
                "guild_id": "",
                "match_pattern": "",
                "agent_config": json.dumps(route_config, separators=(",", ":")),
                "soul_id": "Paw",
            },
        )

        client.action(
            "Channels",
            channel_id,
            "Paw.Channel.ReceiveMessage",
            {
                "message_id": "msg-1",
                "author_id": "proof-user",
                "thread_id": thread_id,
                "content": (
                    "Manage deep-sci-fi for me. Use the normal Open Paw setup for a managed repo: "
                    "a harness, a developer, and monitors. The demo repo is "
                    f"{args.repo_url} if you need the exact URL."
                ),
            },
        )
        reply = collector.wait_for_reply(thread_id, timeout_secs=args.timeout_secs)
        paw_agent_id = str(reply.get("agent_entity_id") or "")
        require(paw_agent_id, f"missing Paw agent id in reply: {reply}")

        deadline = time.time() + args.timeout_secs
        harness = None
        developer_agent = None
        monitors = []
        while time.time() < deadline:
            harnesses = client.list(
                "Harnesses",
                filter_expr=f"repo_url eq '{args.repo_url}'",
                orderby="sequence_nr desc",
                top=5,
            )
            monitors = client.list("Monitors", orderby="sequence_nr desc", top=20)
            agents = client.list("Agents", orderby="sequence_nr desc", top=50)
            harness = next(iter(harnesses), None)
            developer_agent = next(
                (
                    agent
                    for agent in agents
                    if nested_str(agent, ["soul_id", "SoulId"]) == "Developer"
                    and nested_str(agent, ["parent_agent_id", "ParentAgentId"]) == paw_agent_id
                ),
                None,
            )
            if harness and developer_agent and len(monitors) > before_monitors:
                break
            time.sleep(2)

        require(harness is not None, "Paw did not create or reuse the deep-sci-fi Harness")
        require(
            len(client.list("Harnesses")) > before_harnesses,
            "Harness count did not change during Paw orchestration proof",
        )
        require(
            len(client.list("Agents")) > before_agents,
            "Agent count did not change during Paw orchestration proof",
        )
        require(
            len(client.list("Monitors")) > before_monitors,
            "Monitor count did not change during Paw orchestration proof",
        )
        require(developer_agent is not None, "Paw did not spawn a Developer child agent")

        summary = {
            "channel_id": channel_id,
            "route_id": route_id,
            "paw_agent_id": paw_agent_id,
            "reply": reply,
            "project_harness": harness,
            "developer_agent": developer_agent,
            "monitor_count_after": len(client.list("Monitors")),
        }
        print(json.dumps(summary, indent=2))
        return 0
    finally:
        collector.close()


if __name__ == "__main__":
    raise SystemExit(main())
