#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
source "$ROOT_DIR/scripts/load-env.sh"
kindleaf_load_env_files "$ROOT_DIR"

DB_CONTAINER="${DB_CONTAINER:-kindleaf-postgres}"
DB_USER="${DB_USER:-postgres}"
DB_PASSWORD="${DB_PASSWORD:-postgres}"
DB_HOST="${DB_HOST:-127.0.0.1}"
DB_PORT="${DB_PORT:-55432}"
DB_NAME="${DB_NAME:-kindleaf_real_composite_smoke_$(date +%s)}"
API_PORT="${API_PORT:-8081}"
API_BASE_URL="${API_BASE_URL:-http://127.0.0.1:$API_PORT}"
DATABASE_URL="${DATABASE_URL:-postgres://$DB_USER:$DB_PASSWORD@$DB_HOST:$DB_PORT/$DB_NAME}"
LOG_DIR="${LOG_DIR:-$ROOT_DIR/.tmp/smoke-real-composite-provider}"
AUTH_HEADER="Authorization: Bearer ${API_TOKEN:-dev-token}"

server_pid=""

"$ROOT_DIR/scripts/check-real-provider-readiness.sh" --composite

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
  rm -f "$ROOT_DIR/tmp/generated-images/seedream-"*.png 2>/dev/null || true
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
  for _ in $(seq 1 120); do
    if curl -fsS "$url" >/dev/null 2>&1; then
      return 0
    fi
    sleep 0.25
  done
  echo "$label did not become ready: $url" >&2
  tail -160 "$LOG_DIR/server.log" 2>/dev/null || true
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

expect_status() {
  local expected="$1"
  local method="$2"
  local path="$3"
  local auth="${4:-auth}"
  local body="${5:-}"
  local response_file="$LOG_DIR/expect-status-response.json"
  local args=(-sS -o "$response_file" -w "%{http_code}" -X "$method" "$API_BASE_URL$path")
  if [[ "$auth" == "auth" ]]; then
    args+=(-H "$AUTH_HEADER")
  fi
  if [[ -n "$body" ]]; then
    args+=(-H "Content-Type: application/json" -d "$body")
  fi
  local actual
  actual=$(curl "${args[@]}")
  if [[ "$actual" != "$expected" ]]; then
    echo "expected HTTP $expected but got $actual for $method $path" >&2
    cat "$response_file" >&2 || true
    return 1
  fi
}

