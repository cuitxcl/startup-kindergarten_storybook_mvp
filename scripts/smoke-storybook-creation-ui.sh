#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
DB_CONTAINER="${DB_CONTAINER:-kindleaf-postgres}"
DB_USER="${DB_USER:-postgres}"
DB_PASSWORD="${DB_PASSWORD:-postgres}"
DB_HOST="${DB_HOST:-127.0.0.1}"
DB_PORT="${DB_PORT:-55432}"
DB_NAME="${DB_NAME:-kindleaf_storybook_creation_ui_smoke_$(date +%s)}"
API_PORT="${API_PORT:-8082}"
FRONTEND_PORT="${FRONTEND_PORT:-5179}"
API_BASE_URL="${API_BASE_URL:-http://127.0.0.1:$API_PORT}"
FRONTEND_BASE_URL="${FRONTEND_BASE_URL:-http://127.0.0.1:$FRONTEND_PORT}"
DATABASE_URL="${DATABASE_URL:-postgres://$DB_USER:$DB_PASSWORD@$DB_HOST:$DB_PORT/$DB_NAME}"
LOG_DIR="${LOG_DIR:-$ROOT_DIR/.tmp/smoke-storybook-creation-ui}"
KEEP_DB="${KEEP_DB:-false}"

server_pid=""
frontend_pid=""

kill_listening_port() {
  local port="$1"
  if command -v lsof >/dev/null 2>&1; then
    local pids
    pids=$(lsof -tiTCP:"$port" -sTCP:LISTEN 2>/dev/null || true)
    if [[ -n "$pids" ]]; then
      kill $pids 2>/dev/null || true
      sleep 0.2
      kill -9 $pids 2>/dev/null || true
    fi
  fi
}

cleanup() {
  if [[ -n "$frontend_pid" ]] && kill -0 "$frontend_pid" 2>/dev/null; then
    kill "$frontend_pid" 2>/dev/null || true
    wait "$frontend_pid" 2>/dev/null || true
  fi
  if [[ -n "$server_pid" ]] && kill -0 "$server_pid" 2>/dev/null; then
    kill "$server_pid" 2>/dev/null || true
    wait "$server_pid" 2>/dev/null || true
  fi
  kill_listening_port "$FRONTEND_PORT"
  kill_listening_port "$API_PORT"
  if [[ "$KEEP_DB" != "true" ]]; then
    for _ in $(seq 1 20); do
      docker exec "$DB_CONTAINER" psql -U "$DB_USER" -d postgres -v ON_ERROR_STOP=1 >/dev/null 2>&1 <<SQL || true
select pg_terminate_backend(pid)
from pg_stat_activity
where datname = '$DB_NAME'
  and pid <> pg_backend_pid();
SQL
      docker exec "$DB_CONTAINER" dropdb --force -U "$DB_USER" "$DB_NAME" >/dev/null 2>&1 || true
      if ! docker exec "$DB_CONTAINER" psql -U "$DB_USER" -d postgres -tAc "select 1 from pg_database where datname = '$DB_NAME'" | grep -q 1; then
        return 0
      fi
      sleep 0.25
    done
    echo "warning: temporary database was not dropped: $DB_NAME" >&2
  fi
}

require_port_free() {
  local port="$1"
  local label="$2"
  if command -v lsof >/dev/null 2>&1 && lsof -nP -iTCP:"$port" -sTCP:LISTEN >/dev/null 2>&1; then
    echo "$label port is already in use: $port" >&2
    lsof -nP -iTCP:"$port" -sTCP:LISTEN >&2 || true
    return 1
  fi
}

wait_for_url() {
  local url="$1"
  local label="$2"
  local attempts="${3:-120}"
  for _ in $(seq 1 "$attempts"); do
    if curl -fsS "$url" >/dev/null 2>&1; then
      return 0
    fi
    sleep 0.25
  done
  echo "$label did not become ready: $url" >&2
  tail -120 "$LOG_DIR/$label.log" >&2 || true
  return 1
}

trap 'status=$?; cleanup; exit $status' EXIT

mkdir -p "$LOG_DIR"
require_port_free "$API_PORT" "backend"
require_port_free "$FRONTEND_PORT" "frontend"

echo "== Kindleaf personalized storybook creation UI smoke =="
echo "API_BASE_URL=$API_BASE_URL"
echo "FRONTEND_BASE_URL=$FRONTEND_BASE_URL"
echo "DB_NAME=$DB_NAME"
echo "logs=$LOG_DIR"

docker exec "$DB_CONTAINER" dropdb -U "$DB_USER" "$DB_NAME" >/dev/null 2>&1 || true
docker exec "$DB_CONTAINER" createdb -U "$DB_USER" "$DB_NAME"

echo "1. migrate"
(
  cd "$ROOT_DIR/server"
  DATABASE_URL="$DATABASE_URL" cargo run --features db -- -e test db migrate
) >"$LOG_DIR/migrate.log" 2>&1

echo "2. start backend"
(
  cd "$ROOT_DIR/server"
  KINDLEAF_DEMO_SEED=1 \
  KINDLEAF_GENERATION_PROVIDER=mock \
  PORT="$API_PORT" \
  APP_HOST="$API_BASE_URL" \
  DATABASE_URL="$DATABASE_URL" \
  cargo run --features db -- -e production start
) >"$LOG_DIR/backend.log" 2>&1 &
server_pid="$!"
wait_for_url "$API_BASE_URL/api/health" "backend"

echo "3. start frontend"
(
  cd "$ROOT_DIR/frontend"
  VITE_API_BASE_URL="$API_BASE_URL" npm run dev -- --host 127.0.0.1 --port "$FRONTEND_PORT" --strictPort
) >"$LOG_DIR/frontend.log" 2>&1 &
frontend_pid="$!"
wait_for_url "$FRONTEND_BASE_URL/" "frontend"

echo "4. run personalized UI smoke"
FRONTEND_BASE_URL="$FRONTEND_BASE_URL" API_BASE_URL="$API_BASE_URL" \
  DB_CONTAINER="$DB_CONTAINER" DB_USER="$DB_USER" DB_NAME="$DB_NAME" \
  node "$ROOT_DIR/scripts/smoke-storybook-creation-ui.mjs"

echo "== personalized storybook creation UI smoke ok =="
