#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
DB_CONTAINER="${DB_CONTAINER:-kindleaf-postgres}"
DB_USER="${DB_USER:-postgres}"
DB_PASSWORD="${DB_PASSWORD:-postgres}"
DB_HOST="${DB_HOST:-127.0.0.1}"
DB_PORT="${DB_PORT:-55432}"
DB_NAME="${DB_NAME:-kindleaf_operator_readiness_$(date +%s)}"
API_PORT="${API_PORT:-8081}"
API_BASE_URL="${API_BASE_URL:-http://127.0.0.1:$API_PORT}"
DATABASE_URL="${DATABASE_URL:-postgres://$DB_USER:$DB_PASSWORD@$DB_HOST:$DB_PORT/$DB_NAME}"
LOG_DIR="${LOG_DIR:-$ROOT_DIR/.tmp/smoke-operator-readiness}"
STORAGE_ROOT="${KINDLEAF_STORAGE_ROOT:-$ROOT_DIR/.tmp/operator-readiness-storage}"

server_pid=""

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
  if [[ -n "$server_pid" ]] && kill -0 "$server_pid" 2>/dev/null; then
    kill "$server_pid" 2>/dev/null || true
    wait "$server_pid" 2>/dev/null || true
  fi
  kill_listening_port "$API_PORT"
  for _ in $(seq 1 20); do
    docker exec "$DB_CONTAINER" psql -U "$DB_USER" -d postgres -v ON_ERROR_STOP=1 >/dev/null 2>&1 <<SQL || true
select pg_terminate_backend(pid)
from pg_stat_activity
where datname = '$DB_NAME'
  and pid <> pg_backend_pid();
SQL
    docker exec "$DB_CONTAINER" dropdb --force -U "$DB_USER" "$DB_NAME" >/dev/null 2>&1 || true
    if ! docker exec "$DB_CONTAINER" psql -U "$DB_USER" -d postgres -tAc "select 1 from pg_database where datname = '$DB_NAME'" | grep -q 1; then
      break
    fi
    sleep 0.25
  done
}

require_port_free() {
  local port="$1"
  if command -v lsof >/dev/null 2>&1 && lsof -nP -iTCP:"$port" -sTCP:LISTEN >/dev/null 2>&1; then
    echo "backend port is already in use: $port" >&2
    lsof -nP -iTCP:"$port" -sTCP:LISTEN >&2 || true
    return 1
  fi
}

wait_for_api() {
  for _ in $(seq 1 100); do
    if curl -fsS "$API_BASE_URL/api/health" >/dev/null 2>&1; then
      return 0
    fi
    sleep 0.25
  done
  echo "backend did not become ready: $API_BASE_URL/api/health" >&2
  tail -120 "$LOG_DIR/server.log" >&2 || true
  return 1
}

json_get() {
  local script="$1"
  node -e "let s='';process.stdin.on('data',d=>s+=d);process.stdin.on('end',()=>{const p=JSON.parse(s); ${script}});"
}

mkdir -p "$LOG_DIR" "$STORAGE_ROOT"
trap 'status=$?; cleanup; exit $status' EXIT

echo "== Kindleaf operator readiness smoke =="
echo "API_BASE_URL=$API_BASE_URL"
echo "DB_CONTAINER=$DB_CONTAINER"
echo "DB_NAME=$DB_NAME"
echo "DATABASE_URL=$DATABASE_URL"
echo "storage=$STORAGE_ROOT"
echo "logs=$LOG_DIR"

require_port_free "$API_PORT"
docker exec "$DB_CONTAINER" dropdb -U "$DB_USER" "$DB_NAME" >/dev/null 2>&1 || true
docker exec "$DB_CONTAINER" createdb -U "$DB_USER" "$DB_NAME"

echo "1. migrate and seed operator workspace"
(
  cd "$ROOT_DIR/server"
  DATABASE_URL="$DATABASE_URL" cargo run --features db -- -e test db migrate
  DATABASE_URL="$DATABASE_URL" cargo run --features db -- -e test db seed
) >"$LOG_DIR/setup.log" 2>&1

echo "2. start backend with trial-ready configuration"
(
  cd "$ROOT_DIR/server"
  PORT="$API_PORT" \
  APP_HOST="https://trial.kindleaf.example" \
  DATABASE_URL="$DATABASE_URL" \
  KINDLEAF_DEMO_SEED=0 \
  KINDLEAF_GENERATION_PROVIDER= \
  KINDLEAF_AUTH_TOKEN_SECRET="operator-readiness-auth-secret-000000" \
  KINDLEAF_AUTH_TOKEN_TTL_SECONDS=604800 \
  DEEPSEEK_API_KEY="operator-readiness-deepseek-key" \
  SEEDREAM_API_KEY="operator-readiness-seedream-key" \
  KINDLEAF_STORAGE_ROOT="$STORAGE_ROOT" \
  KINDLEAF_COST_BUDGET_LIMIT_MICROS=2000000 \
  cargo run --features db -- -e test start
) >"$LOG_DIR/server.log" 2>&1 &
server_pid="$!"
wait_for_api

echo "3. verify operator readiness"
operator_token=$(curl -fsS -H "Content-Type: application/json" -X POST "$API_BASE_URL/api/auth/login" -d '{"identifier":"lin@example.com","password":"demo"}' | json_get "console.log(p.data.token)")
if [[ "$operator_token" != kindleaf-v1:* ]]; then
  echo "expected signed auth token when KINDLEAF_AUTH_TOKEN_SECRET is configured" >&2
  exit 1
fi
curl -fsS -H "Authorization: Bearer $operator_token" "$API_BASE_URL/api/operator/readiness" | json_get "
if (p.data.ready !== true) {
  console.error(JSON.stringify(p.data, null, 2));
  process.exit(1);
}
if (p.data.mode !== 'trial_ready') process.exit(1);
const keys = p.data.checks.map((item)=>item.key);
for (const key of ['database','database_schema','app_host','auth_token','auth_token_ttl','generation_provider_secrets','generation_provider_config','generation_provider','storage','generation_budget','demo_seed']) {
  if (!keys.includes(key)) process.exit(1);
}
if (!p.data.checks.every((item)=>item.ok === true)) {
  console.error(JSON.stringify(p.data.checks, null, 2));
  process.exit(1);
}
if (p.data.provider.provider !== 'deepseek+seedream' || p.data.provider.production_ready !== true) process.exit(1);
if (!p.data.storage.exports_dir.includes('.tmp/operator-readiness-storage')) process.exit(1);
console.log('operator_readiness=' + p.data.mode + '/' + keys.join(','));
"

echo "== operator readiness smoke ok =="
