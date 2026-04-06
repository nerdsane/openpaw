#!/usr/bin/env python3
"""Run live Foresight concurrency benchmarks against a local OpenPaw daemon.

This drives real Foresight Probe sessions, not a synthetic WASM-only loop.
Each case restarts the daemon with a different `TEMPER_MONTY_REPL_MAX_CONCURRENCY`,
creates a fresh Projection with a configurable probe fanout, then watches RSS
and Probe turn progress until the case succeeds or a failure threshold is hit.
"""

from __future__ import annotations

import argparse
import json
import os
import signal
import subprocess
import time
from pathlib import Path

from openpaw_proof_support import ODataClient, entity_id
from seed_foresight import seed_product_model, wait_for_state

DEFAULT_BASE_URL = "http://127.0.0.1:3467"
DEFAULT_TENANT = "default"
DEFAULT_RSS_FAIL_MB = 4096
DEFAULT_TIMEOUT_SECS = 360
DEFAULT_CHECK_EVERY_SECS = 10
DEFAULT_TARGET_MIN_TURN = 8
DEFAULT_METRICS_INTERVAL_SECS = 2


def kill_port(port: int) -> None:
    try:
        pids = subprocess.check_output(
            ["lsof", "-i", f":{port}", "-t"], text=True
        ).split()
    except subprocess.CalledProcessError:
        return

    for pid in pids:
        try:
            os.kill(int(pid), signal.SIGKILL)
        except ProcessLookupError:
            continue


def wait_for_daemon(client: ODataClient, timeout_secs: int = 60) -> bool:
    deadline = time.time() + timeout_secs
    while time.time() < deadline:
        try:
            client.list("Sessions", top=1)
            return True
        except Exception:
            time.sleep(1)
    return False


def read_rss_mb(pid: int) -> float | None:
    try:
        rss = subprocess.check_output(
            ["ps", "-o", "rss=", "-p", str(pid)], text=True
        ).strip()
    except subprocess.CalledProcessError:
        return None
    if not rss:
        return None
    return float(rss) / 1024.0


def start_daemon(
    root: Path,
    *,
    base_env: dict[str, str],
    version: str,
    max_concurrency: int,
    metrics_interval_secs: int,
) -> tuple[subprocess.Popen[str], Path]:
    log_path = root / f"tmp_openpaw_c{max_concurrency}.log"
    logf = open(log_path, "w")
    env = base_env.copy()
    env.update(
        {
            "OTEL_ENABLED": "true",
            "DD_ENV": env.get("DD_ENV", "local"),
            "DD_VERSION": version,
            "TEMPER_RUNTIME_METRICS_INTERVAL_SECS": str(metrics_interval_secs),
            "TEMPER_MONTY_REPL_MAX_CONCURRENCY": str(max_concurrency),
            "RUST_LOG": env.get("RUST_LOG", "info"),
        }
    )
    proc = subprocess.Popen(
        ["./target/release/openpaw"],
        cwd=root,
        env=env,
        stdout=logf,
        stderr=subprocess.STDOUT,
        text=True,
    )
    return proc, log_path


def create_projection_with_probes(
    client: ODataClient,
    *,
    probe_count: int,
    base_url: str,
) -> list[str]:
    subprocess.run(
        [
            "python3",
            "scripts/seed_foresight.py",
            "--base-url",
            base_url,
            "--tenant",
            client.tenant,
            "--start",
            "--new-projection",
            "--probe-count",
            str(probe_count),
        ],
        cwd=Path(__file__).resolve().parents[1],
        check=True,
        text=True,
    )

    deadline = time.time() + 120
    session_ids: list[str] = []
    while time.time() < deadline:
        sessions = client.list("Sessions", top=probe_count)
        session_ids = [
            entity_id(s)
            for s in sorted(sessions, key=lambda s: entity_id(s))[-probe_count:]
        ]
        if len(session_ids) >= probe_count:
            return session_ids
        time.sleep(1)
    return session_ids


