#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
DB_CONTAINER="${DB_CONTAINER:-kindleaf-postgres}"
DB_USER="${DB_USER:-postgres}"
DB_PASSWORD="${DB_PASSWORD:-postgres}"
DB_HOST="${DB_HOST:-127.0.0.1}"
DB_PORT="${DB_PORT:-55432}"
DB_NAME="${DB_NAME:-kindleaf_generation_provider_failure_smoke_$(date +%s)}"
API_PORT="${API_PORT:-8081}"
DEEPSEEK_PORT="${DEEPSEEK_PORT:-18182}"
API_BASE_URL="${API_BASE_URL:-http://127.0.0.1:$API_PORT}"
DEEPSEEK_BASE_URL="${DEEPSEEK_BASE_URL:-http://127.0.0.1:$DEEPSEEK_PORT}"
DATABASE_URL="${DATABASE_URL:-postgres://$DB_USER:$DB_PASSWORD@$DB_HOST:$DB_PORT/$DB_NAME}"
LOG_DIR="${LOG_DIR:-$ROOT_DIR/.tmp/smoke-generation-provider-failure}"
AUTH_HEADER="Authorization: Bearer ${API_TOKEN:-dev-token}"

server_pid=""
provider_pid=""

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
  if [[ -n "$provider_pid" ]] && kill -0 "$provider_pid" 2>/dev/null; then
    kill "$provider_pid" 2>/dev/null || true
    wait "$provider_pid" 2>/dev/null || true
  fi
  kill_listening_port "$API_PORT"
  kill_listening_port "$DEEPSEEK_PORT"
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

