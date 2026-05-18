#!/usr/bin/env python3
"""Read a small Datadog observability snapshot for the deployed TemperPaw service.

This helper is read-only by default. The optional --capture-profile flag runs a
short authenticated pprof capture before reading Datadog metrics. It expects
DD_API_KEY, DD_APP_KEY, and optionally DD_SITE in the environment; prefer
running it through `railway run` so secrets do not need to be exported into a
local shell.
"""

from __future__ import annotations

import argparse
import datetime as dt
import hashlib
import os
import sys
import time
from typing import Any

try:
    import requests
except ImportError:
    sys.exit("requests is required: pip install requests")


DEFAULT_SERVICE = os.environ.get("DD_SERVICE", "temperpaw")
DEFAULT_DB_INSTANCE = os.environ.get("TEMPER_DD_DB_INSTANCE", "temperpaw-postgres")
DEFAULT_BASE_URL = os.environ.get(
    "TEMPERPAW_BASE_URL",
    "https://openpaw-production.up.railway.app",
)
DEFAULT_APM_SQL_SPAN_QUERY = (
    "service:temperpaw type:sql @db.system:postgresql @peer.service:temperpaw-postgres"
)
WINDOWS = (("1h", 3600), ("24h", 86400))


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description="Read a bounded Datadog metric/APM snapshot for TemperPaw.",
    )
    parser.add_argument("--service", default=DEFAULT_SERVICE)
    parser.add_argument("--db-instance", default=DEFAULT_DB_INSTANCE)
    parser.add_argument("--site", default=os.environ.get("DD_SITE", "datadoghq.com"))
    parser.add_argument("--span-window-minutes", type=int, default=30)
    parser.add_argument("--span-limit", type=int, default=5)
    parser.add_argument(
        "--skip-spans",
        action="store_true",
        help="Only query metrics; skip the APM SQL span correlation search.",
    )
    parser.add_argument(
        "--capture-profile",
        action="store_true",
        help="Run a short authenticated /_admin/profile/cpu capture before reading metrics.",
    )
    parser.add_argument("--profile-base-url", default=DEFAULT_BASE_URL)
    parser.add_argument("--profile-seconds", type=int, default=5)
    parser.add_argument("--profile-frequency", type=int, default=100)
    parser.add_argument("--tenant", default=os.environ.get("TEMPER_TENANT", "default"))
    return parser.parse_args()


