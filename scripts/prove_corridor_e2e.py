#!/usr/bin/env python3
"""Drive one world through the full corridor and record every transition.

Flow: create World -> Configure -> Seed -> (surveyor/bookmaker sessions)
-> Active -> SampleEndpoints -> (writers -> repairs -> challenges -> costing)
-> PathsScored -> forecasts registered -> Render -> artifacts through the
consistency gate.

Usage:
  python3 scripts/prove_corridor_e2e.py --base-url http://127.0.0.1:4500 \
      --api-key <key> --model claude-haiku-4-5-20251001 --provider anthropic \
      [--budget 2] [--far-future] [--timeout-min 45]
"""

from __future__ import annotations

import argparse
import json
import sys
import time
import urllib.error
import urllib.request


class Client:
    def __init__(self, base: str, tenant: str, key: str):
        self.base, self.tenant, self.key = base.rstrip("/"), tenant, key

    def req(self, method: str, path: str, body: dict | None = None):
        req = urllib.request.Request(self.base + path, method=method)
        req.add_header("Authorization", f"Bearer {self.key}")
        req.add_header("X-Tenant-Id", self.tenant)
        req.add_header("X-Temper-Principal-Kind", "admin")
        data = None
        if body is not None:
            req.add_header("Content-Type", "application/json")
            data = json.dumps(body).encode()
        try:
            with urllib.request.urlopen(req, data=data, timeout=120) as r:
                return json.loads(r.read().decode())
        except urllib.error.HTTPError as e:
            raise RuntimeError(f"{method} {path} -> HTTP {e.code}: {e.read().decode()[:300]}")

    def create(self, entity_set: str, fields: dict):
        return self.req("POST", f"/tdata/{entity_set}", fields)

    def action(self, entity_set: str, eid: str, action: str, params: dict):
        return self.req("POST", f"/tdata/{entity_set}('{eid}')/TemperPaw.{action}", params)

    def get(self, entity_set: str, eid: str):
        return self.req("GET", f"/tdata/{entity_set}('{eid}')")

    def list(self, entity_set: str, flt: str | None = None):
        q = f"?$filter={urllib.request.quote(flt)}" if flt else ""
        return self.req("GET", f"/tdata/{entity_set}{q}").get("value", [])


def field(row: dict, name_snake: str, name_pascal: str):
    fields = row.get("fields") or {}
    return fields.get(name_snake) or row.get(name_pascal) or ""


def status_of(row: dict) -> str:
    return row.get("status") or row.get("Status") or ""