wait_for_port_free() {
  local port="$1"
  for _ in $(seq 1 40); do
    if ! command -v lsof >/dev/null 2>&1 || ! lsof -nP -iTCP:"$port" -sTCP:LISTEN >/dev/null 2>&1; then
      return 0
    fi
    sleep 0.25
  done
  echo "port did not become free: $port" >&2
  return 1
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
  tail -120 "$LOG_DIR/server.log" "$LOG_DIR/fake-deepseek.log" 2>/dev/null || true
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

wait_for_job_status() {
  local workspace_id="$1"
  local job_id="$2"
  local expected_status="$3"
  for _ in $(seq 1 80); do
    local payload
    payload=$(api GET "/api/workspaces/$workspace_id/generation-jobs/$job_id")
    local status
    status=$(echo "$payload" | json_get "console.log(p.data.status)")
    if [[ "$status" == "$expected_status" ]]; then
      echo "$payload"
      return 0
    fi
    sleep 0.25
  done
  echo "generation job did not become $expected_status: $job_id" >&2
  api GET "/api/workspaces/$workspace_id/generation-jobs/$job_id" >&2 || true
  return 1
}

start_fake_deepseek() {
  local mode="$1"
  local log_file="$2"
  FAKE_DEEPSEEK_MODE="$mode" node "$ROOT_DIR/scripts/fake-deepseek.mjs" "$DEEPSEEK_PORT" >"$log_file" 2>&1 &
  provider_pid="$!"
  wait_for_url "$DEEPSEEK_BASE_URL/health" "fake DeepSeek"
}

stop_fake_deepseek() {
  if [[ -n "$provider_pid" ]] && kill -0 "$provider_pid" 2>/dev/null; then
    kill "$provider_pid" 2>/dev/null || true
    wait "$provider_pid" 2>/dev/null || true
  fi
  provider_pid=""
  wait_for_port_free "$DEEPSEEK_PORT"
}

mkdir -p "$LOG_DIR"
trap 'status=$?; cleanup; exit $status' EXIT

echo "== Kindleaf generation provider failure smoke =="
echo "API_BASE_URL=$API_BASE_URL"
echo "DEEPSEEK_BASE_URL=$DEEPSEEK_BASE_URL"
echo "DB_NAME=$DB_NAME"
echo "logs=$LOG_DIR"

require_port_free "$API_PORT"
require_port_free "$DEEPSEEK_PORT"

docker exec "$DB_CONTAINER" dropdb -U "$DB_USER" "$DB_NAME" >/dev/null 2>&1 || true
docker exec "$DB_CONTAINER" createdb -U "$DB_USER" "$DB_NAME"

echo "1. migrate"
(
  cd "$ROOT_DIR/server"
  DATABASE_URL="$DATABASE_URL" cargo run --features db -- -e test db migrate
) >"$LOG_DIR/migrate.log" 2>&1

echo "2. start fake DeepSeek in failure mode"
start_fake_deepseek "http_500" "$LOG_DIR/fake-deepseek.log"

echo "3. start backend with deepseek provider"
(
  cd "$ROOT_DIR/server"
  KINDLEAF_DEMO_SEED=1 \
  KINDLEAF_GENERATION_PROVIDER=deepseek \
  DEEPSEEK_API_KEY=test-key \
  DEEPSEEK_BASE_URL="$DEEPSEEK_BASE_URL" \
  PORT="$API_PORT" \
  APP_HOST="$API_BASE_URL" \
  DATABASE_URL="$DATABASE_URL" \
  cargo run --features db -- -e test start
) >"$LOG_DIR/server.log" 2>&1 &
server_pid="$!"
wait_for_url "$API_BASE_URL/api/health" "backend"

echo "4. login demo operator user"
login_json=$(curl -fsS -H "Content-Type: application/json" -X POST "$API_BASE_URL/api/auth/login" -d '{"identifier":"lin@example.com","password":"demo"}')
api_token=$(echo "$login_json" | json_get "if(!p.data.token) process.exit(1); console.log(p.data.token);")
AUTH_HEADER="Authorization: Bearer $api_token"

echo "5. create a text job and verify provider failure"
workspace_id=$(api GET "/api/workspaces" | json_get "const ws=p.data.find((item)=>item.type==='school' && item.role==='school_admin'); if(!ws) process.exit(1); console.log(ws.id);")
job_json=$(api POST "/api/workspaces/$workspace_id/generation-jobs" '{"job_type":"storybook_plan","input_json":{"theme":"provider 失败恢复","age_group":"4-5 岁","teaching_goal":"验证失败和重试"}}')
job_id=$(echo "$job_json" | json_get "if(p.data.job_type!=='storybook_plan' || p.data.status!=='queued') process.exit(1); console.log(p.data.id);")
failed_json=$(wait_for_job_status "$workspace_id" "$job_id" "failed")
echo "$failed_json" | json_get "
if(p.data.output_json?.schema_version!=='generation.error.v1') process.exit(1);
if(p.data.output_json?.provider!=='deepseek') process.exit(1);
if(p.data.output_json?.mode!=='storybook_plan') process.exit(1);
if(p.data.output_json?.error?.retryable !== true) process.exit(1);
if(!p.data.last_error || !p.data.last_error.includes('DeepSeek 请求返回')) process.exit(1);
if(!p.data.next_run_at) process.exit(1);
console.log('provider_failed_job=' + p.data.id);
"

api GET "/api/operator/generation-costs?workspace_id=$workspace_id&provider=deepseek&status=failed&limit=20&offset=0" | json_get "
const item = p.data.items.find((row)=>row.generation_job_id==='$job_id');
if(!item || item.status !== 'failed' || item.estimated_cost_micros !== 0) process.exit(1);
console.log('provider_failed_cost=' + item.id);
"

echo "6. restart fake DeepSeek in success mode and retry the same job"
stop_fake_deepseek
start_fake_deepseek "ok" "$LOG_DIR/fake-deepseek-recovered.log"
retried_json=$(api POST "/api/workspaces/$workspace_id/generation-jobs/$job_id/retry")
echo "$retried_json" | json_get "
if(p.data.id!=='$job_id') process.exit(1);
if(p.data.status!=='succeeded') process.exit(1);
if(p.data.output_json?.provider!=='deepseek') process.exit(1);
if(p.data.output_json?.mode!=='storybook_plan') process.exit(1);
if(!p.data.output_json?.plan?.title) process.exit(1);
if(p.data.attempt_count < 2) process.exit(1);
if(p.data.last_error !== null || p.data.next_run_at !== null || p.data.locked_by !== null || p.data.locked_at !== null) process.exit(1);
console.log('provider_retry_succeeded=' + p.data.id);
"

api GET "/api/operator/generation-costs?workspace_id=$workspace_id&provider=deepseek&status=succeeded&limit=20&offset=0" | json_get "
const item = p.data.items.find((row)=>row.generation_job_id==='$job_id');
if(!item || item.status !== 'succeeded' || item.estimated_output_units <= 0) process.exit(1);
console.log('provider_retry_cost=' + item.id);
"

echo "7. verify invalid provider content is failed without auto retry scheduling"
stop_fake_deepseek
start_fake_deepseek "invalid_content" "$LOG_DIR/fake-deepseek-invalid-content.log"
invalid_job_json=$(api POST "/api/workspaces/$workspace_id/generation-jobs" '{"job_type":"storybook_plan","input_json":{"theme":"provider 非法输出","age_group":"4-5 岁","teaching_goal":"验证 JSON 输出格式错误"}}')
invalid_job_id=$(echo "$invalid_job_json" | json_get "if(p.data.job_type!=='storybook_plan' || p.data.status!=='queued') process.exit(1); console.log(p.data.id);")
invalid_failed_json=$(wait_for_job_status "$workspace_id" "$invalid_job_id" "failed")
echo "$invalid_failed_json" | json_get "
if(p.data.output_json?.schema_version!=='generation.error.v1') process.exit(1);
if(p.data.output_json?.provider!=='deepseek') process.exit(1);
if(p.data.output_json?.mode!=='storybook_plan') process.exit(1);
if(p.data.output_json?.error?.retryable !== false) process.exit(1);
if(!p.data.last_error || !p.data.last_error.includes('DeepSeek content 不是合法 JSON')) process.exit(1);
if(p.data.next_run_at !== null) process.exit(1);
console.log('provider_invalid_content_failed=' + p.data.id);
"

api GET "/api/operator/generation-costs?workspace_id=$workspace_id&provider=deepseek&status=failed&limit=20&offset=0" | json_get "
const item = p.data.items.find((row)=>row.generation_job_id==='$invalid_job_id');
if(!item || item.status !== 'failed' || item.estimated_cost_micros !== 0) process.exit(1);
console.log('provider_invalid_content_cost=' + item.id);
"

echo "8. verify wrong provider schema is failed before writeback"
stop_fake_deepseek
start_fake_deepseek "wrong_shape" "$LOG_DIR/fake-deepseek-wrong-shape.log"
wrong_shape_job_json=$(api POST "/api/workspaces/$workspace_id/generation-jobs" '{"job_type":"storybook_plan","input_json":{"theme":"provider 结构错误","age_group":"4-5 岁","teaching_goal":"验证结构校验"}}')
wrong_shape_job_id=$(echo "$wrong_shape_job_json" | json_get "if(p.data.job_type!=='storybook_plan' || p.data.status!=='queued') process.exit(1); console.log(p.data.id);")
wrong_shape_failed_json=$(wait_for_job_status "$workspace_id" "$wrong_shape_job_id" "failed")
echo "$wrong_shape_failed_json" | json_get "
if(p.data.output_json?.schema_version!=='generation.error.v1') process.exit(1);
if(p.data.output_json?.provider!=='deepseek') process.exit(1);
if(p.data.output_json?.mode!=='storybook_plan') process.exit(1);
if(p.data.output_json?.error?.retryable !== false) process.exit(1);
if(!p.data.last_error || !p.data.last_error.includes('provider 输出 storybook_plan.plan 必须是 object')) process.exit(1);
if(p.data.next_run_at !== null) process.exit(1);
console.log('provider_wrong_shape_failed=' + p.data.id);
"

api GET "/api/operator/generation-costs?workspace_id=$workspace_id&provider=deepseek&status=failed&limit=20&offset=0" | json_get "
const item = p.data.items.find((row)=>row.generation_job_id==='$wrong_shape_job_id');
if(!item || item.status !== 'failed' || item.estimated_cost_micros !== 0) process.exit(1);
console.log('provider_wrong_shape_cost=' + item.id);
"

echo "9. verify sensitive provider output is failed before storybook writeback"
stop_fake_deepseek
start_fake_deepseek "sensitive_output" "$LOG_DIR/fake-deepseek-sensitive-output.log"
sensitive_storybook_json=$(api POST "/api/workspaces/$workspace_id/storybooks" '{"title":"Provider 敏感输出拦截绘本","age_group":"4-5 岁","use_scene":"安全验证","teaching_goal":"验证 provider 输出不会写入敏感信息"}')
sensitive_storybook_id=$(echo "$sensitive_storybook_json" | json_get "if(!p.data.id || !p.data.pages?.[0]?.id) process.exit(1); console.log(p.data.id);")
sensitive_job_json=$(api POST "/api/workspaces/$workspace_id/generation-jobs" "{\"job_type\":\"storybook_pages\",\"storybook_id\":\"$sensitive_storybook_id\",\"input_json\":{\"page_count\":4,\"theme\":\"provider 敏感输出拦截\"}}")
sensitive_job_id=$(echo "$sensitive_job_json" | json_get "if(p.data.job_type!=='storybook_pages' || p.data.status!=='queued') process.exit(1); console.log(p.data.id);")
sensitive_failed_json=$(wait_for_job_status "$workspace_id" "$sensitive_job_id" "failed")
echo "$sensitive_failed_json" | json_get "
if(p.data.output_json?.schema_version!=='generation.error.v1') process.exit(1);
if(p.data.output_json?.provider!=='deepseek') process.exit(1);
if(p.data.output_json?.mode!=='storybook_pages') process.exit(1);
if(p.data.output_json?.error?.retryable !== false) process.exit(1);
if(!p.data.last_error || !p.data.last_error.includes('包含敏感信息') || !p.data.last_error.includes('手机号')) process.exit(1);
if(p.data.next_run_at !== null) process.exit(1);
console.log('provider_sensitive_output_failed=' + p.data.id);
"
api GET "/api/workspaces/$workspace_id/storybooks/$sensitive_storybook_id" | json_get "
const text = JSON.stringify(p.data);
if(text.includes('138 0013 8000') || text.includes('敏感输出验证')) process.exit(1);
console.log('provider_sensitive_output_not_written=' + p.data.id);
"
api GET "/api/operator/generation-costs?workspace_id=$workspace_id&provider=deepseek&status=failed&limit=20&offset=0" | json_get "
const item = p.data.items.find((row)=>row.generation_job_id==='$sensitive_job_id');
if(!item || item.status !== 'failed' || item.estimated_cost_micros !== 0) process.exit(1);
console.log('provider_sensitive_output_cost=' + item.id);
"

echo "== generation provider failure smoke ok =="
