#!/usr/bin/env python3
"""Read a small Datadog metric snapshot for the deployed TemperPaw service.

This helper is intentionally read-only. It expects DD_API_KEY, DD_APP_KEY, and
optionally DD_SITE in the environment; prefer running it through `railway run`
so secrets do not need to be exported into a local shell.
"""

from __future__ import annotations

import os
import sys
import time
from typing import Any

try:
    import requests
except ImportError:
    sys.exit("requests is required: pip install requests")


SERVICE = os.environ.get("DD_SERVICE", "temperpaw")
WINDOWS = (("1h", 3600), ("24h", 86400))
QUERIES = {
    "temper_up": f"avg:temper_up{{service:{SERVICE}}}",
    "cedar_evals": f"sum:temper_cedar_evaluations_total{{service:{SERVICE}}}.as_count()",
    "cedar_duration_avg_s": f"avg:temper_cedar_evaluation_duration{{service:{SERVICE}}}",
    "dispatch_latency_p95_ms": f"p95:temper_dispatch_ask_latency_ms{{service:{SERVICE}}}",
    "projection_update_errors": f"sum:temper_query_projection_update_error_total{{service:{SERVICE}}}.as_count()",
    "postgres_pool_p95_ms": f"p95:temper_postgres_pool_acquire_duration_ms{{service:{SERVICE}}}",
    "profiler_uploads": f"sum:datadog.profiling.rust.profiles_uploaded{{service:{SERVICE}}}.as_count()",
    "profiler_errors": f"sum:datadog.profiling.rust.upload_errors{{service:{SERVICE}}}.as_count()",
}


def summarize(series: list[dict[str, Any]]) -> dict[str, float | int | None]:
    """Return bounded point statistics without printing raw payloads."""
    values: list[float] = []
    for item in series:
        for _, value in item.get("pointlist", []):
            if value is not None:
                values.append(float(value))

    return {
        "series": len(series),
        "points": len(values),
        "last": values[-1] if values else None,
        "max": max(values) if values else None,
        "sum": sum(values) if values else None,
    }


def main() -> int:
    api_key = os.environ.get("DD_API_KEY")
    app_key = os.environ.get("DD_APP_KEY")
    site = os.environ.get("DD_SITE", "datadoghq.com")
    if not api_key or not app_key:
        sys.exit("DD_API_KEY and DD_APP_KEY must be set")

    now = int(time.time())
    headers = {
        "DD-API-KEY": api_key,
        "DD-APPLICATION-KEY": app_key,
    }
    base_url = f"https://api.{site}/api/v1/query"

    print(f"Datadog read-only metric snapshot for service:{SERVICE}")
    for label, seconds in WINDOWS:
        print(f"window={label}")
        start = now - seconds
        for name, query in QUERIES.items():
            response = requests.get(
                base_url,
                headers=headers,
                params={"from": start, "to": now, "query": query},
                timeout=20,
            )
            if response.status_code >= 400:
                print(f"  {name}: http_{response.status_code}")
                continue

            payload = response.json()
            if payload.get("errors"):
                print(f"  {name}: errors={payload['errors']}")
                continue

            stats = summarize(payload.get("series", []))
            print(
                f"  {name}: "
                f"series={stats['series']} "
                f"points={stats['points']} "
                f"last={stats['last']} "
                f"max={stats['max']} "
                f"sum={stats['sum']}"
            )

    return 0


if __name__ == "__main__":
    raise SystemExit(main())
