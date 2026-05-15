#!/usr/bin/env python3
"""Enable Datadog percentile aggregations for Temper latency distributions.

Datadog exposes p50/p75/p90/p95/p99 queries for distribution metrics only after
their metric tag configuration has `include_percentiles` enabled. This script
keeps that configuration repeatable for the latency-observability program.

The script intentionally excludes known high-cardinality values such as
`session_id` from the queryable tag list. Add a tag only when it is needed for a
dashboard, monitor, or live diagnosis.
"""

from __future__ import annotations

import argparse
import json
import os
import sys
import time
from pathlib import Path
from typing import Iterable

try:
    import requests
except ImportError:
    sys.exit("requests is required: pip install requests")


COMMON_TAGS = {
    "env",
    "host",
    "railway_profile",
    "service",
    "team",
    "version",
}

AUTHZ_TAGS = COMMON_TAGS | {
    "action",
    "decision",
    "entity_type",
    "outcome",
    "phase",
    "tenant",
}

DISPATCH_TAGS = COMMON_TAGS | {
    "action",
    "entity_type",
    "error_kind",
    "outcome",
    "tenant",
}

PROJECTION_TAGS = COMMON_TAGS | {
    "entity_type",
    "operation",
    "outcome",
    "result",
    "source",
    "tenant",
}

POSTGRES_TAGS = COMMON_TAGS | {
    "operation",
    "outcome",
    "tenant",
}

SESSION_TAGS = COMMON_TAGS | {
    "entity_type",
    "phase",
    "result",
    "tenant",
    "trigger_action",
    "wasm_module",
}

WASM_TAGS = COMMON_TAGS | {
    "action",
    "call_kind",
    "entity_type",
    "status_code_class",
    "trigger_action",
    "wasm_module",
}

BLOB_TAGS = COMMON_TAGS | {
    "backend",
    "http_method",
    "operation",
    "outcome",
}

ACTOR_TAGS = COMMON_TAGS | {
    "action",
    "backend",
    "cold_start",
    "entity_type",
    "outcome",
    "tenant",
}

ADMISSION_TAGS = COMMON_TAGS | {
    "action",
    "entity_type",
    "outcome",
    "tenant",
}

METRIC_TAGS = {
    "temper_admission_permit_hold_time_ms": ADMISSION_TAGS,
    "temper_admission_wait_time_ms": ADMISSION_TAGS,
    "temper_actor_ask_reply_latency_ms": ACTOR_TAGS,
    "temper_actor_cold_start_duration_ms": ACTOR_TAGS,
    "temper_actor_registry_lock_wait_ms": ACTOR_TAGS,
    "temper_blob_io_wait_duration_ms": BLOB_TAGS,
    "temper_blob_transport_wait_duration_ms": BLOB_TAGS,
    "temper_cedar_evaluation_duration": AUTHZ_TAGS,
    "temper_cedar_evaluation_duration_ms": AUTHZ_TAGS,
    "temper_cedar_evaluation_phase_duration_ms": AUTHZ_TAGS,
    "temper_dispatch_ask_attempts": DISPATCH_TAGS,
    "temper_dispatch_ask_latency_ms": DISPATCH_TAGS,
    "temper_event_store_append_wait_ms": COMMON_TAGS | {"backend", "outcome"},
    "temper_monty_repl_wait_duration_ms": COMMON_TAGS
    | {"max_concurrency", "outcome"},
    "temper_postgres_pool_acquire_duration_ms": POSTGRES_TAGS,
    "temper_postgres_transaction_duration_ms": POSTGRES_TAGS,
    "temper_query_projection_update_duration_ms": PROJECTION_TAGS,
    "temper_query_projection_update_end_to_end_duration_ms": PROJECTION_TAGS,
    "temper_query_projection_update_queue_wait_ms": PROJECTION_TAGS,
    "temper_query_projection_backfill_duration_ms": PROJECTION_TAGS,
    "temper_query_projection_backfill_replay_events": PROJECTION_TAGS,
    "temper_query_projection_replay_parity_duration_ms": PROJECTION_TAGS,
    "temper_query_projection_replay_parity_sequence_gap": PROJECTION_TAGS,
    "temper_query_projection_shadow_sequence_gap": PROJECTION_TAGS,
    "temper_session_context_prepare_duration_ms": SESSION_TAGS,
    "temper_session_phase_duration_ms": SESSION_TAGS,
    "temper_session_phase_step_duration_ms": SESSION_TAGS,
    "temper_trajectory_outbox_persist_latency_ms": COMMON_TAGS | {"outcome"},
    "temper_wasm_host_http_duration_ms": WASM_TAGS,
    "temper_wasm_invocation_duration_ms": WASM_TAGS,
}


