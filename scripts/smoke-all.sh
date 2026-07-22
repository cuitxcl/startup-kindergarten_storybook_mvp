#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
API_PORT="${API_PORT:-8111}"
FRONTEND_PORT="${FRONTEND_PORT:-5178}"
API_BASE_URL="${API_BASE_URL:-http://127.0.0.1:$API_PORT}"
FRONTEND_BASE_URL="${FRONTEND_BASE_URL:-http://127.0.0.1:$FRONTEND_PORT}"
DATABASE_URL="${DATABASE_URL:-postgres://postgres:postgres@127.0.0.1:55432/kindleaf_development}"
DB_NAME="${DB_NAME:-${DATABASE_URL##*/}}"
DB_NAME="${DB_NAME%%\?*}"
DB_CONTAINER="${DB_CONTAINER:-kindleaf-postgres}"
APP_HOST="${APP_HOST:-$API_BASE_URL}"
LOG_DIR="${LOG_DIR:-$ROOT_DIR/.tmp/smoke}"

server_pid=""
frontend_pid=""

cleanup() {
  if [[ -n "$frontend_pid" ]] && kill -0 "$frontend_pid" 2>/dev/null; then
    kill "$frontend_pid" 2>/dev/null || true
    wait "$frontend_pid" 2>/dev/null || true
  fi
  if [[ -n "$server_pid" ]] && kill -0 "$server_pid" 2>/dev/null; then
    kill "$server_pid" 2>/dev/null || true
    wait "$server_pid" 2>/dev/null || true
  fi
}

require_port_free() {
  local port="$1"
  local name="$2"
  if command -v lsof >/dev/null 2>&1 && lsof -nP -iTCP:"$port" -sTCP:LISTEN >/dev/null 2>&1; then
    echo "$name port is already in use: $port" >&2
    lsof -nP -iTCP:"$port" -sTCP:LISTEN >&2 || true
    return 1
  fi
}

wait_for_url() {
  local url="$1"
  local name="$2"
  local attempts="${3:-80}"
  for _ in $(seq 1 "$attempts"); do
    if curl -fsS "$url" >/dev/null 2>&1; then
      return 0
    fi
    sleep 0.25
  done
  echo "$name did not become ready: $url" >&2
  return 1
}

trap 'status=$?; cleanup; exit $status' EXIT

mkdir -p "$LOG_DIR"
require_port_free "$API_PORT" "backend"
require_port_free "$FRONTEND_PORT" "frontend"

echo "== Kindleaf full smoke =="
echo "API_BASE_URL=$API_BASE_URL"
echo "FRONTEND_BASE_URL=$FRONTEND_BASE_URL"
echo "DB_NAME=$DB_NAME"
echo "logs=$LOG_DIR"

echo "1. migrate"
(
  cd "$ROOT_DIR/server"
  DATABASE_URL="$DATABASE_URL" cargo run --features db -- -e test db migrate
) >"$LOG_DIR/migrate.log" 2>&1

echo "2. start backend"
(
  cd "$ROOT_DIR/server"
  KINDLEAF_DEMO_SEED=1 PORT="$API_PORT" APP_HOST="$APP_HOST" DATABASE_URL="$DATABASE_URL" \
    cargo run --features db -- -e production start
) >"$LOG_DIR/server.log" 2>&1 &
server_pid="$!"
wait_for_url "$API_BASE_URL/api/health" "backend"

echo "3. start frontend"
(
  cd "$ROOT_DIR/frontend"
  VITE_USE_API=true VITE_API_BASE_URL="$API_BASE_URL" \
    npm run dev -- --host 127.0.0.1 --port "$FRONTEND_PORT"
) >"$LOG_DIR/frontend.log" 2>&1 &
frontend_pid="$!"
wait_for_url "$FRONTEND_BASE_URL/" "frontend"

echo "4. API smoke"
API_BASE_URL="$API_BASE_URL" DB_CONTAINER="$DB_CONTAINER" DB_NAME="$DB_NAME" "$ROOT_DIR/scripts/smoke-api.sh"

echo "5. UI smoke"
(
  cd "$ROOT_DIR/frontend"
  API_BASE_URL="$API_BASE_URL" FRONTEND_BASE_URL="$FRONTEND_BASE_URL" npm run smoke:ui
)

echo "== full smoke ok =="
