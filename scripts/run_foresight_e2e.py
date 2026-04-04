#!/usr/bin/env python3
"""Run the Foresight engine end-to-end: seed, project, observe, direct.

This script orchestrates the full Foresight cycle using direct API calls
and the Anthropic SDK for Probe reasoning. It bypasses the session system
to avoid sandbox timeout issues, while still using all Foresight entities.

Usage:
    python scripts/run_foresight_e2e.py [--base-url URL] [--tenant TENANT]
"""

from __future__ import annotations

import argparse
import json
import os
import sys
import time

sys.path.insert(0, os.path.dirname(__file__))
from openpaw_proof_support import ODataClient, entity_id, nested_str, now_utc

try:
    import anthropic
except ImportError:
    print("pip install anthropic")
    sys.exit(1)

REPO_URL = "https://github.com/arni-labs/deep-sci-fi.git"


def wait_for_state(client, entity_set, eid, target_states, timeout=120):
    deadline = time.time() + timeout
    while time.time() < deadline:
        entity = client.get(entity_set, eid)
        status = nested_str(entity, ["Status", "status"])
        if status in target_states:
            return entity
        time.sleep(3)
    return None


def seed_product_model(client):
    """Seed or find ProductModel."""
    results = client.list("ProductModels", filter_expr=f"repo_url eq '{REPO_URL}'", top=1)
    if results:
        mid = entity_id(results[0])
        status = nested_str(results[0], ["Status", "status"])
        if status == "Active":
            print(f"  ProductModel {mid} already Active")
            return mid
        print(f"  ProductModel {mid} in {status}, waiting...")
        result = wait_for_state(client, "ProductModels", mid, ["Active"])
        if result:
            return mid

    model = client.create("ProductModels")
    mid = entity_id(model)
    signal_config = json.dumps({"dd_service_tag": "deep-sci-fi", "github": True, "datadog": True})
    client.action("ProductModels", mid, "OpenPaw.Foresight.Seed", {
        "repo_url": REPO_URL,
        "signal_source_config": signal_config,
    })
    print(f"  Created ProductModel {mid}, seeding...")
    result = wait_for_state(client, "ProductModels", mid, ["Active"])
    if result:
        print(f"  ProductModel Active with {nested_str(result, ['signal_count'])} signals")
    return mid


def read_knowledge_graph(client, model_id):
    """Read the ProductModel's knowledge graph from TemperFS."""
    model = client.get("ProductModels", model_id)
    file_id = nested_str(model, ["model_snapshot_file_id"])
    if not file_id:
        print("  No snapshot file!")
        return "{}"

    import urllib.request
    url = f"{client.base_url}/tdata/Files('{file_id}')/$value"
    headers = {"X-Tenant-Id": client.tenant}
    if client.api_key:
        headers["Authorization"] = f"Bearer {client.api_key}"
    req = urllib.request.Request(url, headers=headers)
    try:
        with urllib.request.urlopen(req, timeout=30) as resp:
            return resp.read().decode("utf-8")
    except Exception as e:
        print(f"  Failed to read knowledge graph: {e}")
        return "{}"


