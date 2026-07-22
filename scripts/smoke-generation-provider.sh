#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
DB_CONTAINER="${DB_CONTAINER:-kindleaf-postgres}"
DB_USER="${DB_USER:-postgres}"
DB_PASSWORD="${DB_PASSWORD:-postgres}"
DB_HOST="${DB_HOST:-127.0.0.1}"
DB_PORT="${DB_PORT:-55432}"
DB_NAME="${DB_NAME:-kindleaf_generation_provider_smoke_$(date +%s)}"
API_PORT="${API_PORT:-8081}"
DEEPSEEK_PORT="${DEEPSEEK_PORT:-18182}"
API_BASE_URL="${API_BASE_URL:-http://127.0.0.1:$API_PORT}"
DEEPSEEK_BASE_URL="${DEEPSEEK_BASE_URL:-http://127.0.0.1:$DEEPSEEK_PORT}"
DATABASE_URL="${DATABASE_URL:-postgres://$DB_USER:$DB_PASSWORD@$DB_HOST:$DB_PORT/$DB_NAME}"
LOG_DIR="${LOG_DIR:-$ROOT_DIR/.tmp/smoke-generation-provider}"
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

wait_for_job() {
  local workspace_id="$1"
  local job_id="$2"
  for _ in $(seq 1 80); do
    local payload
    payload=$(api GET "/api/workspaces/$workspace_id/generation-jobs/$job_id")
    local status
    status=$(echo "$payload" | json_get "console.log(p.data.status)")
    if [[ "$status" == "succeeded" ]]; then
      echo "$payload"
      return 0
    fi
    if [[ "$status" == "failed" ]]; then
      echo "$payload" >&2
      return 1
    fi
    sleep 0.25
  done
  echo "generation job did not finish: $job_id" >&2
  api GET "/api/workspaces/$workspace_id/generation-jobs/$job_id" >&2 || true
  return 1
}

create_and_wait_job() {
  local workspace_id="$1"
  local job_type="$2"
  local body="$3"
  local job_json
  job_json=$(api POST "/api/workspaces/$workspace_id/generation-jobs" "$body")
  local job_id
  job_id=$(echo "$job_json" | json_get "if(p.data.job_type!=='$job_type' || p.data.status!=='queued') process.exit(1); console.log(p.data.id);")
  wait_for_job "$workspace_id" "$job_id"
}

mkdir -p "$LOG_DIR"
trap 'status=$?; cleanup; exit $status' EXIT

echo "== Kindleaf generation provider smoke =="
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

echo "2. start fake DeepSeek"
FAKE_DEEPSEEK_REQUIRE_REDACTED_CUSTOMIZATION=true \
  node "$ROOT_DIR/scripts/fake-deepseek.mjs" "$DEEPSEEK_PORT" >"$LOG_DIR/fake-deepseek.log" 2>&1 &
provider_pid="$!"
wait_for_url "$DEEPSEEK_BASE_URL/health" "fake DeepSeek"

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

echo "4. verify fake DeepSeek provider summary"
teacher_auth_header="$AUTH_HEADER"
operator_login_json=$(curl -fsS -H "Content-Type: application/json" -X POST "$API_BASE_URL/api/auth/login" -d '{"identifier":"lin@example.com","password":"demo"}')
operator_token=$(echo "$operator_login_json" | json_get "if(!p.data.token) process.exit(1); console.log(p.data.token);")
AUTH_HEADER="Authorization: Bearer $operator_token"
api GET "/api/operator/generation-provider" | json_get "
if(p.data.provider !== 'deepseek' || p.data.mode !== 'text') process.exit(1);
if(p.data.real_text_ready !== true || p.data.real_image_ready !== false) process.exit(1);
const text = p.data.components?.find((item)=>item.kind==='text' && item.provider==='deepseek');
if(!text || text.ready !== true || text.configured !== true) process.exit(1);
if(text.required_configuration?.length) process.exit(1);
if(!text.endpoint?.includes('/chat/completions')) process.exit(1);
console.log('deepseek_provider_summary=' + text.provider + '/' + text.model);
"
AUTH_HEADER="$teacher_auth_header"