wait_for_job() {
  local workspace_id="$1"
  local job_id="$2"
  for _ in $(seq 1 180); do
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
    sleep 0.5
  done
  echo "real composite generation job did not finish: $job_id" >&2
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

expect_png_download() {
  local file_url="$1"
  local target="${REAL_COMPOSITE_OUTPUT:-$ROOT_DIR/.tmp/real-composite-seedream-image.png}"
  mkdir -p "$(dirname "$target")"
  curl -fsS -H "$AUTH_HEADER" "$API_BASE_URL$file_url" -o "$target"
  "$ROOT_DIR/scripts/validate-png.mjs" "$target" "$file_url"
  echo "$target"
}

mkdir -p "$LOG_DIR"
trap 'status=$?; cleanup; exit $status' EXIT

echo "== Kindleaf real composite provider smoke =="
echo "API_BASE_URL=$API_BASE_URL"
echo "DEEPSEEK_BASE_URL=${DEEPSEEK_BASE_URL:-https://api.deepseek.com}"
echo "DEEPSEEK_MODEL=${DEEPSEEK_MODEL:-deepseek-v4-flash}"
echo "Seedream base URL=${SEEDREAM_BASE_URL:-${ARK_BASE_URL:-https://ark.cn-beijing.volces.com}}"
echo "Seedream image model=${SEEDREAM_IMAGE_MODEL:-${ARK_IMAGE_MODEL:-doubao-seedream-5-0-lite}}"
echo "DB_NAME=$DB_NAME"
echo "logs=$LOG_DIR"
echo "This script calls real DeepSeek and real Seedream providers and may consume quota."

require_port_free "$API_PORT"

docker exec "$DB_CONTAINER" dropdb -U "$DB_USER" "$DB_NAME" >/dev/null 2>&1 || true
docker exec "$DB_CONTAINER" createdb -U "$DB_USER" "$DB_NAME"

echo "1. migrate"
(
  cd "$ROOT_DIR/server"
  DATABASE_URL="$DATABASE_URL" cargo run --features db -- -e test db migrate
) >"$LOG_DIR/migrate.log" 2>&1

echo "2. start backend with real composite provider"
(
  cd "$ROOT_DIR/server"
  KINDLEAF_DEMO_SEED=1 \
  KINDLEAF_GENERATION_PROVIDER= \
  DATABASE_URL="$DATABASE_URL" \
  PORT="$API_PORT" \
  APP_HOST="$API_BASE_URL" \
  cargo run --features db -- -e test start
) >"$LOG_DIR/server.log" 2>&1 &
server_pid="$!"
wait_for_url "$API_BASE_URL/api/health" "backend"

echo "3. verify real composite provider readiness"
teacher_auth_header="$AUTH_HEADER"
operator_login_json=$(curl -fsS -H "Content-Type: application/json" -X POST "$API_BASE_URL/api/auth/login" -d '{"identifier":"lin@example.com","password":"demo"}')
operator_token=$(echo "$operator_login_json" | json_get "if(!p.data.token) process.exit(1); console.log(p.data.token);")
AUTH_HEADER="Authorization: Bearer $operator_token"
api GET "/api/operator/generation-provider" | json_get "
if(p.data.provider !== 'deepseek+seedream' || p.data.mode !== 'composite') process.exit(1);
if(p.data.real_text_ready !== true || p.data.real_image_ready !== true || p.data.production_ready !== true) process.exit(1);
const text = p.data.components?.find((item)=>item.kind==='text' && item.provider==='deepseek');
const image = p.data.components?.find((item)=>item.kind==='image' && item.provider==='seedream');
if(!text || !image || text.ready !== true || image.ready !== true) process.exit(1);
if(text.configured !== true || image.configured !== true) process.exit(1);
if(text.required_configuration?.length || image.required_configuration?.length) process.exit(1);
const configuredTextEndpoint = process.env.DEEPSEEK_ENDPOINT_PATH || '/chat/completions';
if(!text.model || !text.endpoint) process.exit(1);
if(configuredTextEndpoint.startsWith('http://') || configuredTextEndpoint.startsWith('https://')) {
  if(text.endpoint !== configuredTextEndpoint) process.exit(1);
} else if(!text.endpoint.endsWith(configuredTextEndpoint)) {
  process.exit(1);
}
const configuredImageEndpoint = process.env.SEEDREAM_ENDPOINT_PATH || process.env.ARK_IMAGE_ENDPOINT_PATH || '/api/v3/images/generations';
if(!image.model || !image.endpoint) process.exit(1);
if(configuredImageEndpoint.startsWith('http://') || configuredImageEndpoint.startsWith('https://')) {
  if(image.endpoint !== configuredImageEndpoint) process.exit(1);
} else if(!image.endpoint.endsWith(configuredImageEndpoint)) {
  process.exit(1);
}
console.log('real_composite_provider=' + text.provider + '+' + image.provider + '/' + image.model);
"
AUTH_HEADER="$teacher_auth_header"

echo "4. create real text generation jobs"
workspace_id=$(api GET "/api/workspaces" | json_get "const ws=p.data.find((item)=>item.type==='school' && item.role==='school_admin'); if(!ws) process.exit(1); console.log(ws.id);")
storybook_json=$(api POST "/api/workspaces/$workspace_id/storybooks" '{"title":"Real Composite Smoke 普通绘本","age_group":"4-5 岁","use_scene":"班级共读","teaching_goal":"学习轮流等待和温和表达"}')
storybook_id=$(echo "$storybook_json" | json_get "if(p.data.type!=='plain') process.exit(1); console.log(p.data.id);")

plan_json=$(create_and_wait_job "$workspace_id" "storybook_plan" '{"job_type":"storybook_plan","input_json":{"theme":"轮流等待","age_group":"4-5 岁","teaching_goal":"学习轮流等待和温和表达","use_scene":"班级共读"}}')
plan_job_id=$(echo "$plan_json" | json_get "if(p.data.output_json?.provider!=='deepseek') process.exit(1); if(p.data.output_json?.mode!=='storybook_plan') process.exit(1); if(p.data.output_json?.schema_version!=='generation.provider.v1') process.exit(1); if(!p.data.output_json?.plan?.title || !Array.isArray(p.data.output_json?.plan?.outline)) process.exit(1); console.log(p.data.id);")
echo "real_composite_plan_job=$plan_job_id"

roles_json=$(create_and_wait_job "$workspace_id" "storybook_roles" "{\"job_type\":\"storybook_roles\",\"storybook_id\":\"$storybook_id\",\"input_json\":{\"title\":\"Real Composite Smoke 角色\",\"theme\":\"轮流等待\",\"teacher_name\":\"林老师\"}}")
roles_job_id=$(echo "$roles_json" | json_get "if(p.data.output_json?.provider!=='deepseek') process.exit(1); if(p.data.output_json?.mode!=='storybook_roles') process.exit(1); if(!Array.isArray(p.data.output_json?.roles) || !p.data.output_json.roles.length) process.exit(1); console.log(p.data.id);")
echo "real_composite_roles_job=$roles_job_id"

pages_json=$(create_and_wait_job "$workspace_id" "storybook_pages" "{\"job_type\":\"storybook_pages\",\"storybook_id\":\"$storybook_id\",\"input_json\":{\"page_count\":4,\"theme\":\"轮流等待\",\"teaching_goal\":\"学习轮流等待和温和表达\"}}")
pages_job_id=$(echo "$pages_json" | json_get "if(p.data.output_json?.provider!=='deepseek') process.exit(1); if(p.data.output_json?.mode!=='storybook_pages') process.exit(1); if(!Array.isArray(p.data.output_json?.pages) || !p.data.output_json.pages.length) process.exit(1); console.log(p.data.id);")
echo "real_composite_pages_job=$pages_job_id"

storybook_after_text=$(api GET "/api/workspaces/$workspace_id/storybooks/$storybook_id")
page_id=$(echo "$storybook_after_text" | json_get "if(!p.data.roles.length) process.exit(1); if(!p.data.pages.length) process.exit(1); console.log(p.data.pages[0].id);")
echo "real_composite_text_applied=$storybook_id"

echo "5. create one real Seedream image job"
image_job_json=$(api POST "/api/workspaces/$workspace_id/storybooks/$storybook_id/pages/$page_id/image-tasks" '{"prompt":"儿童绘本插图，温暖幼儿园教室，老师和孩子一起练习轮流等待，纸感水彩风格，安全友好，不要文字，不要水印"}')
image_job_id=$(echo "$image_job_json" | json_get "if(p.data.job_type!=='storybook_page_image' || p.data.status!=='queued') process.exit(1); console.log(p.data.id);")
finished_json=$(wait_for_job "$workspace_id" "$image_job_id")
image_file_url=$(echo "$finished_json" | json_get "const expected='/api/workspaces/$workspace_id/generation-jobs/$image_job_id/image'; if(p.data.output_json?.provider!=='seedream') process.exit(1); if(p.data.output_json?.mode!=='storybook_page_image') process.exit(1); if(p.data.output_json?.schema_version!=='generation.provider.v1') process.exit(1); if(p.data.output_json?.image?.image_url!==expected) process.exit(1); console.log(expected);")
output_path=$(expect_png_download "$image_file_url")
echo "real_composite_image_job=$image_job_id"
echo "real_composite_image=$output_path"

echo "6. verify image download authorization"
expect_status 401 GET "$image_file_url" noauth
registered_email="real-composite-cross-space-$RANDOM-$(date +%s)@example.com"
registered_json=$(curl -fsS -H "Content-Type: application/json" -X POST "$API_BASE_URL/api/auth/register" -d "{\"display_name\":\"组合跨空间检查\",\"email\":\"$registered_email\",\"password\":\"password123\"}")
registered_token=$(echo "$registered_json" | json_get "if(p.data.user.email!=='$registered_email' || !p.data.token) process.exit(1); console.log(p.data.token);")
AUTH_HEADER="Authorization: Bearer $registered_token"
expect_status 403 GET "$image_file_url" auth

echo "7. verify operator generation cost ledger"
AUTH_HEADER="Authorization: Bearer $operator_token"
api GET "/api/operator/generation-costs?workspace_id=$workspace_id&status=succeeded&limit=20&offset=0" | json_get "
const expected = ['$plan_job_id', '$roles_job_id', '$pages_job_id', '$image_job_id'];
const ids = new Set(p.data.items.map((row)=>row.generation_job_id));
if(!expected.every((id)=>ids.has(id))) process.exit(1);
if(!p.meta || p.meta.total < expected.length) process.exit(1);
if(!p.data.items.some((row)=>row.provider === 'deepseek')) process.exit(1);
if(!p.data.items.some((row)=>row.provider === 'seedream')) process.exit(1);
console.log('real_composite_costs=' + p.data.items.length + '/' + p.meta.total);
"

echo "== real composite provider smoke ok =="
