#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
DB_CONTAINER="${DB_CONTAINER:-kindleaf-postgres}"
DB_USER="${DB_USER:-postgres}"
DB_PASSWORD="${DB_PASSWORD:-postgres}"
DB_HOST="${DB_HOST:-127.0.0.1}"
DB_PORT="${DB_PORT:-55432}"
DB_NAME="${DB_NAME:-kindleaf_storybook_assets_smoke_$(date +%s)}"
API_PORT="${API_PORT:-8081}"
API_BASE_URL="${API_BASE_URL:-http://127.0.0.1:$API_PORT}"
DATABASE_URL="${DATABASE_URL:-postgres://$DB_USER:$DB_PASSWORD@$DB_HOST:$DB_PORT/$DB_NAME}"
LOG_DIR="${LOG_DIR:-$ROOT_DIR/.tmp/smoke-storybook-creation-assets}"
KEEP_DB="${KEEP_DB:-false}"

server_pid=""

json_get() {
  local script="$1"
  node -e "let s='';process.stdin.on('data',d=>s+=d);process.stdin.on('end',()=>{const p=JSON.parse(s); ${script}});"
}

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

expect_error() {
  local expected_status="$1"
  local expected_code="$2"
  local method="$3"
  local path="$4"
  local body="${5:-}"
  local actual
  if [[ -n "$body" ]]; then
    actual=$(curl -sS -o /tmp/kindleaf-assets-smoke-error.json -w "%{http_code}" -H "$AUTH_HEADER" -H "Content-Type: application/json" -X "$method" "$API_BASE_URL$path" -d "$body")
  else
    actual=$(curl -sS -o /tmp/kindleaf-assets-smoke-error.json -w "%{http_code}" -H "$AUTH_HEADER" -X "$method" "$API_BASE_URL$path")
  fi
  if [[ "$actual" != "$expected_status" ]]; then
    echo "expected HTTP $expected_status but got $actual for $method $path" >&2
    cat /tmp/kindleaf-assets-smoke-error.json >&2 || true
    exit 1
  fi
  node -e "
const fs = require('fs');
const p = JSON.parse(fs.readFileSync('/tmp/kindleaf-assets-smoke-error.json', 'utf8'));
if (p.error?.code !== '$expected_code') {
  console.error('expected $expected_code but got ' + p.error?.code);
  process.exit(1);
}
console.log('error_$expected_code=ok');
"
}

upload_asset() {
  local kind="$1"
  local key="$2"
  curl -fsS -H "$AUTH_HEADER" \
    -F "file=@$PNG_FILE;filename=$kind.png;type=image/png" \
    -F "kind=$kind" \
    -F "idempotency_key=$key" \
    "$API_BASE_URL/api/workspaces/$workspace_id/storybook-creation-sessions/$session_id/assets"
}

wait_for_asset_reference_status() {
  local wait_session_id="$1"
  local wait_asset_reference_id="$2"
  local expected_status="$3"
  local payload=""
  local status=""
  for _ in $(seq 1 120); do
    payload=$(api GET "/api/workspaces/$workspace_id/storybook-creation-sessions/$wait_session_id")
    status=$(echo "$payload" | json_get "const ref=p.data.asset_references.find((item)=>item.id==='$wait_asset_reference_id'); console.log(ref?.status || 'missing');")
    if [[ "$status" == "$expected_status" ]]; then
      printf '%s' "$payload"
      return 0
    fi
    if [[ "$status" == "failed" ]]; then
      echo "asset reference $wait_asset_reference_id failed while waiting for $expected_status" >&2
      echo "$payload" >&2
      return 1
    fi
    sleep 0.25
  done
  echo "timed out waiting for asset reference $wait_asset_reference_id to become $expected_status; last status=$status" >&2
  echo "$payload" >&2
  return 1
}

wait_for_generation_job_succeeded() {
  local wait_job_id="$1"
  local status=""
  for _ in $(seq 1 120); do
    status=$(docker exec -i "$DB_CONTAINER" psql -U "$DB_USER" -d "$DB_NAME" -v ON_ERROR_STOP=1 -tAc "select status from generation_jobs where id = '$wait_job_id'")
    if [[ "$status" == "succeeded" ]]; then
      return 0
    fi
    if [[ "$status" == "failed" ]]; then
      echo "generation job $wait_job_id failed" >&2
      docker exec -i "$DB_CONTAINER" psql -U "$DB_USER" -d "$DB_NAME" -v ON_ERROR_STOP=1 -tAc "select coalesce(last_error, 'no error') from generation_jobs where id = '$wait_job_id'" >&2 || true
      return 1
    fi
    sleep 0.25
  done
  echo "timed out waiting for generation job $wait_job_id to succeed; last status=$status" >&2
  return 1
}

mkdir -p "$LOG_DIR"
trap 'status=$?; cleanup; exit $status' EXIT

echo "== Kindleaf storybook creation assets smoke =="
echo "API_BASE_URL=$API_BASE_URL"
echo "DB_CONTAINER=$DB_CONTAINER"
echo "DB_NAME=$DB_NAME"
echo "DATABASE_URL=$DATABASE_URL"
echo "logs=$LOG_DIR"

require_port_free "$API_PORT"
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
  cargo run --features db -- -e test start
) >"$LOG_DIR/server.log" 2>&1 &
server_pid="$!"
wait_for_api

echo "3. login and create session"
login_json=$(curl -fsS -H "Content-Type: application/json" -X POST "$API_BASE_URL/api/auth/login" -d '{"identifier":"lin@example.com","password":"demo"}')
API_TOKEN=$(echo "$login_json" | json_get "if(!p.data.token) process.exit(1); console.log(p.data.token)")
AUTH_HEADER="Authorization: Bearer $API_TOKEN"
ADMIN_AUTH_HEADER="$AUTH_HEADER"
workspace_id=$(api GET "/api/workspaces" | json_get "const ws=p.data.find((item)=>item.type==='school' && item.role==='school_admin'); if(!ws) process.exit(1); console.log(ws.id);")
admin_id=$(docker exec -i "$DB_CONTAINER" psql -U "$DB_USER" -d "$DB_NAME" -v ON_ERROR_STOP=1 -tAc "select user_id from workspace_members where workspace_id = '$workspace_id' and role = 'school_admin' and status = 'active' order by created_at limit 1")
if [[ -z "$admin_id" ]]; then
  echo "school admin user id not found for workspace $workspace_id" >&2
  exit 1