def monitor_case(
    client: ODataClient,
    *,
    pid: int,
    session_ids: list[str],
    rss_fail_mb: int,
    timeout_secs: int,
    check_every_secs: int,
    target_min_turn: int,
) -> dict[str, object]:
    samples: list[dict[str, object]] = []
    start = time.time()
    failed_sessions: list[dict[str, str | None]] = []

    while time.time() - start < timeout_secs:
        rss_mb = read_rss_mb(pid)
        turns = []
        failed = []

        for sid in session_ids:
            entity = client.get("Sessions", sid)
            status = entity.get("status")
            turn_count = entity.get("fields", {}).get("turn_count")
            turns.append(
                {
                    "id": sid,
                    "status": status,
                    "turn_count": turn_count,
                }
            )
            if status == "Failed":
                failed.append(
                    {
                        "id": sid,
                        "error": entity.get("fields", {}).get("error")
                        or entity.get("fields", {}).get("error_message"),
                    }
                )

        active_turns = [
            int(t["turn_count"] or 0)
            for t in turns
            if t["status"] != "Failed"
        ]
        samples.append(
            {
                "t": round(time.time() - start, 1),
                "rss_mb": rss_mb,
                "alive_sessions": len(turns),
                "failed_count": len(failed),
                "min_turn": min(active_turns) if active_turns else None,
                "max_turn": max(int(t["turn_count"] or 0) for t in turns) if turns else None,
            }
        )

        if rss_mb is not None and rss_mb > rss_fail_mb:
            failed_sessions = failed[:10]
            return {
                "status": "rss_fail",
                "failed_sessions": failed_sessions,
                "samples": samples[-10:],
            }

        if failed and len(failed) >= max(2, len(session_ids) // 5):
            failed_sessions = failed[:10]
            return {
                "status": "session_failures",
                "failed_sessions": failed_sessions,
                "samples": samples[-10:],
            }

        if active_turns and min(active_turns) >= target_min_turn:
            failed_sessions = failed[:10]
            return {
                "status": "reached_turns",
                "failed_sessions": failed_sessions,
                "samples": samples[-10:],
            }

        time.sleep(check_every_secs)

    return {
        "status": "timeout",
        "failed_sessions": failed_sessions,
        "samples": samples[-10:],
    }


def parse_cases(case_args: list[str]) -> list[tuple[int, int]]:
    cases: list[tuple[int, int]] = []
    for raw in case_args:
        if ":" in raw:
            concurrency_s, probe_count_s = raw.split(":", 1)
            cases.append((int(concurrency_s), int(probe_count_s)))
        else:
            concurrency = int(raw)
            cases.append((concurrency, max(concurrency + 2, concurrency)))
    return cases


def main() -> None:
    parser = argparse.ArgumentParser(
        description="Benchmark live Foresight Probe workloads at different Monty concurrency limits."
    )
    parser.add_argument(
        "--base-url", default=os.environ.get("TEMPER_BASE_URL", DEFAULT_BASE_URL)
    )
    parser.add_argument(
        "--tenant", default=os.environ.get("TEMPER_TENANT", DEFAULT_TENANT)
    )
    parser.add_argument(
        "--cases",
        nargs="+",
        required=True,
        help="Concurrency cases. Use N or N:probe_count, e.g. 10 20:24",
    )
    parser.add_argument(
        "--rss-fail-mb", type=int, default=DEFAULT_RSS_FAIL_MB
    )
    parser.add_argument(
        "--timeout-secs", type=int, default=DEFAULT_TIMEOUT_SECS
    )
    parser.add_argument(
        "--check-every-secs", type=int, default=DEFAULT_CHECK_EVERY_SECS
    )
    parser.add_argument(
        "--target-min-turn", type=int, default=DEFAULT_TARGET_MIN_TURN
    )
    parser.add_argument(
        "--metrics-interval-secs",
        type=int,
        default=DEFAULT_METRICS_INTERVAL_SECS,
    )
    args = parser.parse_args()

    root = Path(__file__).resolve().parents[1]
    base_env = os.environ.copy()
    client = ODataClient(args.base_url, args.tenant, base_env.get("TEMPER_API_KEY"))
    results = []

    for concurrency, probe_count in parse_cases(args.cases):
        kill_port(3467)
        Path.home().joinpath(".local/share/openpaw/paw.db").unlink(missing_ok=True)

        version = f"local-investigation-c{concurrency}"
        proc, log_path = start_daemon(
            root,
            base_env=base_env,
            version=version,
            max_concurrency=concurrency,
            metrics_interval_secs=args.metrics_interval_secs,
        )
        if not wait_for_daemon(client):
            results.append(
                {
                    "case": concurrency,
                    "probe_count": probe_count,
                    "status": "daemon_not_ready",
                    "pid": proc.pid,
                    "version": version,
                    "log": str(log_path),
                }
            )
            continue

        model_id = seed_product_model(client)
        try:
            wait_for_state(client, "ProductModels", model_id, ["Active"], timeout=120)
        except Exception:
            pass

        session_ids = create_projection_with_probes(
            client, probe_count=probe_count, base_url=args.base_url
        )
        summary = monitor_case(
            client,
            pid=proc.pid,
            session_ids=session_ids,
            rss_fail_mb=args.rss_fail_mb,
            timeout_secs=args.timeout_secs,
            check_every_secs=args.check_every_secs,
            target_min_turn=args.target_min_turn,
        )
        results.append(
            {
                "case": concurrency,
                "probe_count": probe_count,
                "status": summary["status"],
                "pid": proc.pid,
                "version": version,
                "log": str(log_path),
                "session_count": len(session_ids),
                "failed_sessions": summary["failed_sessions"],
                "samples": summary["samples"],
            }
        )

    print(json.dumps(results, indent=2))


if __name__ == "__main__":
    main()
