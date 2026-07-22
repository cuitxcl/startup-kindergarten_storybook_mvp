#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
DB_CONTAINER="${DB_CONTAINER:-kindleaf-postgres}"
DB_USER="${DB_USER:-postgres}"
DB_PASSWORD="${DB_PASSWORD:-postgres}"
DB_HOST="${DB_HOST:-127.0.0.1}"
DB_PORT="${DB_PORT:-55432}"
DB_NAME="${DB_NAME:-kindleaf_composite_provider_smoke_$(date +%s)}"
API_PORT="${API_PORT:-8081}"
DEEPSEEK_PORT="${DEEPSEEK_PORT:-18182}"
SEEDREAM_PORT="${SEEDREAM_PORT:-18183}"
API_BASE_URL="${API_BASE_URL:-http://127.0.0.1:$API_PORT}"
DEEPSEEK_BASE_URL="${DEEPSEEK_BASE_URL:-http://127.0.0.1:$DEEPSEEK_PORT}"
SEEDREAM_BASE_URL="${SEEDREAM_BASE_URL:-http://127.0.0.1:$SEEDREAM_PORT}"
DATABASE_URL="${DATABASE_URL:-postgres://$DB_USER:$DB_PASSWORD@$DB_HOST:$DB_PORT/$DB_NAME}"
LOG_DIR="${LOG_DIR:-$ROOT_DIR/.tmp/smoke-composite-provider}"
AUTH_HEADER="Authorization: Bearer ${API_TOKEN:-dev-token}"