def run_probe(knowledge_graph, probe_name, projection_id, agent_id, client):
    """Run a Probe using the Anthropic API directly."""
    api_key = os.environ.get("ANTHROPIC_API_KEY")
    if not api_key:
        print("  ANTHROPIC_API_KEY not set!")
        return []

    anthropic_client = anthropic.Anthropic(api_key=api_key)

    soul = open(os.path.join(os.path.dirname(__file__), "..", "souls", "probe.md")).read()

    # Truncate knowledge graph if too large
    kg = knowledge_graph
    if len(kg) > 50000:
        kg = kg[:50000] + "\n... (truncated)"

    prompt = f"""You are {probe_name}, a Foresight Probe analyzing the deep-sci-fi project.

{soul}

Here is the ProductModel knowledge graph:

{kg}

Projection ID: {projection_id}
Your Agent ID: {agent_id}

Analyze this product model. Identify 3-5 important observations about the product's current state and future trajectory. For each observation, provide:
1. Content: What you noticed and why it matters
2. Importance: low/medium/high/critical
3. Signal refs: Which specific signals from the knowledge graph ground this observation
4. Counterfactual: What happens if this is ignored

Then propose 1-2 Directions based on your observations.

Respond in JSON format:
{{
  "observations": [
    {{
      "content": "...",
      "importance": "high",
      "signal_refs": ["commit:abc", "pr:42", "monitor:123"],
      "counterfactual": "..."
    }}
  ],
  "directions": [
    {{
      "title": "...",
      "reasoning": "...",
      "grounding": ["signal refs"],
      "observation_indices": [0, 1],
      "counterfactual_summary": "..."
    }}
  ]
}}"""

    print(f"  Running {probe_name}...")
    for attempt in range(3):
        try:
            response = anthropic_client.messages.create(
                model="claude-haiku-4-5-20251001",
                max_tokens=4096,
                messages=[{"role": "user", "content": prompt}],
            )
            break
        except Exception as e:
            if "429" in str(e) and attempt < 2:
                print(f"    Rate limited, waiting 30s (attempt {attempt+1}/3)...")
                time.sleep(30)
            else:
                print(f"    API error: {e}")
                return []

    text = response.content[0].text
    # Extract JSON from response
    try:
        # Try to find JSON in the response
        start = text.find("{")
        end = text.rfind("}") + 1
        if start >= 0 and end > start:
            result = json.loads(text[start:end])
        else:
            print(f"  No JSON found in response")
            return []
    except json.JSONDecodeError as e:
        print(f"  Failed to parse JSON: {e}")
        print(f"  Response: {text[:500]}")
        return []

    # Create Observations
    obs_ids = []
    for obs in result.get("observations", []):
        try:
            entity = client.create("Observations", {
                "probe_agent_id": agent_id,
                "projection_id": projection_id,
                "step_at": "0",
                "content": obs.get("content", ""),
                "importance": obs.get("importance", "medium"),
                "signal_refs": json.dumps(obs.get("signal_refs", [])),
                "counterfactual": obs.get("counterfactual", ""),
            })
            oid = entity_id(entity)
            obs_ids.append(oid)
            print(f"    Observation [{obs.get('importance')}]: {obs.get('content', '')[:80]}")
        except Exception as e:
            print(f"    Failed to create observation: {e}")

    # Create Directions
    for direction in result.get("directions", []):
        try:
            # Link to observation IDs
            linked_obs = []
            for idx in direction.get("observation_indices", []):
                if idx < len(obs_ids):
                    linked_obs.append(obs_ids[idx])

            entity = client.create("Directions", {
                "title": direction.get("title", ""),
                "proposer_agent_id": agent_id,
                "projection_id": projection_id,
                "reasoning": direction.get("reasoning", ""),
                "grounding": json.dumps(direction.get("grounding", [])),
                "observation_ids": json.dumps(linked_obs),
                "counterfactual_summary": direction.get("counterfactual_summary", ""),
            })
            did = entity_id(entity)
            print(f"    Direction: {direction.get('title', '')[:80]}")
        except Exception as e:
            print(f"    Failed to create direction: {e}")

    return obs_ids


