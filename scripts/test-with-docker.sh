#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"

docker compose -f docker/docker-compose.yml up -d

# redis-cluster bootstrap can take a bit longer
sleep 40

cd src-tauri
cargo test -- --ignored

cd "$ROOT_DIR"
docker compose -f docker/docker-compose.yml down -v