fi
docker exec -i "$DB_CONTAINER" psql -U "$DB_USER" -d "$DB_NAME" -v ON_ERROR_STOP=1 >/dev/null <<SQL
insert into users (id, display_name, email, password_hash, status, created_at, updated_at)
values ('00000000-0000-0000-0000-000000000002', '王老师', 'wang@example.com', 'demo', 'active', now(), now())
on conflict (email) do update
  set display_name = excluded.display_name,
      password_hash = excluded.password_hash,
      status = excluded.status,
      updated_at = now();
SQL
teacher_login_json=$(curl -fsS -H "Content-Type: application/json" -X POST "$API_BASE_URL/api/auth/login" -d '{"identifier":"wang@example.com","password":"demo"}')
TEACHER_API_TOKEN=$(echo "$teacher_login_json" | json_get "if(!p.data.token) process.exit(1); console.log(p.data.token)")
TEACHER_AUTH_HEADER="Authorization: Bearer $TEACHER_API_TOKEN"
session_json=$(api POST "/api/workspaces/$workspace_id/storybook-creation-sessions" '{"quick_idea":"给乐乐做一本爸爸和小汽车一起学会等待的故事。","page_count":6}')
session_id=$(echo "$session_json" | json_get "if(p.data.status!=='understanding_ready') process.exit(1); console.log(p.data.id)")
echo "creation_session=$session_id"

PNG_FILE="$LOG_DIR/reference.png"
node -e "const fs=require('fs'); fs.writeFileSync('$PNG_FILE', Buffer.from('iVBORw0KGgoAAAANSUhEUgAAAAIAAAACCAIAAAD91JpzAAAAEklEQVR4nGP4cGnfsxNbGCAUAEWMCcWN1afmAAAAAElFTkSuQmCC', 'base64'));"
NOT_IMAGE_FILE="$LOG_DIR/not-image.txt"
printf 'this is not a real image\n' > "$NOT_IMAGE_FILE"
OVERSIZED_IMAGE_FILE="$LOG_DIR/oversized.png"
node -e "const fs=require('fs'); const header=Buffer.from('iVBORw0KGgoAAAANSUhEUgAAAAIAAAACCAIAAAD91JpzAAAAEklEQVR4nGP4cGnfsxNbGCAUAEWMCcWN1afmAAAAAElFTkSuQmCC', 'base64'); const padding=Buffer.alloc(10 * 1024 * 1024 + 1, 0); fs.writeFileSync('$OVERSIZED_IMAGE_FILE', Buffer.concat([header, padding]));"

echo "4. upload and restore photo assets"
policy_json=$(api GET "/api/workspaces/$workspace_id/storybook-creation-sessions/$session_id/asset-upload-policy")
echo "$policy_json" | json_get "if(p.data.max_files!==5 || p.data.remaining_slots!==5 || !p.data.accepted_content_types.includes('image/png')) process.exit(1); console.log('upload_policy=' + p.data.remaining_slots + '/' + p.data.max_files);"

invalid_file_status=$(curl -sS -o /tmp/kindleaf-assets-smoke-error.json -w "%{http_code}" -H "$AUTH_HEADER" \
  -F "file=@$NOT_IMAGE_FILE;filename=fake.png;type=image/png" \
  -F "kind=person" \
  -F "idempotency_key=asset-smoke-invalid-file" \
  "$API_BASE_URL/api/workspaces/$workspace_id/storybook-creation-sessions/$session_id/assets")
if [[ "$invalid_file_status" != "400" ]]; then
  echo "expected HTTP 400 for fake image but got $invalid_file_status" >&2
  cat /tmp/kindleaf-assets-smoke-error.json >&2 || true
  exit 1
fi
node -e "
const fs = require('fs');
const p = JSON.parse(fs.readFileSync('/tmp/kindleaf-assets-smoke-error.json', 'utf8'));
if (p.error?.code !== 'unsupported_file_type') {
  console.error('expected unsupported_file_type but got ' + p.error?.code);
  process.exit(1);
}
console.log('unsupported_file_signature=ok');
"

oversized_file_status=$(curl -sS -o /tmp/kindleaf-assets-smoke-error.json -w "%{http_code}" -H "$AUTH_HEADER" \
  -F "file=@$OVERSIZED_IMAGE_FILE;filename=oversized.png;type=image/png" \
  -F "kind=person" \
  -F "idempotency_key=asset-smoke-oversized-file" \
  "$API_BASE_URL/api/workspaces/$workspace_id/storybook-creation-sessions/$session_id/assets")
if [[ "$oversized_file_status" != "400" ]]; then
  echo "expected HTTP 400 for oversized image but got $oversized_file_status" >&2
  cat /tmp/kindleaf-assets-smoke-error.json >&2 || true
  exit 1
fi
node -e "
const fs = require('fs');
const p = JSON.parse(fs.readFileSync('/tmp/kindleaf-assets-smoke-error.json', 'utf8'));
if (p.error?.code !== 'file_too_large') {
  console.error('expected file_too_large but got ' + p.error?.code);
  process.exit(1);
}
console.log('file_too_large=ok');
"

asset_one_json=$(upload_asset person "asset-smoke-person-1")
asset_one_id=$(echo "$asset_one_json" | json_get "if(p.data.asset_reference.kind!=='person' || p.data.asset_reference.status!=='awaiting_usage') process.exit(1); if(p.data.asset_reference.asset.storage_key) process.exit(1); console.log(p.data.asset_reference.id)")
asset_one_preview=$(echo "$asset_one_json" | json_get "if(!p.data.asset_reference.preview_url?.includes('/assets/')) process.exit(1); console.log(p.data.asset_reference.preview_url)")
curl -fsS -H "$AUTH_HEADER" "$API_BASE_URL$asset_one_preview" -o "$LOG_DIR/preview.png"
node "$ROOT_DIR/scripts/validate-png.mjs" "$LOG_DIR/preview.png" "storybook asset preview"
AUTH_HEADER="$TEACHER_AUTH_HEADER"
expect_error 403 forbidden GET "$asset_one_preview"
expect_error 403 forbidden PATCH "/api/workspaces/$workspace_id/storybook-creation-sessions/$session_id/asset-references/$asset_one_id" '{"display_name":"越权修改","usage":"name_only"}'
AUTH_HEADER="$ADMIN_AUTH_HEADER"

