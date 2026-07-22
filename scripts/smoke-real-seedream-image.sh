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
DB_NAME="${DB_NAME:-kindleaf_real_seedream_smoke_$(date +%s)}"
API_PORT="${API_PORT:-8081}"
API_BASE_URL="${API_BASE_URL:-http://127.0.0.1:$API_PORT}"
DATABASE_URL="${DATABASE_URL:-postgres://$DB_USER:$DB_PASSWORD@$DB_HOST:$DB_PORT/$DB_NAME}"
LOG_DIR="${LOG_DIR:-$ROOT_DIR/.tmp/smoke-real-seedream-image}"
AUTH_HEADER="Authorization: Bearer ${API_TOKEN:-dev-token}"

server_pid=""

"$ROOT_DIR/scripts/check-real-provider-readiness.sh" --seedream

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
  for _ in $(seq 1 160); do
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
  echo "real Seedream image job did not finish: $job_id" >&2
  api GET "/api/workspaces/$workspace_id/generation-jobs/$job_id" >&2 || true
  return 1
}

expect_png_download() {
  local file_url="$1"
  local target="${REAL_SEEDREAM_OUTPUT:-$ROOT_DIR/.tmp/real-seedream-image.png}"
  mkdir -p "$(dirname "$target")"
  curl -fsS -H "$AUTH_HEADER" "$API_BASE_URL$file_url" -o "$target"
  "$ROOT_DIR/scripts/validate-png.mjs" "$target" "$file_url"
  echo "$target"
}

mkdir -p "$LOG_DIR"
trap 'status=$?; cleanup; exit $status' EXIT

echo "== Kindleaf real Seedream image smoke =="
echo "API_BASE_URL=$API_BASE_URL"
echo "Seedream base URL=${SEEDREAM_BASE_URL:-${ARK_BASE_URL:-https://ark.cn-beijing.volces.com}}"
echo "Seedream image model=${SEEDREAM_IMAGE_MODEL:-${ARK_IMAGE_MODEL:-doubao-seedream-5-0-lite}}"
echo "DB_NAME=$DB_NAME"
echo "logs=$LOG_DIR"
echo "This script calls the real Seedream provider and may consume quota."

require_port_free "$API_PORT"

docker exec "$DB_CONTAINER" dropdb -U "$DB_USER" "$DB_NAME" >/dev/null 2>&1 || true
docker exec "$DB_CONTAINER" createdb -U "$DB_USER" "$DB_NAME"

echo "1. migrate"
(
  cd "$ROOT_DIR/server"
  DATABASE_URL="$DATABASE_URL" cargo run --features db -- -e test db migrate
) >"$LOG_DIR/migrate.log" 2>&1

echo "2. start backend with real Seedream image provider"
(
  cd "$ROOT_DIR/server"
  KINDLEAF_DEMO_SEED=1 \
  KINDLEAF_GENERATION_PROVIDER=seedream \
  DATABASE_URL="$DATABASE_URL" \
  PORT="$API_PORT" \
  APP_HOST="$API_BASE_URL" \
  cargo run --features db -- -e test start
) >"$LOG_DIR/server.log" 2>&1 &
server_pid="$!"
wait_for_url "$API_BASE_URL/api/health" "backend"

echo "3. verify real Seedream provider readiness"
teacher_auth_header="$AUTH_HEADER"
operator_login_json=$(curl -fsS -H "Content-Type: application/json" -X POST "$API_BASE_URL/api/auth/login" -d '{"identifier":"lin@example.com","password":"demo"}')
operator_token=$(echo "$operator_login_json" | json_get "if(!p.data.token) process.exit(1); console.log(p.data.token);")
AUTH_HEADER="Authorization: Bearer $operator_token"
api GET "/api/operator/generation-provider" | json_get "
if(p.data.provider !== 'seedream') process.exit(1);
if(p.data.mode !== 'image' || p.data.real_image_ready !== true) process.exit(1);
const image = p.data.components?.find((item)=>item.kind==='image' && item.provider==='seedream');
if(!image || image.ready !== true || image.configured !== true) process.exit(1);
if(image.required_configuration?.length) process.exit(1);
const configuredEndpoint = process.env.SEEDREAM_ENDPOINT_PATH || process.env.ARK_IMAGE_ENDPOINT_PATH || '/api/v3/images/generations';
if(!image.model || !image.endpoint) process.exit(1);
if(configuredEndpoint.startsWith('http://') || configuredEndpoint.startsWith('https://')) {
  if(image.endpoint !== configuredEndpoint) process.exit(1);
} else if(!image.endpoint.endsWith(configuredEndpoint)) {
  process.exit(1);
}
console.log('real_seedream_provider=' + image.provider + '/' + image.model);
"
AUTH_HEADER="$teacher_auth_header"