def load_env() -> None:
    """Best-effort .env loading."""
    env_path = Path(__file__).resolve().parent.parent / ".env"
    if not env_path.exists():
        return
    for line in env_path.read_text().splitlines():
        line = line.strip()
        if not line or line.startswith("#") or "=" not in line:
            continue
        key, _, value = line.partition("=")
        os.environ.setdefault(key.strip(), value.strip())


def request(
    method: str,
    url: str,
    headers: dict[str, str],
    *,
    payload: dict | None = None,
) -> requests.Response:
    resp = None
    for attempt in range(6):
        resp = requests.request(method, url, headers=headers, json=payload, timeout=30)
        if resp.status_code != 429:
            break
        retry_after = resp.headers.get("Retry-After")
        if retry_after and retry_after.isdigit():
            sleep_seconds = int(retry_after)
        else:
            sleep_seconds = min(2**attempt, 30)
        time.sleep(sleep_seconds)

    assert resp is not None
    if resp.status_code >= 400 and resp.status_code not in {400, 404, 409}:
        raise RuntimeError(f"{method} {url} failed: {resp.status_code} {resp.text}")
    return resp


def is_missing_metric_response(resp: requests.Response) -> bool:
    if resp.status_code != 400:
        return False
    return "does not exist" in resp.text.lower()


def config_payload(metric_name: str, tags: Iterable[str]) -> dict:
    return {
        "data": {
            "type": "manage_tags",
            "id": metric_name,
            "attributes": {
                "metric_type": "distribution",
                "include_percentiles": True,
                "tags": sorted(tags),
            },
        }
    }


def update_payload(metric_name: str, tags: Iterable[str]) -> dict:
    return {
        "data": {
            "type": "manage_tags",
            "id": metric_name,
            "attributes": {
                "include_percentiles": True,
                "tags": sorted(tags),
            },
        }
    }


def configure_metric(
    base_url: str,
    headers: dict[str, str],
    metric_name: str,
    tags: set[str],
    *,
    apply: bool,
) -> str:
    url = f"{base_url}/metrics/{metric_name}/tags"
    existing = request("GET", url, headers)

    if existing.status_code == 404:
        action = "create"
        desired_tags = tags
        if not apply:
            return f"[dry-run] Would create percentile config: {metric_name} tags={','.join(sorted(desired_tags))}"
        resp = request("POST", url, headers, payload=config_payload(metric_name, desired_tags))
        if is_missing_metric_response(resp):
            return f"Skipped missing metric: {metric_name}"
        if resp.status_code == 409:
            action = "update"
            resp = request("PATCH", url, headers, payload=update_payload(metric_name, desired_tags))
        if resp.status_code >= 400:
            raise RuntimeError(f"{metric_name} {action} failed: {resp.status_code} {resp.text}")
        return f"{action.capitalize()}d percentile config: {metric_name}"

    data = existing.json().get("data", {})
    attributes = data.get("attributes", {})
    existing_tags = set(attributes.get("tags") or [])
    desired_tags = existing_tags | tags
    percentiles_enabled = bool(attributes.get("include_percentiles"))

    if percentiles_enabled and existing_tags == desired_tags:
        return f"Already enabled: {metric_name}"

    if not apply:
        added = ",".join(sorted(desired_tags - existing_tags)) or "<none>"
        return f"[dry-run] Would update percentile config: {metric_name} added_tags={added}"

    resp = request("PATCH", url, headers, payload=update_payload(metric_name, desired_tags))
    if resp.status_code >= 400:
        raise RuntimeError(f"{metric_name} update failed: {resp.status_code} {resp.text}")
    return f"Updated percentile config: {metric_name}"


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--apply",
        action="store_true",
        help="Apply Datadog metric tag configuration changes. Defaults to dry-run.",
    )
    parser.add_argument(
        "--metric",
        action="append",
        choices=sorted(METRIC_TAGS),
        help="Configure only this metric. May be repeated.",
    )
    return parser.parse_args()


def main() -> None:
    args = parse_args()
    load_env()

    api_key = os.environ.get("DD_API_KEY", "")
    app_key = os.environ.get("DD_APP_KEY", "")
    site = os.environ.get("DD_SITE", "datadoghq.com")
    if not api_key or not app_key:
        sys.exit("DD_API_KEY and DD_APP_KEY must be set")

    headers = {
        "DD-API-KEY": api_key,
        "DD-APPLICATION-KEY": app_key,
        "Accept": "application/json",
        "Content-Type": "application/json",
    }
    base_url = f"https://api.{site}/api/v2"
    metric_names = args.metric or sorted(METRIC_TAGS)

    results = []
    for metric_name in metric_names:
        results.append(
            configure_metric(
                base_url,
                headers,
                metric_name,
                set(METRIC_TAGS[metric_name]),
                apply=args.apply,
            )
        )

    for result in results:
        print(result)
    print(json.dumps({"configured_metrics": len(metric_names), "applied": args.apply}))


if __name__ == "__main__":
    main()
