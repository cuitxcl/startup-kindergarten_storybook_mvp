#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
DB_CONTAINER="${DB_CONTAINER:-kindleaf-postgres}"
DB_USER="${DB_USER:-postgres}"
DB_PASSWORD="${DB_PASSWORD:-postgres}"
DB_HOST="${DB_HOST:-127.0.0.1}"
DB_PORT="${DB_PORT:-55432}"
DB_NAME="${DB_NAME:-kindleaf_generation_budget_smoke_$(date +%s)}"
API_PORT="${API_PORT:-8081}"
API_BASE_URL="${API_BASE_URL:-http://127.0.0.1:$API_PORT}"
DATABASE_URL="${DATABASE_URL:-postgres://$DB_USER:$DB_PASSWORD@$DB_HOST:$DB_PORT/$DB_NAME}"
LOG_DIR="${LOG_DIR:-$ROOT_DIR/.tmp/smoke-generation-budget}"
AUTH_HEADER="Authorization: Bearer ${API_TOKEN:-dev-token}"
SCHOOL_WS="20000000-0000-0000-0000-000000000001"

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
      return 0
    fi
    sleep 0.25
  done
  echo "warning: temporary database was not dropped: $DB_NAME" >&2
}

require_port_free() {
  local port="$1"
  if command -v lsof >/dev/null 2>&1 && lsof -nP -iTCP:"$port" -sTCP:LISTEN >/dev/null 2>&1; then
    echo "port is already in use: $port" >&2
    lsof -nP -iTCP:"$port" -sTCP:LISTEN >&2 || true
    return 1
  fi
}

wait_for_url() {
  local url="$1"
  local label="$2"
  for _ in $(seq 1 100); do
    if curl -fsS "$url" >/dev/null 2>&1; then
      return 0
    fi
    sleep 0.25
  done
  echo "$label did not become ready: $url" >&2
  tail -120 "$LOG_DIR/server.log" 2>/dev/null || true
  return 1
}

json_get() {
  local script="$1"
  node -e "let s='';process.stdin.on('data',d=>s+=d);process.stdin.on('end',()=>{const p=JSON.parse(s); ${script}});"
}

api() {
  local method="$1"
  local path="$2"
  local body="${3:-}"
  if [[ -n "$body" ]]; then
    curl -fsS -H "$AUTH_HEADER" -H "Content-Type: application/json" -X "$method" "$API_BASE_URL$path" -d "$body"
  else
    curl -fsS -H "$AUTH_HEADER" -X "$method" "$API_BASE_URL$path"
  fi
}

mkdir -p "$LOG_DIR"
trap 'status=$?; cleanup; exit $status' EXIT

echo "== Kindleaf generation budget smoke =="
echo "API_BASE_URL=$API_BASE_URL"
echo "DB_NAME=$DB_NAME"
echo "logs=$LOG_DIR"

require_port_free "$API_PORT"

docker exec "$DB_CONTAINER" dropdb -U "$DB_USER" "$DB_NAME" >/dev/null 2>&1 || true
docker exec "$DB_CONTAINER" createdb -U "$DB_USER" "$DB_NAME"

echo "1. migrate"
(
  cd "$ROOT_DIR/server"
  DATABASE_URL="$DATABASE_URL" cargo run --features db -- -e test db migrate
) >"$LOG_DIR/migrate.log" 2>&1

echo "2. start backend with generation budget"
(
  cd "$ROOT_DIR/server"
  KINDLEAF_DEMO_SEED=1 \
  KINDLEAF_COST_BUDGET_LIMIT_MICROS=2 \
  KINDLEAF_COST_BUDGET_WARNING_PERCENT=50 \
  DATABASE_URL="$DATABASE_URL" \
  PORT="$API_PORT" \
  APP_HOST="$API_BASE_URL" \
  cargo run --features db -- -e test start
) >"$LOG_DIR/server.log" 2>&1 &
server_pid="$!"
wait_for_url "$API_BASE_URL/api/health" "backend"

