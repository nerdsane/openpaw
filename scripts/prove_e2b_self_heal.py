#!/usr/bin/env python3
"""Run the existing self-heal proof while forcing E2B provisioning."""

from __future__ import annotations

import argparse
import os
import subprocess
import sys

from openpaw_proof_support import (
    DEFAULT_BASE_URL,
    DEFAULT_MODEL,
    DEFAULT_REPO_URL,
    DEFAULT_TENANT,
)


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--base-url", default=os.getenv("OPENPAW_BASE_URL", DEFAULT_BASE_URL))
    parser.add_argument("--tenant", default=os.getenv("PAW_TENANT", DEFAULT_TENANT))
    parser.add_argument("--repo-url", default=DEFAULT_REPO_URL)
    parser.add_argument("--model", default=DEFAULT_MODEL)
    parser.add_argument("--timeout-ms", type=int, default=20 * 60 * 1000)
    args = parser.parse_args()

    cmd = [
        sys.executable,
        os.path.join(os.path.dirname(__file__), "prove_self_heal_loop.py"),
        "--base-url",
        args.base_url,
        "--tenant",
        args.tenant,
        "--repo-url",
        args.repo_url,
        "--model",
        args.model,
        "--timeout-ms",
        str(args.timeout_ms),
        "--sandbox-mode",
        "e2b",
    ]
    return subprocess.run(cmd, check=False).returncode


if __name__ == "__main__":
    raise SystemExit(main())