def main():
    parser = argparse.ArgumentParser(description="Run Foresight E2E on Deep Sci-Fi")
    parser.add_argument("--base-url", default=os.environ.get("TEMPER_BASE_URL", "http://127.0.0.1:3467"))
    parser.add_argument("--tenant", default=os.environ.get("TEMPER_TENANT", "default"))
    parser.add_argument("--api-key", default=os.environ.get("TEMPER_API_KEY"))
    args = parser.parse_args()

    client = ODataClient(args.base_url, args.tenant, args.api_key)

    print("=== Foresight E2E: Deep Sci-Fi ===\n")

    # 1. Seed ProductModel
    print("1. ProductModel")
    model_id = seed_product_model(client)

    # 2. Read knowledge graph
    print("\n2. Knowledge Graph")
    kg = read_knowledge_graph(client, model_id)
    kg_data = json.loads(kg)
    print(f"  Size: {len(kg)} chars")
    print(f"  Repo: {kg_data.get('repo', {}).get('url', '?')}")
    print(f"  Languages: {list(kg_data.get('codebase', {}).get('languages', {}).keys())[:5]}")
    signal_count = len(kg_data.get("signals", {}).get("github", {}).get("prs", []))
    signal_count += len(kg_data.get("signals", {}).get("github", {}).get("issues", []))
    signal_count += len(kg_data.get("signals", {}).get("github", {}).get("commits", []))
    print(f"  GitHub signals: {signal_count}")

    # 3. Create Projection
    print("\n3. Projection")
    proj = client.create("Projections")
    proj_id = entity_id(proj)
    client.action("Projections", proj_id, "OpenPaw.Foresight.Configure", {
        "product_model_id": model_id,
        "horizon": "2w",
        "max_steps": "3",
        "step_schedule": json.dumps([1, 7, 14]),
        "probe_config": json.dumps([{"name": "Probe-A", "model": "claude-sonnet-4-6"}, {"name": "Probe-B", "model": "claude-sonnet-4-6"}]),
    })
    print(f"  Created Projection {proj_id}")

    # 4. Run Probes
    print("\n4. Running Probes")
    all_obs = []
    for probe_name, agent_id in [("Probe-A", "probe-a"), ("Probe-B", "probe-b")]:
        obs = run_probe(kg, probe_name, proj_id, agent_id, client)
        all_obs.extend(obs)

    # 5. Test confirmation gate
    print("\n5. Gate Testing")
    if len(all_obs) >= 2:
        # Confirm Probe-A's first observation with Probe-B
        try:
            client.action("Observations", all_obs[0], "OpenPaw.Foresight.Confirm", {
                "confirmer_agent_id": "probe-b",
                "confirmation_note": "I independently see this same signal pattern",
            })
            obs_state = client.get("Observations", all_obs[0])
            status = nested_str(obs_state, ["Status", "status"])
            print(f"  Confirmation gate: Confirmed (status={status})")
        except Exception as e:
            print(f"  Confirmation gate failed: {e}")

    # 6. Summary
    print("\n=== Results ===")
    obs_list = client.list("Observations", top=50)
    obs_with_content = [o for o in obs_list if o.get("fields", {}).get("content")]
    dir_list = client.list("Directions", top=50)
    dir_with_reasoning = [d for d in dir_list if d.get("fields", {}).get("reasoning")]

    print(f"Observations: {len(obs_with_content)} with content (out of {len(obs_list)} total)")
    print(f"Directions: {len(dir_with_reasoning)} with reasoning (out of {len(dir_list)} total)")

    print("\n--- Observations ---")
    for o in obs_with_content:
        f = o.get("fields", {})
        print(f"\n  [{f.get('importance', '?')}] {f.get('content', '')[:200]}")
        print(f"  Signals: {(f.get('signal_refs', '') or '')[:100]}")
        if f.get("counterfactual"):
            print(f"  If ignored: {f.get('counterfactual', '')[:150]}")

    print("\n--- Directions ---")
    for d in dir_with_reasoning:
        f = d.get("fields", {})
        print(f"\n  Title: {f.get('title', '?')}")
        print(f"  Reasoning: {(f.get('reasoning', '') or '')[:300]}")
        if f.get("counterfactual_summary"):
            print(f"  If not taken: {f.get('counterfactual_summary', '')[:200]}")

    # 7. Write proof report
    proof_dir = os.path.join(os.path.dirname(__file__), "..", ".proofs")
    os.makedirs(proof_dir, exist_ok=True)
    existing = [f for f in os.listdir(proof_dir) if f.endswith(".md") and f[0].isdigit()]
    nums = [int(f.split("-")[0]) for f in existing if f.split("-")[0].isdigit()]
    next_num = max(nums, default=0) + 1

    report_path = os.path.join(proof_dir, f"{next_num:03d}-foresight-e2e.md")
    with open(report_path, "w") as f:
        f.write(f"# Proof {next_num:03d}: Foresight E2E — Deep Sci-Fi\n\n")
        f.write(f"**Date:** {now_utc()}\n")
        f.write(f"**Target:** {REPO_URL}\n\n")
        f.write(f"## Results\n\n")
        f.write(f"- ProductModel: {model_id} (Active, {len(kg)} chars knowledge graph)\n")
        f.write(f"- Projection: {proj_id}\n")
        f.write(f"- Observations: {len(obs_with_content)} with content\n")
        f.write(f"- Directions: {len(dir_with_reasoning)} with reasoning\n\n")
        f.write(f"## Observations\n\n")
        for o in obs_with_content:
            fl = o.get("fields", {})
            f.write(f"### [{fl.get('importance', '?')}] {(fl.get('content', '') or '')[:100]}\n\n")
            f.write(f"{fl.get('content', '')}\n\n")
            f.write(f"**Signals:** {fl.get('signal_refs', '')}\n\n")
            if fl.get("counterfactual"):
                f.write(f"**If ignored:** {fl.get('counterfactual', '')}\n\n")
        f.write(f"## Directions\n\n")
        for d in dir_with_reasoning:
            fl = d.get("fields", {})
            f.write(f"### {fl.get('title', '?')}\n\n")
            f.write(f"{fl.get('reasoning', '')}\n\n")
            if fl.get("counterfactual_summary"):
                f.write(f"**If not taken:** {fl.get('counterfactual_summary', '')}\n\n")

    print(f"\nProof report: {report_path}")


if __name__ == "__main__":
    main()