def metric_queries(service: str, db_instance: str) -> dict[str, str]:
    db_tags = f"service:{service},database_instance:{db_instance}"
    return {
        "temper_up": f"avg:temper_up{{service:{service}}}",
        "cedar_evals": f"sum:temper_cedar_evaluations_total{{service:{service}}}.as_count()",
        "cedar_duration_avg_s": f"avg:temper_cedar_evaluation_duration{{service:{service}}}",
        "dispatch_latency_p95_ms": f"p95:temper_dispatch_ask_latency_ms{{service:{service}}}",
        "projection_update_errors": (
            f"sum:temper_query_projection_update_error_total{{service:{service}}}.as_count()"
        ),
        "postgres_pool_p95_ms": (
            f"p95:temper_postgres_pool_acquire_duration_ms{{service:{service}}}"
        ),
        "profiler_uploads": (
            f"sum:datadog.profiling.rust.profiles_uploaded{{service:{service}}}.as_count()"
        ),
        "profiler_errors": (
            f"sum:datadog.profiling.rust.upload_errors{{service:{service}}}.as_count()"
        ),
        "dbm_query_count": f"sum:postgresql.queries.count{{{db_tags}}}.as_count()",
        "dbm_query_time_ns_avg": f"avg:postgresql.queries.time{{{db_tags}}}",
        "dbm_activity_rows": f"sum:datadog.dbm.activity_rows{{{db_tags}}}.as_count()",
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


def datadog_headers(api_key: str, app_key: str) -> dict[str, str]:
    return {
        "DD-API-KEY": api_key,
        "DD-APPLICATION-KEY": app_key,
    }


def apm_sql_span_query(service: str, db_instance: str) -> str:
    if service == "temperpaw" and db_instance == "temperpaw-postgres":
        return DEFAULT_APM_SQL_SPAN_QUERY
    return f"service:{service} type:sql @db.system:postgresql @peer.service:{db_instance}"


def capture_cpu_profile(args: argparse.Namespace) -> None:
    api_key = (
        os.environ.get("TEMPER_API_KEY")
        or os.environ.get("OPENPAW_API_KEY")
        or os.environ.get("API_KEY")
    )
    if not api_key:
        print("profile_capture: skipped missing TEMPER_API_KEY")
        return

    profile_url = (
        f"{args.profile_base_url.rstrip('/')}/_admin/profile/cpu"
        f"?seconds={args.profile_seconds}&frequency={args.profile_frequency}"
    )
    headers = {
        "Accept": "application/vnd.google.protobuf",
        "Authorization": f"Bearer {api_key}",
        "X-Tenant-Id": args.tenant,
        "X-Temper-Principal-Kind": "admin",
    }
    started = time.time()
    response = requests.get(profile_url, headers=headers, timeout=args.profile_seconds + 40)
    elapsed = time.time() - started
    digest = hashlib.sha256(response.content).hexdigest()
    print(
        "profile_capture: "
        f"status={response.status_code} "
        f"content_type={response.headers.get('content-type')} "
        f"bytes={len(response.content)} "
        f"sha256={digest} "
        f"elapsed_s={elapsed:.2f}"
    )
    response.raise_for_status()


def search_apm_sql_spans(
    *,
    site: str,
    headers: dict[str, str],
    service: str,
    db_instance: str,
    window_minutes: int,
    limit: int,
) -> None:
    now = dt.datetime.now(dt.timezone.utc)
    query = apm_sql_span_query(service, db_instance)
    body = {
        "data": {
            "type": "search_request",
            "attributes": {
                "filter": {
                    "from": (now - dt.timedelta(minutes=window_minutes))
                    .isoformat()
                    .replace("+00:00", "Z"),
                    "to": now.isoformat().replace("+00:00", "Z"),
                    "query": query,
                },
                "page": {"limit": limit},
                "sort": "-timestamp",
            },
        }
    }
    response = requests.post(
        f"https://api.{site}/api/v2/spans/events/search",
        headers={**headers, "Content-Type": "application/json"},
        json=body,
        timeout=20,
    )
    if response.status_code >= 400:
        print(f"apm_sql_spans: http_{response.status_code}")
        return

    payload = response.json()
    spans = payload.get("data", [])
    print(
        "apm_sql_spans: "
        f"returned={len(spans)} "
        f"window_minutes={window_minutes} "
        f"query={query!r}"
    )
    for span in spans:
        attributes = span.get("attributes", {})
        custom = attributes.get("custom", {})
        duration_ns = attributes.get("duration") or custom.get("duration")
        duration_ms = None
        if isinstance(duration_ns, (int, float)):
            duration_ms = round(float(duration_ns) / 1_000_000, 3)
        service_version = (
            custom.get("service", {}).get("version")
            if isinstance(custom.get("service"), dict)
            else None
        ) or custom.get("version")
        print(
            "  span: "
            f"trace_id={attributes.get('trace_id')} "
            f"span_id={attributes.get('span_id')} "
            f"duration_ms={duration_ms} "
            f"version={service_version} "
            f"resource_name={attributes.get('resource_name')!r}"
        )


def main() -> int:
    args = parse_args()
    api_key = os.environ.get("DD_API_KEY")
    app_key = os.environ.get("DD_APP_KEY")
    if not api_key or not app_key:
        sys.exit("DD_API_KEY and DD_APP_KEY must be set")

    if args.capture_profile:
        capture_cpu_profile(args)

    now = int(time.time())
    headers = datadog_headers(api_key, app_key)
    base_url = f"https://api.{args.site}/api/v1/query"
    queries = metric_queries(args.service, args.db_instance)

    print(
        "Datadog read-only observability snapshot "
        f"for service:{args.service} database_instance:{args.db_instance}"
    )
    for label, seconds in WINDOWS:
        print(f"window={label}")
        start = now - seconds
        for name, query in queries.items():
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

    if not args.skip_spans:
        search_apm_sql_spans(
            site=args.site,
            headers=headers,
            service=args.service,
            db_instance=args.db_instance,
            window_minutes=args.span_window_minutes,
            limit=args.span_limit,
        )

    return 0


if __name__ == "__main__":
    raise SystemExit(main())