server_pid=""
deepseek_pid=""
seedream_pid=""

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
  if [[ -n "$deepseek_pid" ]] && kill -0 "$deepseek_pid" 2>/dev/null; then
    kill "$deepseek_pid" 2>/dev/null || true
    wait "$deepseek_pid" 2>/dev/null || true
  fi
  if [[ -n "$seedream_pid" ]] && kill -0 "$seedream_pid" 2>/dev/null; then
    kill "$seedream_pid" 2>/dev/null || true
    wait "$seedream_pid" 2>/dev/null || true
  fi
  kill_listening_port "$API_PORT"
  kill_listening_port "$DEEPSEEK_PORT"
  kill_listening_port "$SEEDREAM_PORT"
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
  tail -120 "$LOG_DIR"/*.log 2>/dev/null || true
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

expect_png_download() {
  local file_url="$1"
  local target="/tmp/kindleaf-smoke-composite-image.png"
  curl -fsS -H "$AUTH_HEADER" "$API_BASE_URL$file_url" -o "$target"
  "$ROOT_DIR/scripts/validate-png.mjs" "$target" "$file_url"
  rm -f "$target"
}

mkdir -p "$LOG_DIR"
trap 'status=$?; cleanup; exit $status' EXIT

echo "== Kindleaf composite provider smoke =="
echo "API_BASE_URL=$API_BASE_URL"
echo "DEEPSEEK_BASE_URL=$DEEPSEEK_BASE_URL"
echo "SEEDREAM_BASE_URL=$SEEDREAM_BASE_URL"
echo "DB_NAME=$DB_NAME"
echo "logs=$LOG_DIR"

require_port_free "$API_PORT"
require_port_free "$DEEPSEEK_PORT"
require_port_free "$SEEDREAM_PORT"

docker exec "$DB_CONTAINER" dropdb -U "$DB_USER" "$DB_NAME" >/dev/null 2>&1 || true
docker exec "$DB_CONTAINER" createdb -U "$DB_USER" "$DB_NAME"

echo "1. migrate"
(
  cd "$ROOT_DIR/server"
  DATABASE_URL="$DATABASE_URL" cargo run --features db -- -e test db migrate
) >"$LOG_DIR/migrate.log" 2>&1

echo "2. start fake providers"
node "$ROOT_DIR/scripts/fake-deepseek.mjs" "$DEEPSEEK_PORT" >"$LOG_DIR/fake-deepseek.log" 2>&1 &
deepseek_pid="$!"
node "$ROOT_DIR/scripts/fake-seedream-image.mjs" "$SEEDREAM_PORT" >"$LOG_DIR/fake-seedream-image.log" 2>&1 &
seedream_pid="$!"
wait_for_url "$DEEPSEEK_BASE_URL/health" "fake DeepSeek"
wait_for_url "$SEEDREAM_BASE_URL/health" "fake Seedream image provider"

echo "3. start backend with composite provider"
(
  cd "$ROOT_DIR/server"
  KINDLEAF_DEMO_SEED=1 \
  KINDLEAF_GENERATION_PROVIDER= \
  DEEPSEEK_API_KEY=test-key \
  DEEPSEEK_BASE_URL="$DEEPSEEK_BASE_URL" \
  SEEDREAM_API_KEY=test-key \
  SEEDREAM_BASE_URL="$SEEDREAM_BASE_URL" \
  SEEDREAM_IMAGE_MODEL=doubao-seedream-5-0-lite \
  DATABASE_URL="$DATABASE_URL" \
  cargo run --features db -- -e test start
) >"$LOG_DIR/server.log" 2>&1 &
server_pid="$!"
wait_for_url "$API_BASE_URL/api/health" "backend"

echo "4. verify provider summary"
login_json=$(curl -fsS -H "Content-Type: application/json" -X POST "$API_BASE_URL/api/auth/login" -d '{"identifier":"lin@example.com","password":"demo"}')
API_TOKEN=$(echo "$login_json" | json_get "if(!p.data.token) process.exit(1); console.log(p.data.token)")
AUTH_HEADER="Authorization: Bearer $API_TOKEN"
api GET "/api/operator/generation-provider" | json_get "
if(p.data.mode!=='composite') process.exit(1);
if(!p.data.real_text_ready || !p.data.real_image_ready || !p.data.production_ready) process.exit(1);
const text = p.data.components?.find((item)=>item.kind==='text' && item.provider==='deepseek');
const image = p.data.components?.find((item)=>item.kind==='image' && item.provider==='seedream');
if(!text || !image || !text.ready || !image.ready) process.exit(1);
if(text.required_configuration?.length || image.required_configuration?.length) process.exit(1);
if(!image.model || !image.endpoint?.includes('/api/v3/images/generations')) process.exit(1);
console.log('provider=' + p.data.provider + '/' + image.provider);
"

echo "5. create text and image jobs"
workspace_id=$(api GET "/api/workspaces" | json_get "const ws=p.data.find((item)=>item.type==='school' && item.role==='school_admin'); if(!ws) process.exit(1); console.log(ws.id);")
storybook_json=$(api POST "/api/workspaces/$workspace_id/storybooks" '{"title":"Composite Provider Smoke 绘本","age_group":"4-5 岁","use_scene":"课堂共读","teaching_goal":"验证组合 provider"}')
storybook_id=$(echo "$storybook_json" | json_get "if(p.data.type!=='plain' || !p.data.pages?.[0]?.id) process.exit(1); console.log(p.data.id);")
page_id=$(echo "$storybook_json" | json_get "console.log(p.data.pages[0].id);")

plan_job_json=$(api POST "/api/workspaces/$workspace_id/generation-jobs" '{"job_type":"storybook_plan","input_json":{"theme":"排队洗手","age_group":"4-5 岁"}}')
plan_job_id=$(echo "$plan_job_json" | json_get "if(p.data.job_type!=='storybook_plan' || p.data.status!=='queued') process.exit(1); console.log(p.data.id);")
plan_finished_json=$(wait_for_job "$workspace_id" "$plan_job_id")
echo "$plan_finished_json" | json_get "if(p.data.output_json?.provider!=='deepseek') process.exit(1); if(p.data.output_json?.mode!=='storybook_plan') process.exit(1); console.log('composite_text_job=' + p.data.id);"

image_job_json=$(api POST "/api/workspaces/$workspace_id/storybooks/$storybook_id/pages/$page_id/image-tasks" '{"prompt":"幼儿园教室里，老师和孩子一起读绘本，温暖纸感插图"}')
image_job_id=$(echo "$image_job_json" | json_get "if(p.data.job_type!=='storybook_page_image' || p.data.status!=='queued') process.exit(1); console.log(p.data.id);")
image_finished_json=$(wait_for_job "$workspace_id" "$image_job_id")
image_file_url=$(echo "$image_finished_json" | json_get "const expected='/api/workspaces/$workspace_id/generation-jobs/$image_job_id/image'; if(p.data.output_json?.provider!=='seedream') process.exit(1); if(p.data.output_json?.mode!=='storybook_page_image') process.exit(1); if(p.data.output_json?.image?.image_url!==expected) process.exit(1); console.log(expected);")
expect_png_download "$image_file_url"
echo "composite_image_job=$image_job_id"

echo "== composite provider smoke ok =="
