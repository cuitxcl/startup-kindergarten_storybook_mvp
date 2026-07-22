#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

wait_for_port_free() {
  local port="$1"
  for _ in $(seq 1 40); do
    if ! command -v lsof >/dev/null 2>&1 || ! lsof -nP -iTCP:"$port" -sTCP:LISTEN >/dev/null 2>&1; then
      return 0
    fi
    sleep 0.25
  done
  echo "port did not become free after smoke step: $port" >&2
  lsof -nP -iTCP:"$port" -sTCP:LISTEN >&2 || true
  return 1
}

echo "== Kindleaf provider smoke suite =="
echo "Provider smoke scripts share Loco test port 8081, so this suite runs them sequentially."
echo "This suite uses fake DeepSeek/Seedream providers only; real provider smoke scripts must be run explicitly with real API keys."

echo
echo "1. text provider"
"$ROOT_DIR/scripts/smoke-generation-provider.sh"
wait_for_port_free 8081
wait_for_port_free 18182

echo
echo "2. text provider failure and retry"
"$ROOT_DIR/scripts/smoke-generation-provider-failure.sh"
wait_for_port_free 8081
wait_for_port_free 18182

echo
echo "3. image provider"
"$ROOT_DIR/scripts/smoke-image-provider.sh"
wait_for_port_free 8081
wait_for_port_free 18183

echo
echo "4. composite provider"
"$ROOT_DIR/scripts/smoke-composite-provider.sh"
wait_for_port_free 8081
wait_for_port_free 18182
wait_for_port_free 18183

echo
echo "== provider smoke suite ok =="