def wait(label: str, fn, timeout_s: int, interval_s: int = 20):
    start = time.time()
    while time.time() - start < timeout_s:
        result = fn()
        if result is not None:
            print(f"[{int(time.time() - start)}s] {label}: {result}")
            return result
        elapsed = int(time.time() - start)
        if elapsed % 120 < interval_s:
            print(f"[{elapsed}s] waiting: {label}")
        time.sleep(interval_s)
    raise TimeoutError(f"timed out waiting for {label} after {timeout_s}s")


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--base-url", required=True)
    ap.add_argument("--tenant", default="default")
    ap.add_argument("--api-key", required=True)
    ap.add_argument("--model", required=True)
    ap.add_argument("--provider", required=True)
    ap.add_argument("--budget", default="2")
    ap.add_argument("--far-future", action="store_true")
    ap.add_argument("--timeout-min", type=int, default=45)
    ap.add_argument("--skip-dwellers", action="store_true", help="stop before the living-worlds pass")
    args = ap.parse_args()

    c = Client(args.base_url, args.tenant, args.api_key)
    t = args.timeout_min * 60

    if args.far_future:
        cfg = {
            "name": "After the Toolmakers — 2045",
            "domain": "software creation and AI tooling, two decades out",
            "description": "A far-future companion world: what building software is by 2045, "
            "for the fiction path. Tail samples welcome; the on-ramp must respect the skeleton.",
            "horizon_months": "228",
            "target_date": "2045-06-11",
            "frontier_date": "2026-12-11",
        }
    else:
        cfg = {
            "name": "AI coding tools — six months out",
            "domain": "the AI coding tools market (agents, IDEs, CLIs, code review)",
            "description": "Near-term decision world: the AI coding tools market through 2026-12-11.",
            "horizon_months": "6",
            "target_date": "2026-12-11",
            "frontier_date": "2026-12-11",
        }
    cfg.update(
        {
            "endpoint_budget": args.budget,
            "agent_model": args.model,
            "agent_provider": args.provider,
            "hindcast_mode": "false",
        }
    )

    print("=== 1. Create + Configure + Seed ===")
    world = c.create("Worlds", {"name": cfg["name"], "domain": cfg["domain"]})
    wid = world["entity_id"]
    print(f"world: {wid}")
    c.action("Worlds", wid, "Configure", cfg)
    c.action("Worlds", wid, "Seed", {})

    print("=== 2. Wait for skeleton (surveyor reports SeedComplete) ===")

    def seeded():
        w = c.get("Worlds", wid)
        s = status_of(w)
        if s == "Active":
            return f"Active, skeleton_node_count={field(w, 'skeleton_node_count', 'SkeletonNodeCount')}"
        if s == "Failed":
            raise RuntimeError(f"world failed during seeding: {field(w, 'error_message', 'ErrorMessage')}")
        return None

    wait("seed complete", seeded, t)
    nodes = c.list("EventNodes", f"world_id eq '{wid}'")
    by_prov: dict[str, int] = {}
    for n in nodes:
        by_prov[field(n, "provenance", "Provenance") or "?"] = (
            by_prov.get(field(n, "provenance", "Provenance") or "?", 0) + 1
        )
    print(f"event nodes: {len(nodes)} by provenance {by_prov}")

    print("=== 3. SampleEndpoints -> corridor ===")
    c.action("Worlds", wid, "SampleEndpoints", {})

    def pass_settled():
        w = c.get("Worlds", wid)
        if status_of(w) == "Failed":
            raise RuntimeError(f"world failed: {field(w, 'error_message', 'ErrorMessage')}")
        cp = field(w, "canonical_path_id", "CanonicalPathId")
        if cp:
            return f"canonical path {cp}"
        return None

    wait("corridor pass settled (PathsScored)", pass_settled, t)

    paths = c.list("Paths", f"world_id eq '{wid}'")
    for p in paths:
        print(
            f"  path {p.get('entity_id') or p.get('Id')}: {status_of(p)} "
            f"cost={field(p, 'repair_cost', 'RepairCost')}"
        )
    endpoints = c.list("Endpoints", f"world_id eq '{wid}'")
    for e in endpoints:
        print(
            f"  endpoint {e.get('entity_id') or e.get('Id')}: {status_of(e)} "
            f"weight={field(e, 'weight', 'Weight')}"
        )

    print("=== 3b. Claims (per-claim verdicts, ADR-004) ===")
    claims = c.list("Claims", f"world_id eq '{wid}'")
    for cl in claims:
        text = field(cl, "current_text", "CurrentText") or field(cl, "original_text", "OriginalText")
        amended = " [AMENDED]" if field(cl, "amendment_log", "AmendmentLog") not in ("", "[]") else ""
        print(
            f"  claim {cl.get('entity_id') or cl.get('Id')}: {status_of(cl)} "
            f"{field(cl, 'classification', 'Classification')} "
            f"cost={field(cl, 'best_route_cost', 'BestRouteCost')} "
            f"routes={field(cl, 'route_count', 'RouteCount')}{amended} | {text[:70]}"
        )
    print(f"claims: {len(claims)}")

    print("=== 4. Forecasts (the scoreboard) ===")
    forecasts = c.list("Forecasts", f"world_id eq '{wid}'")
    for f in forecasts:
        print(
            f"  [{field(f, 'probability', 'Probability')}] {field(f, 'question', 'Question')[:90]} "
            f"(by {field(f, 'resolve_by', 'ResolveBy')})"
        )
    print(f"forecasts registered: {len(forecasts)}")

    print("=== 5. Render -> consistency gate ===")
    c.action("Worlds", wid, "Render", {})

    def published():
        arts = c.list("Artifacts", f"world_id eq '{wid}'")
        done = [a for a in arts if status_of(a) in ("ConsistencyChecked", "Published")]
        states = {status_of(a) for a in arts}
        if done:
            return f"{len(done)}/{len(arts)} artifacts through the gate (states: {states})"
        return None

    wait("artifacts through the consistency gate", published, t)
    arts = c.list("Artifacts", f"world_id eq '{wid}'")
    for a in arts:
        print(
            f"  artifact {a.get('entity_id') or a.get('Id')}: {status_of(a)} kind="
            f"{field(a, 'kind', 'Kind')} title={field(a, 'title', 'Title')[:60]}"
        )

    print("=== 6. AnimateDwellers -> traversals, contradictions, stories ===")
    if args.skip_dwellers:
        print("  skipped (--skip-dwellers)")
        dwellers, stories = [], []
    else:
        c.action("Worlds", wid, "AnimateDwellers", {})

        def stories_published():
            arts2 = c.list("Artifacts", f"world_id eq '{wid}'")
            st = [
                a
                for a in arts2
                if field(a, "kind", "Kind") == "story" and status_of(a) == "Published"
            ]
            dws = c.list("Dwellers", f"world_id eq '{wid}'")
            if st and dws:
                return f"{len(st)} story(ies) Published by {len(dws)} dweller(s)"
            return None

        wait("dweller stories through the gate", stories_published, t)
        dwellers = c.list("Dwellers", f"world_id eq '{wid}'")
        for d in dwellers:
            print(
                f"  dweller {d.get('entity_id') or d.get('Id')}: {field(d, 'name', 'Name')} "
                f"({field(d, 'role', 'Role')}) traversals={field(d, 'traversal_count', 'TraversalCount')} "
                f"contradictions={field(d, 'contradiction_count', 'ContradictionCount')}"
            )
        stories = [
            a
            for a in c.list("Artifacts", f"world_id eq '{wid}'")
            if field(a, "kind", "Kind") == "story"
        ]
        for s in stories:
            print(
                f"  story {s.get('entity_id') or s.get('Id')}: {status_of(s)} "
                f"\"{field(s, 'title', 'Title')[:60]}\" by dweller "
                f"{field(s, 'author_dweller_id', 'AuthorDwellerId')}"
            )

    print("=== SUMMARY ===")
    print(
        json.dumps(
            {
                "world_id": wid,
                "event_nodes": len(nodes),
                "endpoints": len(endpoints),
                "paths": len(paths),
                "forecasts": len(forecasts),
                "artifacts": len(arts),
                "claims": len(claims),
                "dwellers": len(dwellers),
                "stories": len(stories),
            },
            indent=2,
        )
    )
    return 0


if __name__ == "__main__":
    sys.exit(main())