asset_one_dup_json=$(upload_asset person "asset-smoke-person-1")
echo "$asset_one_dup_json" | json_get "if(p.data.asset_reference.id !== '$asset_one_id') process.exit(1); console.log('asset_upload_idempotent=' + p.data.asset_reference.id);"

restored_json=$(api GET "/api/workspaces/$workspace_id/storybook-creation-sessions/$session_id")
echo "$restored_json" | json_get "const ref=p.data.asset_references.find((item)=>item.id==='$asset_one_id'); if(!ref || ref.status!=='awaiting_usage' || !ref.preview_url || ref.asset.storage_key) process.exit(1); console.log('asset_restore=' + p.data.asset_references.length);"

echo "5. school admin can manage teacher-created assets"
AUTH_HEADER="$TEACHER_AUTH_HEADER"
teacher_session_json=$(api POST "/api/workspaces/$workspace_id/storybook-creation-sessions" '{"quick_idea":"老师给班级孩子做一本带操场照片的专属绘本。","page_count":6}')
teacher_session_id=$(echo "$teacher_session_json" | json_get "if(p.data.status!=='understanding_ready') process.exit(1); console.log(p.data.id)")
old_session_id="$session_id"
session_id="$teacher_session_id"
teacher_asset_json=$(upload_asset scene "asset-smoke-teacher-scene-1")
teacher_asset_ref_id=$(echo "$teacher_asset_json" | json_get "if(p.data.asset_reference.kind!=='scene') process.exit(1); console.log(p.data.asset_reference.id)")
teacher_asset_preview=$(echo "$teacher_asset_json" | json_get "if(!p.data.asset_reference.preview_url) process.exit(1); console.log(p.data.asset_reference.preview_url)")
AUTH_HEADER="$ADMIN_AUTH_HEADER"
curl -fsS -H "$AUTH_HEADER" "$API_BASE_URL$teacher_asset_preview" -o "$LOG_DIR/admin-preview-teacher-asset.png"
node "$ROOT_DIR/scripts/validate-png.mjs" "$LOG_DIR/admin-preview-teacher-asset.png" "admin preview teacher storybook asset"
api PATCH "/api/workspaces/$workspace_id/storybook-creation-sessions/$session_id/asset-references/$teacher_asset_ref_id" '{"display_name":"操场","usage":"background_scene"}' | json_get "if(p.data.asset_reference.status!=='awaiting_reference' || p.data.asset_reference.display_name!=='操场') process.exit(1); console.log('admin_manage_teacher_asset=ok');" >/dev/null
session_id="$old_session_id"

echo "6. name_only is ready and does not need visual reference"
name_only_json=$(api PATCH "/api/workspaces/$workspace_id/storybook-creation-sessions/$session_id/asset-references/$asset_one_id" '{"display_name":"爸爸","usage":"name_only"}')
echo "$name_only_json" | json_get "if(p.data.asset_reference.status!=='ready' || p.data.asset_reference.usage!=='name_only') process.exit(1); console.log('name_only_status=' + p.data.asset_reference.status);"
expect_error 409 visual_reference_not_required POST "/api/workspaces/$workspace_id/storybook-creation-sessions/$session_id/asset-references/$asset_one_id/visual-reference:generate" '{"idempotency_key":"visual-not-needed-1"}'