echo "3. seed exceeded cost"
docker exec -i "$DB_CONTAINER" psql -U "$DB_USER" -d "$DB_NAME" -v ON_ERROR_STOP=1 >/dev/null <<SQL
insert into generation_cost_logs (
  id, workspace_id, generation_job_id, provider, job_type, status,
  estimated_input_units, estimated_output_units, image_count, estimated_cost_micros,
  currency, metadata_json, created_at
) values (
  gen_random_uuid(), '$SCHOOL_WS', gen_random_uuid(), 'deepseek', 'storybook_plan', 'succeeded',
  1, 1, 0, 1, 'USD', '{"smoke":"budget"}'::jsonb, now()
);
SQL
direct_cost_count=$(docker exec "$DB_CONTAINER" psql -U "$DB_USER" -d "$DB_NAME" -tAc "select count(*) from generation_cost_logs where workspace_id = '$SCHOOL_WS' and estimated_cost_micros = 1")
if [[ "$direct_cost_count" != "1" ]]; then
  echo "expected one seeded generation cost row, got $direct_cost_count" >&2
  docker exec "$DB_CONTAINER" psql -U "$DB_USER" -d "$DB_NAME" -c "select current_database(), count(*) from generation_cost_logs;" >&2 || true
  docker exec "$DB_CONTAINER" psql -U "$DB_USER" -d "$DB_NAME" -c "select workspace_id, estimated_cost_micros, status from generation_cost_logs order by created_at desc limit 5;" >&2 || true
  exit 1
fi

login_json=$(curl -fsS -H "Content-Type: application/json" -X POST "$API_BASE_URL/api/auth/login" -d '{"identifier":"lin@example.com","password":"demo"}')
API_TOKEN=$(echo "$login_json" | json_get "if(!p.data.token) process.exit(1); console.log(p.data.token)")
AUTH_HEADER="Authorization: Bearer $API_TOKEN"

echo "4. verify budget summary"
budget_json=$(api GET "/api/operator/generation-costs?workspace_id=$SCHOOL_WS&limit=5&offset=0")
echo "$budget_json" | json_get "
if(p.data.summary.budget_limit_micros !== 2) {
  console.error(JSON.stringify(p.data.summary));
  process.exit(1);
}
if(p.data.summary.budget_used_percent !== 50) {
  console.error(JSON.stringify(p.data.summary));
  process.exit(1);
}
if(p.data.summary.budget_warning_percent !== 50 || p.data.summary.budget_warning !== true) {
  console.error(JSON.stringify(p.data.summary));
  process.exit(1);
}
if(p.data.summary.budget_exceeded !== false) {
  console.error(JSON.stringify(p.data.summary));
  process.exit(1);
}
console.log('budget_warning=' + p.data.summary.budget_warning);
"

echo "5. seed exceeded cost and verify generation creation is blocked"
docker exec -i "$DB_CONTAINER" psql -U "$DB_USER" -d "$DB_NAME" -v ON_ERROR_STOP=1 >/dev/null <<SQL
insert into generation_cost_logs (
  id, workspace_id, generation_job_id, provider, job_type, status,
  estimated_input_units, estimated_output_units, image_count, estimated_cost_micros,
  currency, metadata_json, created_at
) values (
  gen_random_uuid(), '$SCHOOL_WS', gen_random_uuid(), 'deepseek', 'storybook_plan', 'succeeded',
  1, 1, 0, 1, 'USD', '{"smoke":"budget-exceeded"}'::jsonb, now()
);
SQL
budget_exceeded_json=$(api GET "/api/operator/generation-costs?workspace_id=$SCHOOL_WS&limit=5&offset=0")
echo "$budget_exceeded_json" | json_get "
if(p.data.summary.budget_used_percent !== 100 || p.data.summary.budget_warning !== true || p.data.summary.budget_exceeded !== true) {
  console.error(JSON.stringify(p.data.summary));
  process.exit(1);
}
console.log('budget_exceeded=' + p.data.summary.budget_exceeded);
"
api GET "/api/operator/readiness" | json_get "
const check = p.data.checks.find((item)=>item.key === 'generation_budget');
if(!check || check.ok !== false) {
  console.error(JSON.stringify(p.data.checks, null, 2));
  process.exit(1);
}
if(!String(check.message || '').includes('已达到上限')) {
  console.error(JSON.stringify(check));
  process.exit(1);
}
console.log('budget_readiness_blocked=' + check.ok);
"
response_file="$LOG_DIR/budget-response.json"
status=$(curl -sS -o "$response_file" -w "%{http_code}" \
  -H "$AUTH_HEADER" \
  -H "Content-Type: application/json" \
  -X POST "$API_BASE_URL/api/workspaces/$SCHOOL_WS/generation-jobs" \
  -d '{"job_type":"storybook_plan","input_json":{"theme":"预算超限验证"}}')
if [[ "$status" != "409" ]]; then
  echo "expected budget block 409, got $status" >&2
  cat "$response_file" >&2
  exit 1
fi
node -e "
const fs = require('fs');
const payload = JSON.parse(fs.readFileSync('$response_file', 'utf8'));
if (payload.error?.code !== 'state_conflict') process.exit(1);
if (!String(payload.error?.message || '').includes('generation_budget_exceeded')) process.exit(1);
console.log('budget_generation_blocked=ok');
"

echo "== generation budget smoke ok =="