echo "4. create one real image generation job"
workspace_id=$(api GET "/api/workspaces" | json_get "const ws=p.data.find((item)=>item.type==='school' && item.role==='school_admin'); if(!ws) process.exit(1); console.log(ws.id);")
storybook_json=$(api POST "/api/workspaces/$workspace_id/storybooks" '{"title":"Real Seedream Smoke 绘本","age_group":"4-5 岁","use_scene":"课堂共读","teaching_goal":"验证真实 Seedream 插图"}')
storybook_id=$(echo "$storybook_json" | json_get "if(p.data.type!=='plain' || !p.data.pages?.[0]?.id) process.exit(1); console.log(p.data.id);")
page_id=$(echo "$storybook_json" | json_get "console.log(p.data.pages[0].id);")
image_job_json=$(api POST "/api/workspaces/$workspace_id/storybooks/$storybook_id/pages/$page_id/image-tasks" '{"prompt":"儿童绘本插图，温暖幼儿园教室，老师和三名孩子围坐读绘本，纸感水彩风格，柔和光线，安全友好，不要文字，不要水印"}')
image_job_id=$(echo "$image_job_json" | json_get "if(p.data.job_type!=='storybook_page_image' || p.data.status!=='queued') process.exit(1); console.log(p.data.id);")
finished_json=$(wait_for_job "$workspace_id" "$image_job_id")
image_file_url=$(echo "$finished_json" | json_get "const expected='/api/workspaces/$workspace_id/generation-jobs/$image_job_id/image'; if(p.data.output_json?.provider!=='seedream') process.exit(1); if(p.data.output_json?.mode!=='storybook_page_image') process.exit(1); if(p.data.output_json?.schema_version!=='generation.provider.v1') process.exit(1); if(p.data.output_json?.image?.image_url!==expected) process.exit(1); console.log(expected);")
output_path=$(expect_png_download "$image_file_url")
echo "real_seedream_image_job=$image_job_id"
echo "real_seedream_storybook=$storybook_id"
echo "real_seedream_image=$output_path"

echo "5. verify image download authorization"
expect_status 401 GET "$image_file_url" noauth
registered_email="real-seedream-cross-space-$RANDOM-$(date +%s)@example.com"
registered_json=$(curl -fsS -H "Content-Type: application/json" -X POST "$API_BASE_URL/api/auth/register" -d "{\"display_name\":\"Seedream跨空间检查\",\"email\":\"$registered_email\",\"password\":\"password123\"}")
registered_token=$(echo "$registered_json" | json_get "if(p.data.user.email!=='$registered_email' || !p.data.token) process.exit(1); console.log(p.data.token);")
AUTH_HEADER="Authorization: Bearer $registered_token"
expect_status 403 GET "$image_file_url" auth

echo "6. verify operator generation cost ledger"
AUTH_HEADER="Authorization: Bearer $operator_token"
api GET "/api/operator/generation-costs?workspace_id=$workspace_id&provider=seedream&job_type=storybook_page_image&status=succeeded&limit=20&offset=0" | json_get "
const item = p.data.items.find((row)=>row.generation_job_id==='$image_job_id');
if(!item) process.exit(1);
if(item.provider !== 'seedream' || item.job_type !== 'storybook_page_image' || item.status !== 'succeeded') process.exit(1);
if(item.image_count !== 1) process.exit(1);
if(!p.meta || p.meta.total < 1) process.exit(1);
if(!p.data.summary || p.data.summary.total_images < 1 || p.data.summary.total_jobs < 1) process.exit(1);
console.log('real_seedream_cost=' + item.estimated_cost_micros + '/' + item.currency);
"

echo "== real Seedream image smoke ok =="