echo "5. create real text generation jobs"
workspace_id=$(api GET "/api/workspaces" | json_get "const ws=p.data.find((item)=>item.type==='school' && item.role==='school_admin'); if(!ws) process.exit(1); console.log(ws.id);")
storybook_json=$(api POST "/api/workspaces/$workspace_id/storybooks" '{"title":"Provider Smoke 普通绘本","age_group":"4-5 岁","use_scene":"规则引导","teaching_goal":"学习等待和洗手步骤"}')
storybook_id=$(echo "$storybook_json" | json_get "if(p.data.type!=='plain') process.exit(1); console.log(p.data.id);")
child_json=$(api POST "/api/workspaces/$workspace_id/children" '{"nickname":"Provider Smoke 儿童","age_group":"4-5 岁","classroom":"小一班","interests":["洗手歌","贴纸"],"traits":["认真观察"],"focus":"排队等待和洗手步骤"}')
child_id=$(echo "$child_json" | json_get "if(!p.data.id) process.exit(1); console.log(p.data.id);")

plan_json=$(create_and_wait_job "$workspace_id" "storybook_plan" '{"job_type":"storybook_plan","input_json":{"theme":"排队洗手","age_group":"4-5 岁","teaching_goal":"学习等待和洗手步骤"}}')
echo "$plan_json" | json_get "if(p.data.output_json?.provider!=='deepseek') process.exit(1); if(p.data.output_json?.mode!=='storybook_plan') process.exit(1); if(p.data.output_json?.schema_version!=='generation.provider.v1') process.exit(1); if(p.data.output_json?.provider_usage?.prompt_tokens!==120 || p.data.output_json?.provider_usage?.completion_tokens!==80) process.exit(1); if(p.data.output_json?.plan?.title!=='排队洗手小约定') process.exit(1); console.log('deepseek_plan_job=' + p.data.id);"

roles_json=$(create_and_wait_job "$workspace_id" "storybook_roles" "{\"job_type\":\"storybook_roles\",\"storybook_id\":\"$storybook_id\",\"input_json\":{\"title\":\"Provider Smoke 角色\",\"teacher_name\":\"周老师\"}}")
echo "$roles_json" | json_get "if(p.data.output_json?.provider!=='deepseek') process.exit(1); if(p.data.output_json?.mode!=='storybook_roles') process.exit(1); if(!p.data.output_json?.roles?.some((role)=>role.name==='真真')) process.exit(1); console.log('deepseek_roles_job=' + p.data.id);"

pages_json=$(create_and_wait_job "$workspace_id" "storybook_pages" "{\"job_type\":\"storybook_pages\",\"storybook_id\":\"$storybook_id\",\"input_json\":{\"page_count\":2,\"theme\":\"排队洗手\"}}")
echo "$pages_json" | json_get "if(p.data.output_json?.provider!=='deepseek') process.exit(1); if(p.data.output_json?.mode!=='storybook_pages') process.exit(1); if(!p.data.output_json?.pages?.some((page)=>page.title==='水龙头前排好队')) process.exit(1); console.log('deepseek_pages_job=' + p.data.id);"

storybook_after_generation=$(api GET "/api/workspaces/$workspace_id/storybooks/$storybook_id")
echo "$storybook_after_generation" | json_get "if(!p.data.roles.some((role)=>role.name==='真真')) process.exit(1); if(!p.data.pages.some((page)=>page.title==='水龙头前排好队')) process.exit(1); console.log('deepseek_text_applied=' + p.data.id);"

customization_json=$(create_and_wait_job "$workspace_id" "customization_plan" "{\"job_type\":\"customization_plan\",\"storybook_id\":\"$storybook_id\",\"input_json\":{\"child_id\":\"$child_id\",\"child_nickname\":\"Provider Smoke 儿童\",\"intensity\":\"standard\",\"source_title\":\"Provider Smoke 普通绘本\",\"interests\":[\"洗手歌\",\"贴纸\"],\"focus\":\"排队等待和洗手步骤\",\"parent_email\":\"parent@example.com\",\"guardian_phone\":\"138 0013 8000\",\"family_note\":\"爸爸近期出差\"}}")
echo "$customization_json" | json_get "
if(p.data.output_json?.provider!=='deepseek') process.exit(1);
if(p.data.output_json?.mode!=='customization_plan') process.exit(1);
if(!p.data.output_json?.customization?.strategy?.includes('洗手排队')) process.exit(1);
const audit = p.data.output_json?.privacy_audit;
if(audit?.redacted !== true || !audit.labels?.includes('sensitive_field')) process.exit(1);
console.log('deepseek_customization_job=' + p.data.id);
"

echo "== generation provider smoke ok =="
