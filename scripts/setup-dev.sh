#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

command -v cargo >/dev/null || { echo "cargo is required"; exit 1; }
command -v rustup >/dev/null || { echo "rustup is required"; exit 1; }
command -v npm >/dev/null || { echo "npm is required"; exit 1; }

echo "==> Ensuring Rust WASM targets"
rustup target add wasm32-unknown-unknown wasm32-wasip1 >/dev/null

echo "==> Installing dashboard dependencies"
(cd "$ROOT_DIR/dashboard" && npm install)

if [[ ! -f "$ROOT_DIR/.env" ]]; then
  echo "==> Seeding .env from .env.example"
  cp "$ROOT_DIR/.env.example" "$ROOT_DIR/.env"
else
  echo "==> Keeping existing .env"
fi

echo "==> Fetching Rust dependencies"
(cd "$ROOT_DIR" && cargo fetch)

echo "Development setup complete."