echo "7. locked materials without outline placement are blocked"
unplaced_session_json=$(api POST "/api/workspaces/$workspace_id/storybook-creation-sessions" '{"quick_idea":"给乐乐做一本爸爸学习等待的故事。","page_count":6}')
unplaced_session_id=$(echo "$unplaced_session_json" | json_get "if(p.data.status!=='understanding_ready') process.exit(1); console.log(p.data.id)")
placed_material_id=$(echo "$unplaced_session_json" | json_get "const ids=p.data.materials.filter((item)=>item.locked).map((item)=>item.id); if(ids.length < 2) process.exit(1); console.log(ids[0]);")
missing_material_id=$(echo "$unplaced_session_json" | json_get "const ids=p.data.materials.filter((item)=>item.locked).map((item)=>item.id); if(ids.length < 2) process.exit(1); console.log(ids[1]);")
unplaced_directions_json=$(api POST "/api/workspaces/$workspace_id/storybook-creation-sessions/$unplaced_session_id/directions:generate" '{"direction_count":3,"refresh_reason":"initial"}')
unplaced_direction_id=$(echo "$unplaced_directions_json" | json_get "console.log(p.data.directions[0].id)")
api POST "/api/workspaces/$workspace_id/storybook-creation-sessions/$unplaced_session_id/direction" "{\"direction_id\":\"$unplaced_direction_id\"}" >/dev/null
unplaced_outline_json=$(api POST "/api/workspaces/$workspace_id/storybook-creation-sessions/$unplaced_session_id/outline:generate" '{"page_count":6}')
bad_outline_body=$(echo "$unplaced_outline_json" | node -e "
let s='';process.stdin.on('data',d=>s+=d);process.stdin.on('end',()=>{
  const p=JSON.parse(s);
  const outline=p.data.outline;
  const pages=outline.pages.map((page)=>({
    page_number: page.page_number,
    summary: page.summary,
    material_ids: ['$placed_material_id']
  }));
  console.log(JSON.stringify({summary:outline.summary,pages,review_points:outline.review_points || []}));
});
")
api PATCH "/api/workspaces/$workspace_id/storybook-creation-sessions/$unplaced_session_id/outline" "$bad_outline_body" >/dev/null
expect_error 409 material_unplaced POST "/api/workspaces/$workspace_id/storybook-creation-sessions/$unplaced_session_id/storybook:generate" '{"generation_mode":"full_draft","include_images":false,"idempotency_key":"storybook-unplaced-1"}'
node -e "const fs=require('fs'); const p=JSON.parse(fs.readFileSync('/tmp/kindleaf-assets-smoke-error.json','utf8')); if(!p.error?.details?.unplaced_material_ids?.includes('$missing_material_id')) process.exit(1); console.log('material_unplaced_details=ok');"

echo "8. second photo creates soft and hard gate"
asset_two_json=$(upload_asset object "asset-smoke-object-2")
asset_two_id=$(echo "$asset_two_json" | json_get "if(p.data.asset_reference.kind!=='object') process.exit(1); console.log(p.data.asset_reference.id)")
api PATCH "/api/workspaces/$workspace_id/storybook-creation-sessions/$session_id/asset-references/$asset_two_id" '{"display_name":"小汽车","usage":"story_object"}' >/dev/null

directions_json=$(api POST "/api/workspaces/$workspace_id/storybook-creation-sessions/$session_id/directions:generate" '{"direction_count":3,"refresh_reason":"initial"}')
direction_id=$(echo "$directions_json" | json_get "if(!Array.isArray(p.data.directions) || p.data.directions.length!==3) process.exit(1); if(!p.meta?.warnings?.some((w)=>w.code==='visual_reference_pending' && w.asset_reference_ids.includes('$asset_two_id'))) process.exit(1); console.log(p.data.directions[0].id);")
api POST "/api/workspaces/$workspace_id/storybook-creation-sessions/$session_id/direction" "{\"direction_id\":\"$direction_id\"}" >/dev/null
outline_json=$(api POST "/api/workspaces/$workspace_id/storybook-creation-sessions/$session_id/outline:generate" '{"page_count":6}')
echo "$outline_json" | json_get "if(!p.data.outline?.pages || p.data.outline.pages.length!==6) process.exit(1); if(!p.meta?.warnings?.some((w)=>w.code==='visual_reference_pending' && w.asset_reference_ids.includes('$asset_two_id'))) process.exit(1); console.log('outline_soft_gate=' + p.data.outline.pages.length);"
expect_error 409 visual_reference_required POST "/api/workspaces/$workspace_id/storybook-creation-sessions/$session_id/storybook:generate" '{"generation_mode":"full_draft","include_images":false,"idempotency_key":"storybook-blocked-1"}'
node -e "
const fs = require('fs');
const p = JSON.parse(fs.readFileSync('/tmp/kindleaf-assets-smoke-error.json', 'utf8'));
if (!p.error?.details?.blocking_asset_reference_ids?.includes('$asset_two_id')) process.exit(1);
if (p.error?.details?.next_action !== 'confirm_visual_reference') process.exit(1);
console.log('hard_gate_details=ok');
"

echo "9. revoke unblocks final generation snapshot"
api DELETE "/api/workspaces/$workspace_id/storybook-creation-sessions/$session_id/asset-references/$asset_two_id" >/dev/null
directions_after_revoke_json=$(api POST "/api/workspaces/$workspace_id/storybook-creation-sessions/$session_id/directions:generate" '{"direction_count":3,"refresh_reason":"user_clicked_refresh"}')
direction_after_revoke_id=$(echo "$directions_after_revoke_json" | json_get "if(p.meta?.warnings?.length) process.exit(1); console.log(p.data.directions[0].id);")
api POST "/api/workspaces/$workspace_id/storybook-creation-sessions/$session_id/direction" "{\"direction_id\":\"$direction_after_revoke_id\"}" >/dev/null
api POST "/api/workspaces/$workspace_id/storybook-creation-sessions/$session_id/outline:generate" '{"page_count":6}' | json_get "if(p.meta?.warnings?.length) process.exit(1); console.log('outline_after_revoke=' + p.data.outline.pages.length);" >/dev/null
generate_json=$(api POST "/api/workspaces/$workspace_id/storybook-creation-sessions/$session_id/storybook:generate" '{"generation_mode":"full_draft","include_images":false,"idempotency_key":"storybook-ready-1"}')
job_id=$(echo "$generate_json" | json_get "if(p.data.status!=='generating' || !p.data.job_id) process.exit(1); console.log(p.data.job_id)")
docker exec -i "$DB_CONTAINER" psql -U "$DB_USER" -d "$DB_NAME" -v ON_ERROR_STOP=1 -tAc "select input_json from generation_jobs where id = '$job_id'" | json_get "const refs=p.asset_references || []; if(refs.length!==1 || refs[0].id !== '$asset_one_id' || refs[0].usage !== 'name_only') process.exit(1); if(refs.some((item)=>item.id==='$asset_two_id')) process.exit(1); const pageEvidence=p.page_evidence || []; if(!pageEvidence.some((page)=>(page.asset_reference_ids||[]).includes('$asset_one_id'))) process.exit(1); console.log('generation_snapshot_asset_refs=' + refs.length);"
wait_for_generation_job_succeeded "$job_id"
docker exec -i "$DB_CONTAINER" psql -U "$DB_USER" -d "$DB_NAME" -v ON_ERROR_STOP=1 -tAc "select s.customization_plan from storybooks s join generation_jobs g on g.storybook_id = s.id where g.id = '$job_id'" | json_get "if(p.entry_type !== 'direct_create' || p.generation_job_id !== '$job_id') process.exit(1); if(!p.page_evidence?.some((page)=>(page.asset_reference_ids||[]).includes('$asset_one_id'))) process.exit(1); console.log('direct_storybook_evidence_frozen=ok');"

echo "10. visual reference can generate, preview and confirm"
visual_session_json=$(api POST "/api/workspaces/$workspace_id/storybook-creation-sessions" '{"quick_idea":"给乐乐做一本和爸爸的小汽车一起过桥的故事。","page_count":6}')
visual_session_id=$(echo "$visual_session_json" | json_get "if(p.data.status!=='understanding_ready') process.exit(1); console.log(p.data.id)")
old_session_id="$session_id"
session_id="$visual_session_id"
visual_asset_json=$(upload_asset object "asset-smoke-visual-object-1")
visual_asset_ref_id=$(echo "$visual_asset_json" | json_get "if(p.data.asset_reference.status!=='awaiting_usage') process.exit(1); console.log(p.data.asset_reference.id)")
api PATCH "/api/workspaces/$workspace_id/storybook-creation-sessions/$session_id/asset-references/$visual_asset_ref_id" '{"display_name":"小汽车","usage":"story_object"}' | json_get "if(p.data.asset_reference.status!=='awaiting_reference') process.exit(1); console.log('visual_usage_status=' + p.data.asset_reference.status);" >/dev/null
visual_generate_json=$(api POST "/api/workspaces/$workspace_id/storybook-creation-sessions/$session_id/asset-references/$visual_asset_ref_id/visual-reference:generate" '{"idempotency_key":"visual-success-1"}')
visual_reference_id=$(echo "$visual_generate_json" | json_get "if(!p.data.visual_reference?.id || p.data.next_action!=='poll_visual_reference') process.exit(1); if(!['queued','generating','awaiting_confirmation'].includes(p.data.visual_reference.status)) process.exit(1); console.log(p.data.visual_reference.id);")
visual_generate_dup_json=$(api POST "/api/workspaces/$workspace_id/storybook-creation-sessions/$session_id/asset-references/$visual_asset_ref_id/visual-reference:generate" '{"idempotency_key":"visual-success-1"}')
echo "$visual_generate_dup_json" | json_get "if(p.data.visual_reference?.id !== '$visual_reference_id') process.exit(1); console.log('visual_generation_idempotent=' + p.data.visual_reference.id);"
visual_ready_json=$(wait_for_asset_reference_status "$session_id" "$visual_asset_ref_id" "awaiting_confirmation")
echo "$visual_ready_json" | json_get "const ref=p.data.asset_references.find((item)=>item.id==='$visual_asset_ref_id'); if(ref.visual_reference.id!=='$visual_reference_id' || ref.visual_reference.status!=='awaiting_confirmation' || !ref.visual_reference.preview_url) process.exit(1); console.log('visual_first_awaiting_confirmation=' + ref.visual_reference.id);"
visual_replace_json=$(api POST "/api/workspaces/$workspace_id/storybook-creation-sessions/$session_id/asset-references/$visual_asset_ref_id/visual-reference:generate" '{"idempotency_key":"visual-success-2"}')
visual_replacement_id=$(echo "$visual_replace_json" | json_get "if(!p.data.visual_reference?.id || p.data.visual_reference.id === '$visual_reference_id') process.exit(1); console.log(p.data.visual_reference.id);")
docker exec -i "$DB_CONTAINER" psql -U "$DB_USER" -d "$DB_NAME" -v ON_ERROR_STOP=1 -tAc "select json_build_object('status', status, 'is_active', is_active) from storybook_visual_references where id = '$visual_reference_id'" | json_get "if(p.status !== 'rejected' || p.is_active !== false) process.exit(1); console.log('visual_old_rejected=ok');"
expect_error 409 idempotency_key_replaced POST "/api/workspaces/$workspace_id/storybook-creation-sessions/$session_id/asset-references/$visual_asset_ref_id/visual-reference:generate" '{"idempotency_key":"visual-success-1"}'
visual_ready_json=$(wait_for_asset_reference_status "$session_id" "$visual_asset_ref_id" "awaiting_confirmation")
visual_preview_url=$(echo "$visual_ready_json" | json_get "const ref=p.data.asset_references.find((item)=>item.id==='$visual_asset_ref_id'); if(ref.visual_reference.id!=='$visual_replacement_id' || ref.visual_reference.status!=='awaiting_confirmation' || !ref.visual_reference.preview_url) process.exit(1); console.log(ref.visual_reference.preview_url);")
AUTH_HEADER="$TEACHER_AUTH_HEADER"
expect_error 403 forbidden GET "$visual_preview_url"
AUTH_HEADER="$ADMIN_AUTH_HEADER"
curl -fsS -H "$AUTH_HEADER" "$API_BASE_URL$visual_preview_url" -o "$LOG_DIR/visual-reference.png"
node "$ROOT_DIR/scripts/validate-png.mjs" "$LOG_DIR/visual-reference.png" "storybook visual reference preview"
api POST "/api/workspaces/$workspace_id/storybook-creation-sessions/$session_id/visual-references/$visual_replacement_id/confirm" '{}' | json_get "const ref=p.data.asset_reference; if(ref.status!=='ready' || ref.visual_reference.status!=='confirmed' || ref.visual_reference.id !== '$visual_replacement_id') process.exit(1); console.log('visual_confirmed=' + ref.visual_reference.id);" >/dev/null
visual_directions_json=$(api POST "/api/workspaces/$workspace_id/storybook-creation-sessions/$session_id/directions:generate" '{"direction_count":3,"refresh_reason":"initial"}')
visual_direction_id=$(echo "$visual_directions_json" | json_get "if(p.meta?.warnings?.length) process.exit(1); if(!Array.isArray(p.data.directions) || p.data.directions.length!==3) process.exit(1); console.log(p.data.directions[0].id);")
api POST "/api/workspaces/$workspace_id/storybook-creation-sessions/$session_id/direction" "{\"direction_id\":\"$visual_direction_id\"}" >/dev/null
api POST "/api/workspaces/$workspace_id/storybook-creation-sessions/$session_id/outline:generate" '{"page_count":6}' | json_get "if(p.meta?.warnings?.length) process.exit(1); if(p.data.outline.pages.length!==6) process.exit(1); console.log('visual_outline=' + p.data.outline.pages.length);" >/dev/null
visual_generate_storybook_json=$(api POST "/api/workspaces/$workspace_id/storybook-creation-sessions/$session_id/storybook:generate" '{"generation_mode":"full_draft","include_images":false,"idempotency_key":"storybook-visual-ready-1"}')
visual_job_id=$(echo "$visual_generate_storybook_json" | json_get "if(p.data.status!=='generating' || !p.data.job_id) process.exit(1); console.log(p.data.job_id)")
visual_generate_storybook_dup_json=$(api POST "/api/workspaces/$workspace_id/storybook-creation-sessions/$session_id/storybook:generate" '{"generation_mode":"full_draft","include_images":false,"idempotency_key":"storybook-visual-ready-1"}')
echo "$visual_generate_storybook_dup_json" | json_get "if(p.data.job_id !== '$visual_job_id' || !['generating','storybook_ready'].includes(p.data.status)) process.exit(1); console.log('direct_storybook_generation_idempotent=' + p.data.job_id);" >/dev/null
visual_generate_storybook_second_click_json=$(api POST "/api/workspaces/$workspace_id/storybook-creation-sessions/$session_id/storybook:generate" '{"generation_mode":"full_draft","include_images":false,"idempotency_key":"storybook-visual-ready-second-click"}')
echo "$visual_generate_storybook_second_click_json" | json_get "if(p.data.job_id !== '$visual_job_id' || !['generating','storybook_ready'].includes(p.data.status)) process.exit(1); console.log('direct_storybook_generation_second_click_reused=' + p.data.job_id);" >/dev/null
docker exec -i "$DB_CONTAINER" psql -U "$DB_USER" -d "$DB_NAME" -v ON_ERROR_STOP=1 -tAc "select input_json from generation_jobs where id = '$visual_job_id'" | json_get "const refs=p.asset_references || []; if(refs.length!==1 || refs[0].id !== '$visual_asset_ref_id' || refs[0].usage !== 'story_object') process.exit(1); if(!refs[0].visual_reference || refs[0].visual_reference.id !== '$visual_replacement_id' || !refs[0].visual_reference.confirmed_at) process.exit(1); const pageEvidence=p.page_evidence || []; const pageRef=pageEvidence.flatMap((page)=>page.asset_references||[]).find((ref)=>ref.asset_reference_id==='$visual_asset_ref_id'); if(!pageRef || pageRef.visual_reference_id !== '$visual_replacement_id') process.exit(1); console.log('visual_snapshot_confirmed=' + refs[0].visual_reference.id);"
wait_for_generation_job_succeeded "$visual_job_id"
docker exec -i "$DB_CONTAINER" psql -U "$DB_USER" -d "$DB_NAME" -v ON_ERROR_STOP=1 -tAc "select s.customization_plan from storybooks s join generation_jobs g on g.storybook_id = s.id where g.id = '$visual_job_id'" | json_get "if(p.entry_type !== 'direct_create' || p.generation_job_id !== '$visual_job_id') process.exit(1); const pageRef=(p.page_evidence || []).flatMap((page)=>page.asset_references||[]).find((ref)=>ref.asset_reference_id==='$visual_asset_ref_id'); if(!pageRef || pageRef.visual_reference_id !== '$visual_replacement_id') process.exit(1); console.log('direct_storybook_visual_evidence_frozen=ok');"
visual_storybook_id=$(docker exec -i "$DB_CONTAINER" psql -U "$DB_USER" -d "$DB_NAME" -v ON_ERROR_STOP=1 -tAc "select storybook_id from generation_jobs where id = '$visual_job_id'")
docker exec -i "$DB_CONTAINER" psql -U "$DB_USER" -d "$DB_NAME" -v ON_ERROR_STOP=1 -c "update storybooks set status = 'exportable', teacher_review_status = 'confirmed', teacher_reviewed_by = '$admin_id', teacher_reviewed_at = now() where id = '$visual_storybook_id'" >/dev/null
docker exec -i "$DB_CONTAINER" psql -U "$DB_USER" -d "$DB_NAME" -v ON_ERROR_STOP=1 -c "update storybook_pages set status = 'ready' where storybook_id = '$visual_storybook_id'" >/dev/null
api POST "/api/workspaces/$workspace_id/storybooks/$visual_storybook_id/exports" '{}' | json_get "if(p.data.status!=='queued' || !p.data.id) process.exit(1); console.log('direct_storybook_export_queued=ok');" >/dev/null
direct_share_token=$(api POST "/api/workspaces/$workspace_id/storybooks/$visual_storybook_id/share-links" '{}' | json_get "if(!p.data.token || !p.data.url) process.exit(1); console.log(p.data.token);")
echo "direct_storybook_share_link_created=ok"
docker exec -i "$DB_CONTAINER" psql -U "$DB_USER" -d "$DB_NAME" -v ON_ERROR_STOP=1 -c "update storybooks set customization_plan = customization_plan - 'outline' where id = '$visual_storybook_id'" >/dev/null
expect_error 409 direct_creation_evidence_missing POST "/api/workspaces/$workspace_id/storybooks/$visual_storybook_id/exports" '{}'
expect_error 409 direct_creation_evidence_missing POST "/api/workspaces/$workspace_id/storybooks/$visual_storybook_id/share-links" '{}'
public_export_status=$(curl -sS -o /tmp/kindleaf-assets-smoke-error.json -w "%{http_code}" -H "Content-Type: application/json" -X POST "$API_BASE_URL/api/share-links/$direct_share_token/exports")
if [[ "$public_export_status" != "409" ]]; then
  echo "expected HTTP 409 for public export without evidence but got $public_export_status" >&2
  cat /tmp/kindleaf-assets-smoke-error.json >&2 || true
  exit 1
fi
node -e "const fs=require('fs'); const p=JSON.parse(fs.readFileSync('/tmp/kindleaf-assets-smoke-error.json','utf8')); if(p.error?.code!=='direct_creation_evidence_missing') process.exit(1); const details=p.error?.details || {}; if(!details.missing?.includes('outline')) process.exit(1); console.log('error_direct_creation_evidence_missing_public=ok');"

plain_storybook_json=$(api POST "/api/workspaces/$workspace_id/storybooks" '{"title":"Assets smoke 普通绘本导出不受专属证据门禁影响","age_group":"4-5 岁","use_scene":"家庭共读","teaching_goal":"验证普通绘本导出不受专属 evidence gate 影响","cover_tone":"温暖水彩"}')
plain_storybook_id=$(echo "$plain_storybook_json" | json_get "if(!p.data.id || p.data.source !== 'blank') process.exit(1); console.log(p.data.id);")
plain_roles_job_json=$(api POST "/api/workspaces/$workspace_id/generation-jobs" "{\"job_type\":\"storybook_roles\",\"storybook_id\":\"$plain_storybook_id\",\"input_json\":{\"title\":\"Assets smoke 普通绘本导出不受专属证据门禁影响\"}}")
plain_roles_job_id=$(echo "$plain_roles_job_json" | json_get "if(!p.data.id) process.exit(1); console.log(p.data.id);")
wait_for_generation_job_succeeded "$plain_roles_job_id"
plain_pages_job_json=$(api POST "/api/workspaces/$workspace_id/generation-jobs" "{\"job_type\":\"storybook_pages\",\"storybook_id\":\"$plain_storybook_id\",\"input_json\":{\"page_count\":2}}")
plain_pages_job_id=$(echo "$plain_pages_job_json" | json_get "if(!p.data.id) process.exit(1); console.log(p.data.id);")
wait_for_generation_job_succeeded "$plain_pages_job_id"
docker exec -i "$DB_CONTAINER" psql -U "$DB_USER" -d "$DB_NAME" -v ON_ERROR_STOP=1 -c "update storybooks set source = 'blank', status = 'exportable', visibility = 'workspace', teacher_review_status = 'confirmed', teacher_reviewed_by = '$admin_id', teacher_reviewed_at = now(), customization_plan = null where id = '$plain_storybook_id'" >/dev/null
docker exec -i "$DB_CONTAINER" psql -U "$DB_USER" -d "$DB_NAME" -v ON_ERROR_STOP=1 -c "update storybook_pages set status = 'ready' where storybook_id = '$plain_storybook_id'" >/dev/null
api POST "/api/workspaces/$workspace_id/storybooks/$plain_storybook_id/exports" '{}' | json_get "if(p.data.status!=='queued' || !p.data.id) process.exit(1); console.log('plain_storybook_export_not_blocked_by_evidence_gate=ok');" >/dev/null
session_id="$old_session_id"

echo "11. failed visual reference blocks and can retry"
failed_session_json=$(api POST "/api/workspaces/$workspace_id/storybook-creation-sessions" '{"quick_idea":"给乐乐做一本小飞机学会排队的故事。","page_count":6}')
failed_session_id=$(echo "$failed_session_json" | json_get "if(p.data.status!=='understanding_ready') process.exit(1); console.log(p.data.id)")
old_session_id="$session_id"
session_id="$failed_session_id"
failed_asset_json=$(upload_asset object "asset-smoke-failed-object-1")
failed_asset_ref_id=$(echo "$failed_asset_json" | json_get "if(p.data.asset_reference.status!=='awaiting_usage') process.exit(1); console.log(p.data.asset_reference.id)")
api PATCH "/api/workspaces/$workspace_id/storybook-creation-sessions/$session_id/asset-references/$failed_asset_ref_id" '{"display_name":"小飞机","usage":"story_object"}' >/dev/null
failed_visual_json=$(api POST "/api/workspaces/$workspace_id/storybook-creation-sessions/$session_id/asset-references/$failed_asset_ref_id/visual-reference:generate" '{"idempotency_key":"visual-failed-1"}')
failed_visual_id=$(echo "$failed_visual_json" | json_get "if(!p.data.visual_reference?.id) process.exit(1); console.log(p.data.visual_reference.id);")
docker exec -i "$DB_CONTAINER" psql -U "$DB_USER" -d "$DB_NAME" -v ON_ERROR_STOP=1 >/dev/null <<SQL
update storybook_visual_references
set status = 'failed',
    failure_reason = 'smoke simulated provider failure',
    image_storage_key = null,
    updated_at = now()
where id = '$failed_visual_id';
update storybook_asset_references
set status = 'failed',
    updated_at = now()
where id = '$failed_asset_ref_id';
SQL
failed_restore_json=$(api GET "/api/workspaces/$workspace_id/storybook-creation-sessions/$session_id")
echo "$failed_restore_json" | json_get "const ref=p.data.asset_references.find((item)=>item.id==='$failed_asset_ref_id'); if(ref.status!=='failed' || ref.visual_reference.status!=='failed' || ref.visual_reference.failure_reason!=='smoke simulated provider failure') process.exit(1); console.log('visual_failed_restore=ok');"
expect_error 409 visual_reference_not_ready POST "/api/workspaces/$workspace_id/storybook-creation-sessions/$session_id/visual-references/$failed_visual_id/confirm" '{}'
failed_directions_json=$(api POST "/api/workspaces/$workspace_id/storybook-creation-sessions/$session_id/directions:generate" '{"direction_count":3,"refresh_reason":"initial"}')
failed_direction_id=$(echo "$failed_directions_json" | json_get "if(!p.meta?.warnings?.some((w)=>w.code==='visual_reference_pending' && w.asset_reference_ids.includes('$failed_asset_ref_id'))) process.exit(1); console.log(p.data.directions[0].id);")
api POST "/api/workspaces/$workspace_id/storybook-creation-sessions/$session_id/direction" "{\"direction_id\":\"$failed_direction_id\"}" >/dev/null
api POST "/api/workspaces/$workspace_id/storybook-creation-sessions/$session_id/outline:generate" '{"page_count":6}' | json_get "if(!p.meta?.warnings?.some((w)=>w.code==='visual_reference_pending' && w.asset_reference_ids.includes('$failed_asset_ref_id'))) process.exit(1); console.log('visual_failed_outline_warning=ok');" >/dev/null
expect_error 409 visual_reference_required POST "/api/workspaces/$workspace_id/storybook-creation-sessions/$session_id/storybook:generate" '{"generation_mode":"full_draft","include_images":false,"idempotency_key":"storybook-failed-visual-blocked-1"}'
failed_retry_json=$(api POST "/api/workspaces/$workspace_id/storybook-creation-sessions/$session_id/asset-references/$failed_asset_ref_id/visual-reference:generate" '{"idempotency_key":"visual-failed-retry-1"}')
failed_retry_visual_id=$(echo "$failed_retry_json" | json_get "if(!p.data.visual_reference?.id || p.data.visual_reference.id === '$failed_visual_id') process.exit(1); console.log(p.data.visual_reference.id);")
docker exec -i "$DB_CONTAINER" psql -U "$DB_USER" -d "$DB_NAME" -v ON_ERROR_STOP=1 -tAc "select json_build_object('status', status, 'is_active', is_active) from storybook_visual_references where id = '$failed_visual_id'" | json_get "if(p.status !== 'failed' || p.is_active !== false) process.exit(1); console.log('visual_failed_replaced=ok');"
failed_retry_ready_json=$(wait_for_asset_reference_status "$session_id" "$failed_asset_ref_id" "awaiting_confirmation")
echo "$failed_retry_ready_json" | json_get "const ref=p.data.asset_references.find((item)=>item.id==='$failed_asset_ref_id'); if(ref.visual_reference.id !== '$failed_retry_visual_id' || ref.visual_reference.status !== 'awaiting_confirmation') process.exit(1); console.log('visual_failed_retry_ready=' + ref.visual_reference.id);"
api POST "/api/workspaces/$workspace_id/storybook-creation-sessions/$session_id/visual-references/$failed_retry_visual_id/confirm" '{}' | json_get "const ref=p.data.asset_reference; if(ref.status!=='ready' || ref.visual_reference.id !== '$failed_retry_visual_id') process.exit(1); console.log('visual_failed_retry_confirmed=' + ref.visual_reference.id);" >/dev/null
session_id="$old_session_id"

echo "12. revoke remains allowed while session is generating"
generating_revoke_session_json=$(api POST "/api/workspaces/$workspace_id/storybook-creation-sessions" '{"quick_idea":"验证制作中也可以撤销照片引用。","page_count":6}')
generating_revoke_session_id=$(echo "$generating_revoke_session_json" | json_get "console.log(p.data.id)")
old_session_id="$session_id"
session_id="$generating_revoke_session_id"
generating_revoke_asset_json=$(upload_asset person "asset-smoke-generating-revoke")
generating_revoke_ref_id=$(echo "$generating_revoke_asset_json" | json_get "console.log(p.data.asset_reference.id)")
docker exec -i "$DB_CONTAINER" psql -U "$DB_USER" -d "$DB_NAME" -v ON_ERROR_STOP=1 -tAc "update storybook_creation_sessions set status = 'generating', updated_at = now() where id = '$generating_revoke_session_id'"
api DELETE "/api/workspaces/$workspace_id/storybook-creation-sessions/$generating_revoke_session_id/asset-references/$generating_revoke_ref_id" | json_get "if(p.data.status !== 'revoked') process.exit(1); console.log('generating_revoke_status=' + p.data.status);"
docker exec -i "$DB_CONTAINER" psql -U "$DB_USER" -d "$DB_NAME" -v ON_ERROR_STOP=1 -tAc "select json_build_object('session_status', s.status, 'reference_status', r.status) from storybook_creation_sessions s join storybook_asset_references r on r.creation_session_id = s.id where s.id = '$generating_revoke_session_id' and r.id = '$generating_revoke_ref_id'" | json_get "if(p.session_status !== 'generating' || p.reference_status !== 'revoked') process.exit(1); console.log('generating_revoke_preserves_session=ok');"
session_id="$old_session_id"

echo "13. five effective photos limit"
limit_session_json=$(api POST "/api/workspaces/$workspace_id/storybook-creation-sessions" '{"quick_idea":"验证照片上限的专属绘本故事。","page_count":6}')
limit_session_id=$(echo "$limit_session_json" | json_get "console.log(p.data.id)")
old_session_id="$session_id"
session_id="$limit_session_id"
limit_first_ref_id=""
for index in 1 2 3 4 5; do
  limit_asset_json=$(upload_asset scene "asset-smoke-limit-$index")
  if [[ "$index" == "1" ]]; then
    limit_first_ref_id=$(echo "$limit_asset_json" | json_get "console.log(p.data.asset_reference.id)")
  fi
done
limit_status=$(curl -sS -o /tmp/kindleaf-assets-smoke-error.json -w "%{http_code}" -H "$AUTH_HEADER" \
  -F "file=@$PNG_FILE;filename=limit-6.png;type=image/png" \
  -F "kind=scene" \
  -F "idempotency_key=asset-smoke-limit-6" \
  "$API_BASE_URL/api/workspaces/$workspace_id/storybook-creation-sessions/$session_id/assets")
if [[ "$limit_status" != "400" ]]; then
  echo "expected HTTP 400 for sixth photo but got $limit_status" >&2
  cat /tmp/kindleaf-assets-smoke-error.json >&2 || true
  exit 1
fi
node -e "
const fs = require('fs');
const p = JSON.parse(fs.readFileSync('/tmp/kindleaf-assets-smoke-error.json', 'utf8'));
if (p.error?.code !== 'photo_limit_exceeded') {
  console.error('expected photo_limit_exceeded but got ' + p.error?.code);
  process.exit(1);
}
console.log('photo_limit_exceeded=ok');
"
api PATCH "/api/workspaces/$workspace_id/storybook-creation-sessions/$session_id/asset-references/$limit_first_ref_id" '{"display_name":"不用的场景","usage":"unused"}' | json_get "if(p.data.asset_reference.status!=='unused' || p.data.remaining_slots !== 1) process.exit(1); console.log('unused_releases_slot=ok');"
upload_asset scene "asset-smoke-limit-replacement" | json_get "if(p.data.remaining_slots !== 0) process.exit(1); console.log('unused_replacement_uploaded=ok');"
docker exec -i "$DB_CONTAINER" psql -U "$DB_USER" -d "$DB_NAME" -v ON_ERROR_STOP=1 -tAc "select count(*) from storybook_asset_references where creation_session_id = '$session_id' and status not in ('unused', 'revoked')" | grep -qx "5"
session_id="$old_session_id"

echo "== storybook creation assets smoke ok =="
