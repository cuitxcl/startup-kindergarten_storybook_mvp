#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
BASE="${API_BASE_URL:-http://127.0.0.1:8080}"
DB_CONTAINER="${DB_CONTAINER:-kindleaf-postgres}"
DB_NAME="${DB_NAME:-kindleaf_development}"
AUTH_HEADER="Authorization: Bearer ${API_TOKEN:-dev-token}"
PERSONAL_WS="10000000-0000-0000-0000-000000000001"
SCHOOL_WS=""
TEACHER_WS=""
PERSONAL_ROLE="personal_owner"
SCHOOL_ADMIN_WS=""
SCHOOL_TEACHER_WS=""
SCHOOL_ROLE="school_teacher"

created_child_id=""
archived_child_id=""
teacher_child_id=""
confirmed_intake_child_id=""
created_plain_id=""
risky_storybook_id=""
duplicated_storybook_id=""
created_custom_id=""
copied_book_id=""
approved_copied_book_id=""
share_link_id=""
expired_share_link_id=""
expired_share_token=""
export_job_id=""
public_export_id=""
export_file_url=""
public_export_file_url=""
image_job_id=""
image_file_url=""
plan_job_id=""
roles_job_id=""
pages_job_id=""
teacher_plan_job_id=""
applied_roles_job_id=""
applied_pages_job_id=""
customization_plan_job_id=""
retry_job_id=""
scoped_recover_job_id=""
foreign_recover_job_id=""
member_id=""
member_email=""
revoked_member_id=""
revoked_member_email=""
classroom_id=""
submission_id=""
risky_submission_id=""
rejected_submission_id=""
approved_template_id=""
intake_nickname=""
intake_id=""
intake_link_id=""
intake_link_token=""
revoked_intake_link_id=""
revoked_intake_link_token=""
expired_intake_link_id=""
expired_intake_link_token=""
registered_user_id=""
registered_email=""
registered_workspace_id=""
registered_storybook_id=""

json_get() {
  local script="$1"
  node -e "let s='';process.stdin.on('data',d=>s+=d);process.stdin.on('end',()=>{const p=JSON.parse(s); ${script}});"
}

wait_for_job_status() {
  local job_id="$1"
  local expected="$2"
  local attempts="${3:-60}"
  for _ in $(seq 1 "$attempts"); do
    local status
    status=$(api GET "/api/workspaces/$SCHOOL_WS/generation-jobs/$job_id" | json_get "console.log(p.data.status)")
    if [[ "$status" == "$expected" ]]; then
      return 0
    fi
    sleep 0.25
  done
  echo "job $job_id did not reach status $expected" >&2
  api GET "/api/workspaces/$SCHOOL_WS/generation-jobs/$job_id" >&2 || true
  exit 1
}

wait_for_export_status() {
  local storybook_id="$1"
  local export_id="$2"
  local expected="$3"
  local attempts="${4:-60}"
  for _ in $(seq 1 "$attempts"); do
    local status
    status=$(api GET "/api/workspaces/$SCHOOL_WS/storybooks/$storybook_id/exports/$export_id" | json_get "console.log(p.data.status)")
    if [[ "$status" == "$expected" ]]; then
      return 0
    fi
    sleep 0.25
  done
  echo "export $export_id did not reach status $expected" >&2
  api GET "/api/workspaces/$SCHOOL_WS/storybooks/$storybook_id/exports/$export_id" >&2 || true
  exit 1
}

wait_for_public_export_status() {
  local token="$1"
  local export_id="$2"
  local expected="$3"
  local attempts="${4:-60}"
  for _ in $(seq 1 "$attempts"); do
    local status
    status=$(curl -fsS "$BASE/api/share-links/$token/exports/$export_id" | json_get "console.log(p.data.status)")
    if [[ "$status" == "$expected" ]]; then
      return 0
    fi
    sleep 0.25
  done
  echo "public export $export_id did not reach status $expected" >&2
  curl -fsS "$BASE/api/share-links/$token/exports/$export_id" >&2 || true
  exit 1
}

expect_pdf_download() {
  local file_url="$1"
  local label="$2"
  local auth="${3:-public}"
  local expected_text="${4:-}"
  local expected_extra_text="${5:-}"
  local target="/tmp/kindleaf-smoke-${label}.pdf"
  if [[ "$auth" == "auth" ]]; then
    curl -fsS -H "$AUTH_HEADER" "$BASE$file_url" -o "$target"
  else
    curl -fsS "$BASE$file_url" -o "$target"
  fi
  if [[ "$(head -c 4 "$target")" != "%PDF" ]]; then
    echo "expected PDF content for $label at $file_url" >&2
    exit 1
  fi
  if [[ -n "$expected_text" ]]; then
    node -e "
const fs = require('fs');
const text = fs.readFileSync('$target', 'latin1');
if (!text.includes('$expected_text')) {
  console.error('expected PDF text fragment $expected_text for $label at $file_url');
  process.exit(1);
}
"
  fi
  if [[ -n "$expected_extra_text" ]]; then
    node -e "
const fs = require('fs');
const text = fs.readFileSync('$target', 'latin1');
if (!text.includes('$expected_extra_text')) {
  console.error('expected PDF text fragment $expected_extra_text for $label at $file_url');
  process.exit(1);
}
"
  fi
  rm -f "$target"
  echo "${label}_pdf=ok"
}

expect_png_download() {
  local file_url="$1"
  local label="$2"
  local auth="${3:-public}"
  local target="/tmp/kindleaf-smoke-${label}.png"
  if [[ "$auth" == "auth" ]]; then
    curl -fsS -H "$AUTH_HEADER" "$BASE$file_url" -o "$target"
  else
    curl -fsS "$BASE$file_url" -o "$target"
  fi
  "$ROOT_DIR/scripts/validate-png.mjs" "$target" "$label at $file_url"
  rm -f "$target"
  echo "${label}_png=ok"
}

api() {
  local method="$1"
  local path="$2"
  local body="${3:-}"
  if [[ -n "$body" ]]; then
    curl -fsS -H "$AUTH_HEADER" -H "Content-Type: application/json" -X "$method" "$BASE$path" -d "$body"
  else
    curl -fsS -H "$AUTH_HEADER" -X "$method" "$BASE$path"
  fi
}

expect_status() {
  local expected="$1"
  local method="$2"
  local path="$3"
  local auth="${4:-auth}"
  local body="${5:-}"
  local args=(-sS -o /tmp/kindleaf-smoke-response.txt -w "%{http_code}" -X "$method" "$BASE$path")
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
    cat /tmp/kindleaf-smoke-response.txt >&2 || true
    exit 1
  fi
}

expect_error() {
  local expected_status="$1"
  local expected_code="$2"
  local expected_field="$3"
  local method="$4"
  local path="$5"
  local auth="${6:-auth}"
  local body="${7:-}"
  expect_status "$expected_status" "$method" "$path" "$auth" "$body"
  node -e "
const fs = require('fs');
const payload = JSON.parse(fs.readFileSync('/tmp/kindleaf-smoke-response.txt', 'utf8'));
if (payload.error?.code !== '$expected_code') {
  console.error('expected error code $expected_code but got ' + payload.error?.code);
  process.exit(1);
}
const actualField = payload.error?.field ?? null;
const expectedField = '$expected_field' === '-' ? null : '$expected_field';
if (actualField !== expectedField) {
  console.error('expected error field ' + expectedField + ' but got ' + actualField);
  process.exit(1);
}
" 
  rm -f /tmp/kindleaf-smoke-response.txt
}

cleanup() {
  docker exec -i "$DB_CONTAINER" psql -U postgres -d "$DB_NAME" -v ON_ERROR_STOP=1 >/dev/null <<SQL || true
delete from audit_logs
where resource_id in (
  select value::uuid from jsonb_array_elements_text(
    jsonb_build_array('$member_id', '$revoked_member_id', '$classroom_id', '$export_job_id', '$public_export_id', '$share_link_id', '$expired_share_link_id', '$submission_id', '$risky_submission_id', '$approved_template_id', '$intake_link_id', '$revoked_intake_link_id', '$expired_intake_link_id')
  ) as value where value <> ''
)
   or (action = 'parent_intake.submitted' and metadata_json->>'child_nickname' = nullif('$intake_nickname', ''))
   or (action = 'generation_job.recovered' and workspace_id = nullif('$SCHOOL_WS', '')::uuid and metadata_json->>'limit' = '5')
   or actor_user_id = nullif('$registered_user_id', '')::uuid
   or workspace_id = nullif('$registered_workspace_id', '')::uuid;
delete from generation_jobs where id = nullif('$image_job_id', '')::uuid;
delete from generation_jobs where id in (
  select value::uuid from jsonb_array_elements_text(
    jsonb_build_array('$plan_job_id', '$roles_job_id', '$pages_job_id', '$teacher_plan_job_id', '$applied_roles_job_id', '$applied_pages_job_id', '$customization_plan_job_id', '$retry_job_id', '$scoped_recover_job_id', '$foreign_recover_job_id')
  ) as value where value <> ''
);
delete from export_jobs where id in (
  select value::uuid from jsonb_array_elements_text(
    jsonb_build_array('$export_job_id', '$public_export_id')
  ) as value where value <> ''
);
delete from share_links where id in (
  select value::uuid from jsonb_array_elements_text(
    jsonb_build_array('$share_link_id', '$expired_share_link_id')
  ) as value where value <> ''
);
delete from marketplace_templates where id = nullif('$approved_template_id', '')::uuid;
delete from marketplace_submissions where id in (
  select value::uuid from jsonb_array_elements_text(
    jsonb_build_array('$submission_id', '$risky_submission_id', '$rejected_submission_id')
  ) as value where value <> ''
);
delete from storybook_roles where storybook_id in (
  select value::uuid from jsonb_array_elements_text(
    jsonb_build_array('$created_custom_id', '$duplicated_storybook_id', '$created_plain_id', '$risky_storybook_id', '$copied_book_id', '$approved_copied_book_id')
  ) as value where value <> ''
);
delete from storybook_pages where storybook_id in (
  select value::uuid from jsonb_array_elements_text(
    jsonb_build_array('$created_custom_id', '$duplicated_storybook_id', '$created_plain_id', '$risky_storybook_id', '$copied_book_id', '$approved_copied_book_id')
  ) as value where value <> ''
);
delete from storybooks where id in (
  select value::uuid from jsonb_array_elements_text(
    jsonb_build_array('$created_custom_id', '$duplicated_storybook_id', '$created_plain_id', '$risky_storybook_id', '$copied_book_id', '$approved_copied_book_id')
  ) as value where value <> ''
);
delete from children where id in (
  select value::uuid from jsonb_array_elements_text(
    jsonb_build_array('$created_child_id', '$archived_child_id', '$teacher_child_id', '$confirmed_intake_child_id')
  ) as value where value <> ''
);
delete from workspace_members where id in (
  select value::uuid from jsonb_array_elements_text(
    jsonb_build_array('$member_id', '$revoked_member_id')
  ) as value where value <> ''
);
delete from users where email in (nullif('$member_email', ''), nullif('$revoked_member_email', ''));
delete from classrooms where id = nullif('$classroom_id', '')::uuid;
delete from parent_intakes where child_nickname = nullif('$intake_nickname', '');
delete from parent_intake_links
where id in (
  select value::uuid from jsonb_array_elements_text(
    jsonb_build_array('$intake_link_id', '$revoked_intake_link_id', '$expired_intake_link_id')
  ) as value where value <> ''
)
   or token in (nullif('$intake_link_token', ''), nullif('$revoked_intake_link_token', ''), nullif('$expired_intake_link_token', ''));
delete from storybook_roles where storybook_id = nullif('$registered_storybook_id', '')::uuid;
delete from storybook_pages where storybook_id = nullif('$registered_storybook_id', '')::uuid;
delete from storybooks where id = nullif('$registered_storybook_id', '')::uuid;
delete from workspace_members where workspace_id = nullif('$registered_workspace_id', '')::uuid;
delete from workspaces where id = nullif('$registered_workspace_id', '')::uuid;
delete from users where id = nullif('$registered_user_id', '')::uuid or email = nullif('$registered_email', '');
SQL
  if [[ -n "$export_file_url" ]]; then
    rm -f "tmp/exports/${export_job_id}.pdf"
  fi
  if [[ -n "$public_export_file_url" ]]; then
    rm -f "tmp/exports/${public_export_id}.pdf"
  fi
  if [[ -n "$image_file_url" ]]; then
    rm -f "tmp/generated-images/mock-${image_job_id}.png"
  fi
}

trap 'status=$?; cleanup; exit $status' EXIT

echo "== Kindleaf API smoke =="
echo "BASE=$BASE"

echo "1. health"
curl -fsS "$BASE/api/health" | json_get "if(p.data.status!=='ok') process.exit(1); console.log(p.data.service);"
expect_error 403 forbidden - GET "/api/operator/submissions"

echo "2. auth/me and workspaces"
expect_error 401 unauthorized - POST "/api/auth/login" public '{"identifier":"lin@example.com","password":"wrong-password"}'
login_json=$(curl -fsS -H "Content-Type: application/json" -X POST "$BASE/api/auth/login" -d '{"identifier":"lin@example.com","password":"demo"}')
API_TOKEN=$(echo "$login_json" | json_get "if(!p.data.token) process.exit(1); console.log(p.data.token)")
AUTH_HEADER="Authorization: Bearer $API_TOKEN"
auth_me_json=$(api GET "/api/auth/me")
echo "$auth_me_json" | json_get "
if(!p.data.workspaces?.length) process.exit(1);
const personal = p.data.workspaces.find((item)=>item.type==='personal');
const school = p.data.workspaces.find((item)=>item.type==='school');
const operator = p.data.workspaces.find((item)=>item.type==='platform' && item.role==='platform_operator');
if(!personal || !school || !operator) process.exit(1);
console.log(p.data.user.display_name + ' / ' + p.data.workspaces.length + ' workspaces');
"
PERSONAL_WS=$(echo "$auth_me_json" | json_get "const personal=p.data.workspaces.find((item)=>item.type==='personal'); if(!personal) process.exit(1); console.log(personal.id)")
SCHOOL_ADMIN_WS=$(echo "$auth_me_json" | json_get "const school=p.data.workspaces.find((item)=>item.type==='school' && item.role==='school_admin'); if(school) console.log(school.id)")
SCHOOL_TEACHER_WS=$(echo "$auth_me_json" | json_get "const school=p.data.workspaces.find((item)=>item.type==='school' && item.role==='school_teacher'); if(school) console.log(school.id)")
if [[ -n "$SCHOOL_ADMIN_WS" ]]; then
  SCHOOL_WS="$SCHOOL_ADMIN_WS"
elif [[ -n "$SCHOOL_TEACHER_WS" ]]; then
  SCHOOL_WS="$SCHOOL_TEACHER_WS"
else
  echo "no school workspace available" >&2
  exit 1
fi
TEACHER_WS="${SCHOOL_TEACHER_WS:-$SCHOOL_WS}"
teacher_invitation_token=""
intake_stamp="$(date +%s)"
registered_email="register-$RANDOM-$(date +%s)@example.com"
register_json=$(curl -fsS -H "Content-Type: application/json" -X POST "$BASE/api/auth/register" -d "{\"display_name\":\"Smoke注册老师\",\"email\":\"$registered_email\",\"password\":\"password123\"}")
registered_user_id=$(echo "$register_json" | json_get "if(p.data.user.email!=='$registered_email') process.exit(1); console.log(p.data.user.id)")
registered_token=$(echo "$register_json" | json_get "if(!p.data.token.includes(p.data.user.id)) process.exit(1); console.log(p.data.token)")
registered_workspace_id=$(echo "$register_json" | json_get "const ws=p.data.workspaces.find((item)=>item.type==='personal' && item.role==='personal_owner'); if(!ws) process.exit(1); console.log(ws.id)")
expect_error 401 unauthorized - POST "/api/auth/login" public "{\"identifier\":\"$registered_email\",\"password\":\"wrong-password\"}"
curl -fsS -H "Content-Type: application/json" -X POST "$BASE/api/auth/login" -d "{\"identifier\":\"$registered_email\",\"password\":\"password123\"}" | json_get "if(p.data.user.id!=='$registered_user_id') process.exit(1); console.log('registered_login=password_checked');"
curl -fsS -H "Authorization: Bearer $registered_token" "$BASE/api/auth/me" | json_get "if(p.data.user.id!=='$registered_user_id' || p.data.workspaces[0].id!=='$registered_workspace_id') process.exit(1); console.log('registered=' + p.data.workspaces[0].name);"
demo_auth_header="$AUTH_HEADER"
AUTH_HEADER="Authorization: Bearer $registered_token"
expect_error 403 forbidden - GET "/api/operator/submissions"
expect_error 403 forbidden - GET "/api/operator/audit-logs"
expect_error 403 forbidden - GET "/api/operator/generation-costs"
expect_error 403 forbidden - GET "/api/operator/generation-provider"
expect_error 403 forbidden - GET "/api/operator/storage"
expect_error 403 forbidden - GET "/api/operator/readiness"
expect_error 403 forbidden - PATCH "/api/operator/marketplace/templates/50000000-0000-0000-0000-000000000001" auth '{"title":"Smoke越权模板"}'
api GET "/api/workspaces/$registered_workspace_id/generation-provider" | json_get "
if(!p.data.provider || !Array.isArray(p.data.supports_text)) process.exit(1);
const text = p.data.components?.find((item)=>item.kind==='text' && item.provider==='deepseek');
const image = p.data.components?.find((item)=>item.kind==='image' && item.provider==='seedream');
if(!text || !image) process.exit(1);
if(text.configured !== false || text.ready !== false || !text.required_configuration?.includes('DEEPSEEK_API_KEY')) process.exit(1);
if(image.configured !== false || image.ready !== false || !image.required_configuration?.includes('SEEDREAM_API_KEY 或 ARK_API_KEY')) process.exit(1);
if(!image.model || !image.endpoint?.includes('/api/v3/images/generations')) process.exit(1);
console.log('registered_generation_provider=' + p.data.provider + '/' + image.provider);
"
expect_error 403 forbidden - GET "/api/workspaces/$SCHOOL_WS/dashboard"
expect_error 403 forbidden - POST "/api/workspaces/$SCHOOL_WS/children" auth '{"nickname":"Smoke跨空间儿童","age_group":"4-5 岁","interests":["积木"],"traits":["好奇"],"focus":"不应创建"}'
expect_error 403 forbidden - POST "/api/workspaces/$SCHOOL_WS/storybooks" auth '{"title":"Smoke跨空间绘本","age_group":"4-5 岁","use_scene":"不应创建","teaching_goal":"不应创建"}'
AUTH_HEADER="$demo_auth_header"
echo "registered_cross_workspace_forbidden=ok"
api GET "/api/operator/generation-provider" | json_get "
if(p.data.provider!=='mock' || !Array.isArray(p.data.components)) process.exit(1);
const text = p.data.components.find((item)=>item.kind==='text' && item.provider==='deepseek');
const image = p.data.components.find((item)=>item.kind==='image' && item.provider==='seedream');
if(!text || !image || text.ready !== false || image.ready !== false) process.exit(1);
if(!image.endpoint?.includes('/api/v3/images/generations')) process.exit(1);
console.log('operator_generation_provider=' + p.data.provider + '/' + image.provider);
"
api GET "/api/operator/storage" | json_get "
if(p.data.backend !== 'local') process.exit(1);
if(!p.data.exports_dir?.endsWith('exports')) process.exit(1);
if(!p.data.generated_images_dir?.endsWith('generated-images')) process.exit(1);
if(p.data.export_max_bytes !== 52428800) process.exit(1);
if(p.data.generated_image_max_bytes !== 15728640) process.exit(1);
if(p.data.filename_validation !== true) process.exit(1);
if(p.data.size_limit_enabled !== true) process.exit(1);
if(p.data.download_strategy !== 'authenticated_api') process.exit(1);
if(p.data.public_direct_access !== false) process.exit(1);
console.log('operator_storage=' + p.data.exports_dir + '/' + p.data.export_max_bytes);
"
api GET "/api/operator/readiness" | json_get "
if(typeof p.data.ready !== 'boolean') process.exit(1);
if(!Array.isArray(p.data.checks) || p.data.checks.length < 7) process.exit(1);
const keys = p.data.checks.map((item)=>item.key);
for (const key of ['database','database_schema','app_host','generation_provider','storage','generation_budget','demo_seed']) {
  if(!keys.includes(key)) process.exit(1);
}
if(!p.data.provider || p.data.provider.provider !== 'mock') process.exit(1);
if(!p.data.storage || p.data.storage.backend !== 'local') process.exit(1);
if(p.data.ready !== false || p.data.mode !== 'needs_attention') process.exit(1);
console.log('operator_readiness=' + p.data.mode + '/' + keys.join(','));
"
registered_storybook_json=$(curl -fsS -H "Authorization: Bearer $registered_token" -H "Content-Type: application/json" -X POST "$BASE/api/workspaces/$registered_workspace_id/storybooks" -d '{"title":"Smoke注册个人绘本","age_group":"4-5 岁","use_scene":"个人共读","teaching_goal":"验证注册个人空间"}')
registered_storybook_id=$(echo "$registered_storybook_json" | json_get "if(p.data.workspace_id!=='$registered_workspace_id') process.exit(1); console.log(p.data.id)")

echo "3. dashboard"
api GET "/api/workspaces/$SCHOOL_WS/dashboard" | json_get "console.log(p.data.workspace.name + ' / storybooks=' + p.data.storybooks.length);"

echo "4. permission boundaries"
expect_error 401 unauthorized - GET "/api/auth/me" noauth
if [[ -n "$TEACHER_WS" ]]; then
  teacher_invite_json=$(api POST "/api/workspaces/$SCHOOL_WS/members" "{\"name\":\"Smoke老师权限验证\",\"email\":\"teacher-smoke-$intake_stamp@example.com\",\"classes\":[\"小一班\"]}")
  teacher_invitation_token=$(echo "$teacher_invite_json" | json_get "console.log(p.data.invitation_token || p.data.id)")
  api GET "/api/invitations/$teacher_invitation_token" | json_get "if(p.data.status!=='invited') process.exit(1); console.log('teacher_invite=' + p.data.workspace_name);"
  api POST "/api/invitations/$teacher_invitation_token/accept" | json_get "if(p.data.status!=='active') process.exit(1); console.log('teacher_invite_accepted=' + p.data.status);"
  expect_error 403 forbidden - GET "/api/workspaces/$TEACHER_WS/members"
  expect_error 403 forbidden - POST "/api/workspaces/$TEACHER_WS/classes" auth '{"name":"Smoke老师越权班级","age_group":"4-5 岁"}'
  expect_error 403 forbidden - GET "/api/workspaces/$TEACHER_WS/submissions"
  expect_error 403 forbidden - GET "/api/workspaces/$TEACHER_WS/parent-intakes"
  expect_error 403 forbidden - GET "/api/workspaces/$TEACHER_WS/parent-intake-links"
  expect_error 401 unauthorized - GET "/api/workspaces/$TEACHER_WS/generation-provider" noauth
  AUTH_HEADER="Authorization: Bearer $registered_token"
  expect_error 403 forbidden - GET "/api/workspaces/$TEACHER_WS/generation-provider"
  AUTH_HEADER="$demo_auth_header"
  api GET "/api/workspaces/$TEACHER_WS/children" | json_get "const allowed=['中一班','小二班']; if(!p.data.some((row)=>row.nickname==='安安')) process.exit(1); if(!p.data.every((row)=>allowed.includes(row.classroom))) process.exit(1); console.log('teacher_children_scope=' + p.data.length);"
  api GET "/api/workspaces/$TEACHER_WS/children?limit=1&offset=0" | json_get "if(!p.meta || p.meta.limit!==1 || p.meta.offset!==0 || p.data.length!==1 || p.meta.total < 1) process.exit(1); console.log('teacher_children_page=' + p.data.length + '/' + p.meta.total);"
  api GET "/api/workspaces/$TEACHER_WS/children?limit=1&offset=999" | json_get "if(!p.meta || p.meta.limit!==1 || p.meta.total < 1 || p.data.length!==0 || p.meta.has_more!==false) process.exit(1); console.log('teacher_children_page_empty=' + p.meta.offset + '/' + p.meta.total);"
  expect_error 403 forbidden - POST "/api/workspaces/$TEACHER_WS/children" auth '{"nickname":"Smoke老师未选班级儿童","age_group":"4-5 岁","interests":["积木"],"traits":["好奇"],"focus":"验证班级授权"}'
  api GET "/api/workspaces/$TEACHER_WS/generation-provider" | json_get "
if(!p.data.provider || !Array.isArray(p.data.supports_text)) process.exit(1);
const text = p.data.components?.find((item)=>item.kind==='text' && item.provider==='deepseek');
const image = p.data.components?.find((item)=>item.kind==='image' && item.provider==='seedream');
if(!text || !image) process.exit(1);
if(!Array.isArray(text.supports) || !text.supports.includes('storybook_plan')) process.exit(1);
if(!Array.isArray(image.supports) || !image.supports.includes('storybook_page_image')) process.exit(1);
console.log('teacher_generation_provider=' + p.data.provider + '/' + image.provider);
"
  teacher_child_json=$(api POST "/api/workspaces/$TEACHER_WS/children" "{\"nickname\":\"Smoke老师授权班级儿童$(date +%s)\",\"age_group\":\"4-5 岁\",\"classroom\":\"中一班\",\"interests\":[\"积木\"],\"traits\":[\"好奇\"],\"focus\":\"验证班级授权\"}")
  teacher_child_id=$(echo "$teacher_child_json" | json_get "if(p.data.classroom!=='中一班') process.exit(1); console.log(p.data.id)")
  teacher_plan_job_json=$(api POST "/api/workspaces/$TEACHER_WS/generation-jobs" '{"job_type":"storybook_plan","input_json":{"theme":"老师任务输入脱敏验证","private_note":"不应在老师查询响应中原样返回"}}')
  teacher_plan_job_id=$(echo "$teacher_plan_job_json" | json_get "if(p.data.job_type!=='storybook_plan') process.exit(1); console.log(p.data.id)")
  api GET "/api/workspaces/$TEACHER_WS/generation-jobs/$teacher_plan_job_id" | json_get "if(p.data.input_json?.redacted!==true || p.data.input_json?.reason!=='limited_workspace_role') process.exit(1); console.log('teacher_generation_job_redacted=' + p.data.id);"
  api GET "/api/workspaces/$TEACHER_WS/generation-jobs?limit=20&offset=0" | json_get "const row=p.data.find((item)=>item.id==='$teacher_plan_job_id'); if(!row || row.input_json?.redacted!==true) process.exit(1); console.log('teacher_generation_job_list_redacted=' + row.id);"
  echo "permission boundaries ok"
fi

echo "5. create child"
expect_error 400 validation_error nickname POST "/api/workspaces/$SCHOOL_WS/children" auth '{"nickname":"   ","age_group":"4-5 岁","classroom":"小一班","interests":["积木"],"traits":["好奇"],"focus":"等待和表达"}'
created_child_json=$(api POST "/api/workspaces/$SCHOOL_WS/children" "{\"nickname\":\" Smoke儿童$intake_stamp \",\"age_group\":\"4-5 岁\",\"classroom\":\"小一班\",\"interests\":[\" 积木 \",\"\",\"积木\",\"唱歌\"],\"traits\":[\" 好奇 \",\"好奇\",\"\"],\"focus\":\"等待和表达\"}")
created_child_id=$(echo "$created_child_json" | json_get "if(p.data.nickname!=='Smoke儿童$intake_stamp' || p.data.completeness!==90 || p.data.interests.join('|') !== '积木|唱歌' || p.data.traits.join('|') !== '好奇') process.exit(1); console.log(p.data.id)")
api GET "/api/workspaces/$SCHOOL_WS/children?limit=1&offset=0" | json_get "if(!p.meta || p.meta.limit!==1 || p.meta.offset!==0 || p.data.length!==1 || p.meta.total < 2 || p.meta.has_more!==true) process.exit(1); console.log('children_page=' + p.data.length + '/' + p.meta.total);"
updated_child_name="Smoke更新儿童$intake_stamp"
api PATCH "/api/workspaces/$SCHOOL_WS/children/$created_child_id" "{\"nickname\":\" $updated_child_name \",\"interests\":[\"积木\",\" 贴纸 \",\"贴纸\",\"\"],\"traits\":[\"好奇\",\"需要鼓励\",\"需要鼓励\"],\"focus\":\"等待、表达和收拾玩具\"}" | json_get "if(p.data.nickname!=='$updated_child_name' || p.data.interests.join('|') !== '积木|贴纸' || p.data.traits.join('|') !== '好奇|需要鼓励' || p.data.focus!=='等待、表达和收拾玩具' || p.data.completeness!==100) process.exit(1); console.log('child_updated=' + p.data.nickname + ':' + p.data.completeness);"
archived_child_json=$(api POST "/api/workspaces/$SCHOOL_WS/children" "{\"nickname\":\"Smoke归档儿童$intake_stamp\",\"age_group\":\"4-5 岁\",\"classroom\":\"小一班\",\"interests\":[\"积木\"],\"traits\":[\"好奇\"],\"focus\":\"离园归档验证\"}")
archived_child_id=$(echo "$archived_child_json" | json_get "if(p.data.status!=='active') process.exit(1); console.log(p.data.id)")
api POST "/api/workspaces/$SCHOOL_WS/children/$archived_child_id/archive" | json_get "if(p.data.id!=='$archived_child_id' || p.data.status!=='archived') process.exit(1); console.log('child_archived=' + p.data.nickname);"
api GET "/api/workspaces/$SCHOOL_WS/children?limit=100&offset=0" | json_get "if(p.data.some((row)=>row.id==='$archived_child_id')) process.exit(1); console.log('child_archive_hidden=ok');"
expect_error 404 not_found - GET "/api/workspaces/$SCHOOL_WS/children/$archived_child_id"
expect_error 409 state_conflict - POST "/api/workspaces/$SCHOOL_WS/children/$archived_child_id/archive"
api POST "/api/workspaces/$SCHOOL_WS/children/$archived_child_id/restore" | json_get "if(p.data.id!=='$archived_child_id' || p.data.status!=='active') process.exit(1); console.log('child_restored=' + p.data.nickname);"
api GET "/api/workspaces/$SCHOOL_WS/children?limit=100&offset=0" | json_get "if(!p.data.some((row)=>row.id==='$archived_child_id')) process.exit(1); console.log('child_restore_visible=ok');"
expect_error 409 state_conflict - POST "/api/workspaces/$SCHOOL_WS/children/$archived_child_id/restore"
echo "child=$created_child_id"

echo "6. create plain storybook"
plan_job_json=$(api POST "/api/workspaces/$SCHOOL_WS/generation-jobs" '{"job_type":"storybook_plan","input_json":{"theme":"验证普通绘本方案"}}')
plan_job_id=$(echo "$plan_job_json" | json_get "if(p.data.job_type!=='storybook_plan' || p.data.status!=='queued' || p.data.output_json!==null) process.exit(1); console.log(p.data.id)")
wait_for_job_status "$plan_job_id" succeeded
roles_job_json=$(api POST "/api/workspaces/$SCHOOL_WS/generation-jobs" '{"job_type":"storybook_roles","input_json":{"title":"Smoke角色生成"}}')
roles_job_id=$(echo "$roles_job_json" | json_get "if(p.data.job_type!=='storybook_roles' || p.data.status!=='queued' || p.data.output_json!==null) process.exit(1); console.log(p.data.id)")
wait_for_job_status "$roles_job_id" succeeded
pages_job_json=$(api POST "/api/workspaces/$SCHOOL_WS/generation-jobs" '{"job_type":"storybook_pages","input_json":{"page_count":6}}')
pages_job_id=$(echo "$pages_job_json" | json_get "if(p.data.job_type!=='storybook_pages' || p.data.status!=='queued' || p.data.output_json!==null) process.exit(1); console.log(p.data.id)")
wait_for_job_status "$pages_job_id" succeeded
retry_job_id=$(node -e "console.log(require('crypto').randomUUID())")
docker exec -i "$DB_CONTAINER" psql -U postgres -d "$DB_NAME" -v ON_ERROR_STOP=1 >/dev/null <<SQL
insert into generation_jobs
  (id, workspace_id, storybook_id, job_type, status, input_json, output_json, created_at, finished_at)
values
  ('$retry_job_id', '$SCHOOL_WS', null, 'storybook_plan', 'failed', '{"theme":"重试路径验证"}'::jsonb, '{"schema_version":"generation.error.v1","message":"待重试"}'::jsonb, now(), now());
SQL
retry_job_json=$(api POST "/api/workspaces/$SCHOOL_WS/generation-jobs/$retry_job_id/retry")
echo "$retry_job_json" | json_get "if(p.data.id!=='$retry_job_id' || p.data.status!=='succeeded' || p.data.output_json?.provider!=='mock' || p.data.attempt_count < 1 || p.data.locked_by !== null || p.data.locked_at !== null) process.exit(1); console.log('retry_job=' + p.data.id)" >/dev/null
cancel_job_id=$(node -e "console.log(require('crypto').randomUUID())")
docker exec -i "$DB_CONTAINER" psql -U postgres -d "$DB_NAME" -v ON_ERROR_STOP=1 >/dev/null <<SQL
insert into generation_jobs
  (id, workspace_id, storybook_id, job_type, status, input_json, output_json, created_at)
values
  ('$cancel_job_id', '$SCHOOL_WS', null, 'storybook_plan', 'queued', '{"theme":"取消路径验证"}'::jsonb, null, now());
SQL
api POST "/api/workspaces/$SCHOOL_WS/generation-jobs/$cancel_job_id/cancel" | json_get "if(p.data.id!=='$cancel_job_id' || p.data.status!=='canceled' || p.data.output_json?.schema_version!=='generation.canceled.v1' || p.data.locked_by !== null || p.data.locked_at !== null || p.data.finished_at === null) process.exit(1); console.log('cancel_job=' + p.data.id);" >/dev/null
expect_error 409 state_conflict - POST "/api/workspaces/$SCHOOL_WS/generation-jobs/$plan_job_id/cancel"
scoped_recover_job_id=$(node -e "console.log(require('crypto').randomUUID())")
foreign_recover_job_id=$(node -e "console.log(require('crypto').randomUUID())")
docker exec -i "$DB_CONTAINER" psql -U postgres -d "$DB_NAME" -v ON_ERROR_STOP=1 >/dev/null <<SQL
insert into generation_jobs
  (id, workspace_id, storybook_id, job_type, status, input_json, output_json, attempt_count, locked_by, locked_at, created_at)
values
  ('$scoped_recover_job_id', '$SCHOOL_WS', null, 'storybook_plan', 'running', '{"theme":"本空间恢复验证"}'::jsonb, null, 1, 'stale-worker', now() - interval '30 minutes', now()),
  ('$foreign_recover_job_id', '$registered_workspace_id', null, 'storybook_plan', 'running', '{"theme":"跨空间不应恢复"}'::jsonb, null, 1, 'stale-worker', now() - interval '30 minutes', now());
SQL
api POST "/api/workspaces/$SCHOOL_WS/generation-jobs/recover" '{"age_minutes":15,"limit":5}' | json_get "if(p.data.processed < 1) process.exit(1); console.log('recover_processed=' + p.data.processed);" >/dev/null
api GET "/api/workspaces/$SCHOOL_WS/generation-jobs/$scoped_recover_job_id" | json_get "if(p.data.status!=='succeeded' || p.data.locked_by !== null || p.data.locked_at !== null) process.exit(1); console.log('recover_scoped=' + p.data.status);" >/dev/null
AUTH_HEADER="Authorization: Bearer $registered_token"
api GET "/api/workspaces/$registered_workspace_id/generation-jobs/$foreign_recover_job_id" | json_get "if(p.data.status!=='running' || p.data.locked_by !== 'stale-worker') process.exit(1); console.log('recover_foreign_untouched=' + p.data.status);" >/dev/null
AUTH_HEADER="$demo_auth_header"
api GET "/api/workspaces/$SCHOOL_WS/generation-jobs" | json_get "const expected=['$plan_job_id','$roles_job_id','$pages_job_id','$retry_job_id']; const ids=new Set(p.data.map((row)=>row.id)); if(!expected.every((id)=>ids.has(id))) process.exit(1); if(!p.data.every((row)=>row.workspace_id==='$SCHOOL_WS')) process.exit(1); console.log('generation_jobs=' + p.data.length);" >/dev/null
api GET "/api/workspaces/$SCHOOL_WS/generation-jobs?limit=1&offset=0" | json_get "if(!p.meta || p.meta.limit!==1 || p.meta.offset!==0 || p.data.length!==1 || p.meta.total < 4 || p.meta.has_more!==true) process.exit(1); console.log('generation_job_page=' + p.data.length + '/' + p.meta.total);" >/dev/null
api GET "/api/workspaces/$SCHOOL_WS/generation-jobs?limit=1&offset=999" | json_get "if(!p.meta || p.meta.limit!==1 || p.meta.total < 4 || p.data.length!==0 || p.meta.has_more!==false) process.exit(1); console.log('generation_job_page_empty=' + p.meta.offset + '/' + p.meta.total);" >/dev/null
expect_error 401 unauthorized - GET "/api/workspaces/$SCHOOL_WS/generation-jobs" noauth
created_plain_json=$(api POST "/api/workspaces/$SCHOOL_WS/storybooks" "{\"title\":\"Smoke普通绘本$intake_stamp\",\"age_group\":\"4-5 岁\",\"use_scene\":\"规则引导\",\"teaching_goal\":\"验证普通绘本闭环\"}")
created_plain_id=$(echo "$created_plain_json" | json_get "console.log(p.data.id)")
page_id=$(echo "$created_plain_json" | json_get "console.log(p.data.pages[0].id)")
role_id=$(echo "$created_plain_json" | json_get "console.log(p.data.roles[0].id)")
echo "storybook=$created_plain_id page=$page_id role=$role_id"
AUTH_HEADER="Authorization: Bearer $registered_token"
expect_error 403 forbidden - GET "/api/workspaces/$SCHOOL_WS/storybooks/$created_plain_id"
expect_error 403 forbidden - PATCH "/api/workspaces/$SCHOOL_WS/storybooks/$created_plain_id" auth '{"visibility":"workspace"}'
expect_error 403 forbidden - POST "/api/workspaces/$SCHOOL_WS/storybooks/$created_plain_id/duplicate"
expect_error 403 forbidden - POST "/api/workspaces/$SCHOOL_WS/storybooks/$created_plain_id/derive-custom" auth "{\"child_id\":\"$created_child_id\",\"intensity\":\"standard\"}"
AUTH_HEADER="$demo_auth_header"
echo "storybook_cross_workspace_forbidden=ok"
expect_error 409 state_conflict - POST "/api/workspaces/$SCHOOL_WS/storybooks/$created_plain_id/exports"
expect_error 409 state_conflict - POST "/api/workspaces/$SCHOOL_WS/storybooks/$created_plain_id/share-links" auth '{}'
echo "undeliverable_delivery_blocked=ok"
personal_child_id=$(api GET "/api/workspaces/$PERSONAL_WS/children?limit=1&offset=0" | json_get "if(!p.data[0]?.id) process.exit(1); console.log(p.data[0].id)")
expect_error 404 not_found - POST "/api/workspaces/$SCHOOL_WS/generation-jobs" auth "{\"job_type\":\"customization_plan\",\"storybook_id\":\"$created_plain_id\",\"input_json\":{\"child_id\":\"$personal_child_id\",\"intensity\":\"standard\"}}"
applied_roles_json=$(api POST "/api/workspaces/$SCHOOL_WS/generation-jobs" "{\"job_type\":\"storybook_roles\",\"storybook_id\":\"$created_plain_id\",\"input_json\":{\"title\":\"Smoke角色落库\",\"teacher_name\":\"Smoke鹿老师\"}}")
applied_roles_job_id=$(echo "$applied_roles_json" | json_get "if(p.data.status!=='queued' || p.data.storybook_id!=='$created_plain_id') process.exit(1); console.log(p.data.id)")
wait_for_job_status "$applied_roles_job_id" succeeded
applied_pages_json=$(api POST "/api/workspaces/$SCHOOL_WS/generation-jobs" "{\"job_type\":\"storybook_pages\",\"storybook_id\":\"$created_plain_id\",\"input_json\":{\"page_count\":6}}")
applied_pages_job_id=$(echo "$applied_pages_json" | json_get "if(p.data.status!=='queued' || p.data.storybook_id!=='$created_plain_id') process.exit(1); console.log(p.data.id)")
wait_for_job_status "$applied_pages_job_id" succeeded
api GET "/api/workspaces/$SCHOOL_WS/generation-jobs?storybook_id=$created_plain_id&limit=1&offset=0" | json_get "if(!p.meta || p.meta.limit!==1 || p.meta.offset!==0 || p.data.length!==1 || p.meta.total < 2 || !p.data.every((row)=>row.storybook_id==='$created_plain_id')) process.exit(1); console.log('storybook_generation_job_page=' + p.data.length + '/' + p.meta.total);" >/dev/null
api GET "/api/workspaces/$SCHOOL_WS/generation-jobs?storybook_id=$created_plain_id&limit=1&offset=999" | json_get "if(!p.meta || p.meta.limit!==1 || p.meta.total < 2 || p.data.length!==0 || p.meta.has_more!==false) process.exit(1); console.log('storybook_generation_job_page_empty=' + p.meta.offset + '/' + p.meta.total);" >/dev/null
created_plain_after_generation_json=$(api GET "/api/workspaces/$SCHOOL_WS/storybooks/$created_plain_id")
page_id=$(echo "$created_plain_after_generation_json" | json_get "if(p.data.pages.length!==6) process.exit(1); console.log(p.data.pages[0].id)")
role_id=$(echo "$created_plain_after_generation_json" | json_get "const teacher=p.data.roles.find((item)=>item.name==='Smoke鹿老师'); if(!teacher) process.exit(1); console.log(teacher.id)")

echo "7. update page and image task"
expect_error 400 validation_error body PATCH "/api/workspaces/$SCHOOL_WS/storybooks/$created_plain_id/pages/$page_id" auth '{"body":"   "}'
expect_error 400 validation_error name PATCH "/api/workspaces/$SCHOOL_WS/storybooks/$created_plain_id/roles/$role_id" auth '{"name":"   "}'
expect_error 400 validation_error title PATCH "/api/workspaces/$SCHOOL_WS/storybooks/$created_plain_id" auth '{"title":"   "}'
expect_error 400 validation_error use_scene PATCH "/api/workspaces/$SCHOOL_WS/storybooks/$created_plain_id" auth '{"use_scene":"   "}'
renamed_plain_title="Smoke改名绘本$intake_stamp"
encoded_renamed_plain_title=$(node -e "console.log(encodeURIComponent(process.argv[1]))" "$renamed_plain_title")
api PATCH "/api/workspaces/$SCHOOL_WS/storybooks/$created_plain_id" "{\"title\":\" $renamed_plain_title \",\"age_group\":\"5-6 岁\",\"use_scene\":\"情绪引导\",\"teaching_goal\":\"练习表达生气和寻求帮助\",\"cover_tone\":\"明亮、有安全感\"}" | json_get "if(p.data.title!=='$renamed_plain_title' || p.data.age_group!=='5-6 岁' || p.data.use_scene!=='情绪引导' || p.data.teaching_goal!=='练习表达生气和寻求帮助' || p.data.cover_tone!=='明亮、有安全感') process.exit(1); console.log('storybook_metadata_updated=' + p.data.title);" >/dev/null
api PATCH "/api/workspaces/$SCHOOL_WS/storybooks/$created_plain_id" '{"status":"roles_pending"}' | json_get "if(p.data.status!=='roles_pending') process.exit(1); console.log('storybook_status=' + p.data.status);" >/dev/null
api PATCH "/api/workspaces/$SCHOOL_WS/storybooks/$created_plain_id" '{"status":"editing"}' | json_get "if(p.data.status!=='editing') process.exit(1); console.log('storybook_status=' + p.data.status);" >/dev/null
api PATCH "/api/workspaces/$SCHOOL_WS/storybooks/$created_plain_id/pages/$page_id" '{"status":"generating"}' >/dev/null
expect_error 409 state_conflict - PATCH "/api/workspaces/$SCHOOL_WS/storybooks/$created_plain_id" auth '{"status":"exportable"}'
api PATCH "/api/workspaces/$SCHOOL_WS/storybooks/$created_plain_id/pages/$page_id" '{"status":"ready"}' >/dev/null
echo "deliverable_check_blocks_generating_page=ok"
api PATCH "/api/workspaces/$SCHOOL_WS/storybooks/$created_plain_id" '{"status":"exportable"}' | json_get "if(p.data.status!=='exportable') process.exit(1); console.log('storybook_deliverable=' + p.data.status);" >/dev/null
api PATCH "/api/workspaces/$SCHOOL_WS/storybooks/$created_plain_id/pages/$page_id" '{"title":" Smoke 第 1 页 ","body":" 孩子们一起练习等待。 ","illustration_prompt":" 明亮教室，老师和孩子围坐在地毯上 "}' >/dev/null
api PATCH "/api/workspaces/$SCHOOL_WS/storybooks/$created_plain_id/roles/$role_id" '{"name":" Smoke老师形象 ","role_type":" teacher ","appearance":" 温和、稳定、会蹲下来和孩子说话 ","story_function":" 帮助孩子理解等待和表达 ","needs_consistency":true}' | json_get "if(p.data.name!=='Smoke老师形象' || p.data.role_type!=='teacher') process.exit(1); console.log('role=' + p.data.name);" >/dev/null
api GET "/api/workspaces/$SCHOOL_WS/storybooks/$created_plain_id" | json_get "const page=p.data.pages.find((item)=>item.id==='$page_id'); if(!page || page.title!=='Smoke 第 1 页' || page.body!=='孩子们一起练习等待。' || page.illustration_prompt!=='明亮教室，老师和孩子围坐在地毯上') process.exit(1); const role=p.data.roles.find((item)=>item.id==='$role_id'); if(!role || role.name!=='Smoke老师形象' || role.appearance!=='温和、稳定、会蹲下来和孩子说话') process.exit(1); console.log('role_saved=' + role.name);" >/dev/null
duplicated_storybook_json=$(api POST "/api/workspaces/$SCHOOL_WS/storybooks/$created_plain_id/duplicate")
duplicated_storybook_id=$(echo "$duplicated_storybook_json" | json_get "if(p.data.status!=='draft' || p.data.visibility!=='private' || p.data.source!=='duplicate' || p.data.source_title !== '$renamed_plain_title' || !p.data.title.includes('副本')) process.exit(1); const page=p.data.pages.find((item)=>item.title==='Smoke 第 1 页' && item.body==='孩子们一起练习等待。'); const role=p.data.roles.find((item)=>item.name==='Smoke老师形象' && item.appearance==='温和、稳定、会蹲下来和孩子说话'); if(!page || !role) process.exit(1); console.log(p.data.id)")
api GET "/api/workspaces/$SCHOOL_WS/storybooks/$duplicated_storybook_id" | json_get "if(p.data.id!=='$duplicated_storybook_id' || p.data.status!=='draft' || p.data.visibility!=='private' || p.data.source_title !== '$renamed_plain_title' || p.data.pages[0].id==='$page_id') process.exit(1); console.log('storybook_duplicated=' + p.data.title);"
image_job_json=$(api POST "/api/workspaces/$SCHOOL_WS/storybooks/$created_plain_id/pages/$page_id/image-tasks" '{"prompt":"明亮教室，老师和孩子围坐在地毯上"}')
image_job_id=$(echo "$image_job_json" | json_get "if(p.data.status!=='queued' || p.data.output_json!==null) process.exit(1); console.log(p.data.id)")
wait_for_job_status "$image_job_id" succeeded
image_job_detail_json=$(api GET "/api/workspaces/$SCHOOL_WS/generation-jobs/$image_job_id")
image_file_url=$(echo "$image_job_detail_json" | json_get "const expected='/api/workspaces/$SCHOOL_WS/generation-jobs/$image_job_id/image'; const url=p.data.output_json?.image?.image_url; if(p.data.status!=='succeeded' || url !== expected) process.exit(1); console.log(url);")
expect_png_download "$image_file_url" generated_image auth
expect_error 401 unauthorized - GET "$image_file_url" noauth
AUTH_HEADER="Authorization: Bearer $registered_token"
expect_error 403 forbidden - GET "$image_file_url"
AUTH_HEADER="$demo_auth_header"
expect_error 404 not_found - GET "/generated-images/mock-$image_job_id.png" public
expect_error 404 not_found - GET "/generated-images/not-a-generated-image.txt" public
expect_error 404 not_found - GET "/generated-images/mock-page-1.png" public
echo "generated_image_cross_workspace_forbidden=ok"
echo "image_job=$image_job_id"

echo "8. derive custom storybook"
customization_plan_json=$(api POST "/api/workspaces/$SCHOOL_WS/generation-jobs" "{\"job_type\":\"customization_plan\",\"storybook_id\":\"$created_plain_id\",\"input_json\":{\"child_id\":\"$created_child_id\",\"intensity\":\"standard\"}}")
customization_plan_job_id=$(echo "$customization_plan_json" | json_get "if(p.data.job_type!=='customization_plan' || p.data.storybook_id!=='$created_plain_id' || p.data.status!=='queued' || p.data.output_json!==null) process.exit(1); console.log(p.data.id)")
wait_for_job_status "$customization_plan_job_id" succeeded
api GET "/api/operator/generation-costs?workspace_id=$SCHOOL_WS&status=succeeded&limit=100&offset=0" | json_get "
const expected = ['$plan_job_id', '$roles_job_id', '$pages_job_id', '$image_job_id', '$customization_plan_job_id'];
const ids = new Set(p.data.items.map((row)=>row.generation_job_id));
if(!expected.every((id)=>ids.has(id))) process.exit(1);
if(!p.meta || p.meta.total < expected.length) process.exit(1);
if(p.data.summary.total_jobs < expected.length) process.exit(1);
if(p.data.summary.total_images < 1) process.exit(1);
if(p.data.items.some((row)=>row.workspace_id !== '$SCHOOL_WS' || row.status !== 'succeeded')) process.exit(1);
console.log('operator_generation_costs=' + p.data.items.length + '/' + p.meta.total);
" >/dev/null
api GET "/api/operator/generation-costs?workspace_id=$SCHOOL_WS&provider=mock&job_type=storybook_page_image&limit=20&offset=0" | json_get "
const image = p.data.items.find((row)=>row.generation_job_id==='$image_job_id');
if(!image || image.provider !== 'mock' || image.job_type !== 'storybook_page_image' || image.image_count !== 1) process.exit(1);
console.log('operator_generation_cost_filter=' + image.id);
" >/dev/null
created_custom_json=$(api POST "/api/workspaces/$SCHOOL_WS/storybooks/$created_plain_id/derive-custom" "{\"child_id\":\"$created_child_id\",\"intensity\":\"standard\"}")
created_custom_id=$(echo "$created_custom_json" | json_get "const childName='$updated_child_name'; const hasChildTitle=p.data.title.includes(childName); const hasCustomPage=p.data.pages.some((page)=>page.title.includes(childName) && page.body.includes('定制改写') && page.body.includes('贴纸') && page.status==='needs_regeneration'); const hasCustomRole=p.data.roles.some((role)=>role.name.includes(childName) && role.role_type==='protagonist'); if(p.data.type!=='custom' || !hasChildTitle || !hasCustomPage || !hasCustomRole) process.exit(1); console.log(p.data.id)")
echo "custom=$created_custom_id"
api GET "/api/workspaces/$SCHOOL_WS/storybooks?limit=1&offset=0" | json_get "if(!p.meta || p.meta.limit!==1 || p.meta.offset!==0 || p.data.length!==1 || p.meta.total < 2 || p.meta.has_more!==true) process.exit(1); console.log('storybook_page=' + p.data.length + '/' + p.meta.total);"
api GET "/api/workspaces/$SCHOOL_WS/storybooks?type=custom&target_child_id=$created_child_id&limit=1&offset=0" | json_get "if(!p.meta || p.meta.limit!==1 || p.meta.offset!==0 || p.data.length!==1 || p.data[0].id!=='$created_custom_id' || p.data[0].target_child_id!=='$created_child_id') process.exit(1); console.log('child_storybook_page=' + p.data.length + '/' + p.meta.total);"
api GET "/api/workspaces/$SCHOOL_WS/storybooks?type=plain&q=$encoded_renamed_plain_title&limit=5&offset=0" | json_get "if(!p.meta || p.meta.limit!==5 || p.meta.offset!==0 || p.meta.total < 1 || !p.data.some((row)=>row.id==='$created_plain_id')) process.exit(1); console.log('storybook_search=' + p.data.length + '/' + p.meta.total);"
api GET "/api/workspaces/$SCHOOL_WS/storybooks?q=no-such-smoke-storybook&limit=5&offset=0" | json_get "if(!p.meta || p.meta.total!==0 || p.data.length!==0 || p.meta.has_more!==false) process.exit(1); console.log('storybook_search_empty=0');"

echo "9. visibility, export, share"
api PATCH "/api/workspaces/$SCHOOL_WS/storybooks/$created_plain_id/pages/$page_id" '{"body":"家长手机号是 138 0013 8000，这段内容不能被导出或分享。"}' >/dev/null
expect_error 409 state_conflict - POST "/api/workspaces/$SCHOOL_WS/storybooks/$created_plain_id/exports"
expect_error 409 state_conflict - POST "/api/workspaces/$SCHOOL_WS/storybooks/$created_plain_id/share-links" auth '{}'
api PATCH "/api/workspaces/$SCHOOL_WS/storybooks/$created_plain_id/pages/$page_id" '{"body":"孩子们一起练习等待。"}' >/dev/null
echo "delivery_privacy_blocked=ok"
api PATCH "/api/workspaces/$SCHOOL_WS/storybooks/$created_plain_id" '{"visibility":"workspace"}' | json_get "if(p.data.visibility!=='workspace') process.exit(1); console.log('visibility=' + p.data.visibility);"
export_json=$(api POST "/api/workspaces/$SCHOOL_WS/storybooks/$created_plain_id/exports")
export_job_id=$(echo "$export_json" | json_get "if(!['queued','running','succeeded'].includes(p.data.status)) process.exit(1); console.log(p.data.id)")
wait_for_export_status "$created_plain_id" "$export_job_id" succeeded
api GET "/api/workspaces/$SCHOOL_WS/storybooks/$created_plain_id/exports" | json_get "if(!p.data.some((row)=>row.id==='$export_job_id' && row.status==='succeeded' && row.file_url)) process.exit(1); console.log('exports=' + p.data.length);"
api GET "/api/workspaces/$SCHOOL_WS/storybooks/$created_plain_id/exports?limit=1&offset=0" | json_get "if(!p.meta || p.meta.limit!==1 || p.meta.offset!==0 || p.data.length!==1 || p.meta.total < 1 || !p.data.some((row)=>row.id==='$export_job_id')) process.exit(1); console.log('export_page=' + p.data.length + '/' + p.meta.total);"
export_detail_json=$(api GET "/api/workspaces/$SCHOOL_WS/storybooks/$created_plain_id/exports/$export_job_id")
export_file_url=$(echo "$export_detail_json" | json_get "const expected='/api/workspaces/$SCHOOL_WS/storybooks/$created_plain_id/exports/$export_job_id/download'; if(p.data.id!=='$export_job_id' || p.data.status!=='succeeded' || p.data.file_url !== expected) process.exit(1); console.log(p.data.file_url);")
echo "$export_detail_json" | json_get "console.log('export=' + p.data.status);"
expect_pdf_download "$export_file_url" export auth "Smoke" "/Subtype /Image"
expect_error 401 unauthorized - GET "$export_file_url" noauth
AUTH_HEADER="Authorization: Bearer $registered_token"
expect_error 403 forbidden - GET "$export_file_url"
AUTH_HEADER="$demo_auth_header"
expect_error 404 not_found - GET "/exports/$export_job_id.pdf" public
expect_error 404 not_found - GET "/exports/storybook-1.pdf" public
echo "export_cross_workspace_forbidden=ok"
share_json=$(api POST "/api/workspaces/$SCHOOL_WS/storybooks/$created_plain_id/share-links")
share_link_id=$(echo "$share_json" | json_get "if(p.data.access_count !== 0 || p.data.last_accessed_at) process.exit(1); console.log(p.data.id)")
share_token=$(echo "$share_json" | json_get "console.log(p.data.token)")
expired_share_json=$(api POST "/api/workspaces/$SCHOOL_WS/storybooks/$created_plain_id/share-links" '{"expires_at":"2020-01-01T00:00:00Z"}')
expired_share_link_id=$(echo "$expired_share_json" | json_get "if(p.data.status!=='expired' || !p.data.expires_at) process.exit(1); console.log(p.data.id)")
expired_share_token=$(echo "$expired_share_json" | json_get "console.log(p.data.token)")
api GET "/api/workspaces/$SCHOOL_WS/storybooks/$created_plain_id/share-links" | json_get "if(!p.data.some((row)=>row.id==='$share_link_id' && row.status==='active')) process.exit(1); console.log('share_links=' + p.data.length);"
api GET "/api/workspaces/$SCHOOL_WS/storybooks/$created_plain_id/share-links" | json_get "if(p.data.some((row)=>row.id==='$expired_share_link_id')) process.exit(1); console.log('expired_share_hidden=ok');"
api GET "/api/workspaces/$SCHOOL_WS/storybooks/$created_plain_id/share-links?limit=1&offset=0" | json_get "if(!p.meta || p.meta.limit!==1 || p.meta.offset!==0 || p.data.length!==1 || p.meta.total < 1 || !p.data.some((row)=>row.id==='$share_link_id')) process.exit(1); console.log('share_link_page=' + p.data.length + '/' + p.meta.total);"
curl -fsS "$BASE/api/share-links/$share_token" | json_get "if(p.data.id!=='$created_plain_id') process.exit(1); console.log('share=' + p.data.title);"
api GET "/api/workspaces/$SCHOOL_WS/storybooks/$created_plain_id/share-links" | json_get "const item=p.data.find((row)=>row.id==='$share_link_id'); if(!item || item.access_count < 1 || !item.last_accessed_at) process.exit(1); console.log('share_link_access=' + item.access_count);"
expect_error 404 not_found - GET "/api/share-links/$expired_share_token" public
api PATCH "/api/workspaces/$SCHOOL_WS/storybooks/$created_plain_id/pages/$page_id" '{"body":"公开分享导出前再次出现手机号 138 0013 8000，必须拦截。"}' >/dev/null
expect_error 409 state_conflict - POST "/api/share-links/$share_token/exports" public
api PATCH "/api/workspaces/$SCHOOL_WS/storybooks/$created_plain_id/pages/$page_id" '{"body":"孩子们一起练习等待。"}' >/dev/null
echo "public_delivery_privacy_blocked=ok"
public_export_json=$(curl -fsS -X POST "$BASE/api/share-links/$share_token/exports")
public_export_id=$(echo "$public_export_json" | json_get "if(!['queued','running','succeeded'].includes(p.data.status)) process.exit(1); console.log(p.data.id)")
wait_for_public_export_status "$share_token" "$public_export_id" succeeded
public_export_detail_json=$(curl -fsS "$BASE/api/share-links/$share_token/exports/$public_export_id")
public_export_file_url=$(echo "$public_export_detail_json" | json_get "const expected='/api/share-links/$share_token/exports/$public_export_id/download'; if(p.data.id!=='$public_export_id' || p.data.status!=='succeeded' || p.data.file_url !== expected) process.exit(1); console.log(p.data.file_url);")
echo "$public_export_detail_json" | json_get "console.log('public_export=' + p.data.status);"
expect_pdf_download "$public_export_file_url" public_export public "Smoke" "/Subtype /Image"
expect_error 404 not_found - GET "/exports/$public_export_id.pdf" public
api POST "/api/workspaces/$SCHOOL_WS/storybooks/$created_plain_id/share-links/$share_link_id/revoke" | json_get "if(p.data.status!=='revoked') process.exit(1); console.log('share_revoked=' + p.data.status);"
api GET "/api/workspaces/$SCHOOL_WS/storybooks/$created_plain_id/share-links" | json_get "if(p.data.some((row)=>row.id==='$share_link_id')) process.exit(1); console.log('share_links_after_revoke=' + p.data.length);"
expect_error 404 not_found - GET "/api/share-links/$share_token" public
expect_error 404 not_found - POST "/api/share-links/$share_token/exports" public
expect_error 404 not_found - GET "/api/share-links/$share_token/exports/$public_export_id" public
expect_error 404 not_found - GET "$public_export_file_url" public

echo "10. marketplace copy"
templates_json=$(api GET "/api/marketplace/templates")
api GET "/api/marketplace/templates?limit=1&offset=0" | json_get "if(!p.meta || p.meta.limit!==1 || p.meta.offset!==0 || p.data.length!==1 || p.meta.total < 1) process.exit(1); console.log('template_page=' + p.data.length + '/' + p.meta.total);"
api GET "/api/marketplace/templates?source=platform&supports_customization=true&limit=5&offset=0" | json_get "if(!p.meta || p.meta.limit!==5 || p.meta.offset!==0 || p.meta.total < 1 || !p.data.every((row)=>row.source_type==='platform' && row.supports_customization===true)) process.exit(1); console.log('template_filter=' + p.data.length + '/' + p.meta.total);"
api GET "/api/marketplace/templates?q=no-such-market-template&limit=5&offset=0" | json_get "if(!p.meta || p.meta.total!==0 || p.data.length!==0 || p.meta.has_more!==false) process.exit(1); console.log('template_search_empty=0');"
template_id=$(echo "$templates_json" | json_get "console.log(p.data[0].id)")
copied_json=$(api POST "/api/workspaces/$SCHOOL_WS/marketplace/templates/$template_id/copy")
copied_book_id=$(echo "$copied_json" | json_get "if(p.data.source!=='marketplace') process.exit(1); console.log(p.data.id)")
echo "copied=$copied_book_id"

echo "11. admin member, class, submission"
member_email="smoke-$intake_stamp@example.com"
expect_error 404 not_found - POST "/api/workspaces/$SCHOOL_WS/members" auth "{\"name\":\"Smoke无效班级老师\",\"email\":\"invalid-class-$intake_stamp@example.com\",\"classes\":[\"不存在班级\"]}"
member_json=$(api POST "/api/workspaces/$SCHOOL_WS/members" "{\"name\":\"Smoke老师\",\"email\":\"$member_email\",\"classes\":[\" 小一班 \",\"\",\"小一班\"]}")
member_id=$(echo "$member_json" | json_get "console.log(p.data.id)")
invitation_token=$(echo "$member_json" | json_get "console.log(p.data.invitation_token || p.data.id)")
curl -fsS "$BASE/api/invitations/$invitation_token" | json_get "if(p.data.status!=='invited' || p.data.role!=='school_teacher') process.exit(1); console.log('invite=' + p.data.workspace_name);"
revoked_member_email="revoked-smoke-$intake_stamp@example.com"
revoked_member_json=$(api POST "/api/workspaces/$SCHOOL_WS/members" "{\"name\":\"Smoke撤回老师\",\"email\":\"$revoked_member_email\",\"classes\":[\"小一班\"]}")
revoked_member_id=$(echo "$revoked_member_json" | json_get "console.log(p.data.id)")
revoked_invitation_token=$(echo "$revoked_member_json" | json_get "console.log(p.data.invitation_token || p.data.id)")
api POST "/api/workspaces/$SCHOOL_WS/members/$revoked_member_id/revoke-invitation" | json_get "if(p.data.status!=='revoked' || p.data.invitation_token) process.exit(1); console.log('member_invite_revoked=' + p.data.status);"
curl -fsS "$BASE/api/invitations/$revoked_invitation_token" | json_get "if(p.data.status!=='revoked') process.exit(1); console.log('revoked_invite=' + p.data.status);"
curl -fsS -X POST "$BASE/api/invitations/$revoked_invitation_token/accept" | json_get "if(p.data.status!=='revoked') process.exit(1); console.log('revoked_invite_accept=' + p.data.status);"
expect_error 409 state_conflict - POST "/api/workspaces/$SCHOOL_WS/members/$revoked_member_id/revoke-invitation" auth
curl -fsS -X POST "$BASE/api/invitations/$invitation_token/accept" | json_get "if(p.data.status!=='active') process.exit(1); console.log('accepted=' + p.data.invited_contact);"
api GET "/api/workspaces/$SCHOOL_WS/members" | json_get "const item=p.data.find((row)=>row.id==='$member_id'); if(!item || item.status!=='active' || item.classes.join('|') !== '小一班') process.exit(1); console.log('member=' + item.status);"
api GET "/api/workspaces/$SCHOOL_WS/members?limit=1&offset=0" | json_get "if(!p.meta || p.meta.limit!==1 || p.meta.offset!==0 || p.data.length!==1 || p.meta.total < 2 || p.meta.has_more!==true) process.exit(1); console.log('member_page=' + p.data.length + '/' + p.meta.total);"
expect_error 400 validation_error age_group POST "/api/workspaces/$SCHOOL_WS/classes" auth "{\"name\":\"Smoke空年龄段$intake_stamp\",\"age_group\":\"   \"}"
expect_error 409 state_conflict - POST "/api/workspaces/$SCHOOL_WS/classes/80000000-0000-0000-0000-000000000001/archive" auth
class_json=$(api POST "/api/workspaces/$SCHOOL_WS/classes" "{\"name\":\" Smoke班级$intake_stamp \",\"age_group\":\" 4-5 岁 \"}")
classroom_id=$(echo "$class_json" | json_get "if(p.data.name!=='Smoke班级$intake_stamp' || p.data.age_group!=='4-5 岁') process.exit(1); console.log(p.data.id)")
expect_error 409 state_conflict - POST "/api/workspaces/$SCHOOL_WS/classes" auth "{\"name\":\"Smoke班级$intake_stamp\",\"age_group\":\"4-5 岁\"}"
api GET "/api/workspaces/$SCHOOL_WS/classes?limit=1&offset=0" | json_get "if(!p.meta || p.meta.limit!==1 || p.meta.offset!==0 || p.data.length!==1 || p.meta.total < 2 || p.meta.has_more!==true) process.exit(1); console.log('class_page=' + p.data.length + '/' + p.meta.total);"
api POST "/api/workspaces/$SCHOOL_WS/classes/$classroom_id/archive" | json_get "if(p.data.status!=='archived') process.exit(1); console.log('class_archived=' + p.data.status);"
api GET "/api/workspaces/$SCHOOL_WS/classes" | json_get "if(p.data.some((row)=>row.id==='$classroom_id')) process.exit(1); console.log('class_hidden_after_archive=ok');"
expect_error 409 state_conflict - POST "/api/workspaces/$SCHOOL_WS/classes/$classroom_id/archive" auth
risky_storybook_json=$(api POST "/api/workspaces/$SCHOOL_WS/storybooks" "{\"title\":\"Smoke隐私风险绘本$intake_stamp\",\"age_group\":\"4-5 岁\",\"use_scene\":\"市场投稿隐私验证\",\"teaching_goal\":\"验证投稿前隐私扫描\"}")
risky_storybook_id=$(echo "$risky_storybook_json" | json_get "console.log(p.data.id)")
risky_page_id=$(echo "$risky_storybook_json" | json_get "console.log(p.data.pages[0].id)")
api PATCH "/api/workspaces/$SCHOOL_WS/storybooks/$risky_storybook_id/pages/$risky_page_id" '{"body":"老师电话是 138 0013 8000，请不要进入市场。"}' >/dev/null
risky_submission_json=$(api POST "/api/workspaces/$SCHOOL_WS/submissions" "{\"storybook_id\":\"$risky_storybook_id\"}")
risky_submission_id=$(echo "$risky_submission_json" | json_get "if(p.data.status!=='draft' || p.data.privacy_confirmed!==false) process.exit(1); console.log(p.data.id)")
expect_error 409 state_conflict - POST "/api/workspaces/$SCHOOL_WS/submissions/$risky_submission_id/privacy-confirm" auth
api GET "/api/workspaces/$SCHOOL_WS/submissions?status=draft&limit=50&offset=0" | json_get "const item=p.data.find((row)=>row.id==='$risky_submission_id'); if(!item || item.status!=='draft' || item.privacy_confirmed!==false) process.exit(1); console.log('submission_privacy_risk_blocked=' + item.status);"
rejected_submission_json=$(api POST "/api/workspaces/$SCHOOL_WS/submissions" "{\"storybook_id\":\"$copied_book_id\"}")
rejected_submission_id=$(echo "$rejected_submission_json" | json_get "console.log(p.data.id)")
api POST "/api/workspaces/$SCHOOL_WS/submissions/$rejected_submission_id/privacy-confirm" | json_get "if(!p.data.privacy_confirmed || p.data.status!=='submitted') process.exit(1); console.log('submission_for_reject=' + p.data.status);"
api POST "/api/operator/submissions/$rejected_submission_id/reject" | json_get "if(p.data.status!=='rejected') process.exit(1); console.log('submission_rejected=' + p.data.status);"
api GET "/api/workspaces/$SCHOOL_WS/submissions?status=rejected&limit=20&offset=0" | json_get "if(!p.data.some((row)=>row.id==='$rejected_submission_id' && row.status==='rejected')) process.exit(1); console.log('submission_filter_rejected=' + p.data.length + '/' + p.meta.total);"
api GET "/api/operator/submissions?status=rejected&limit=20&offset=0" | json_get "if(!p.data.some((row)=>row.id==='$rejected_submission_id' && row.status==='rejected')) process.exit(1); console.log('operator_submission_filter_rejected=' + p.data.length + '/' + p.meta.total);"
api GET "/api/workspaces/$SCHOOL_WS/storybooks/$copied_book_id" | json_get "if(p.data.status!=='exportable' || p.data.visibility!=='private') process.exit(1); console.log('storybook_rejected_returned=' + p.data.visibility);"
submission_json=$(api POST "/api/workspaces/$SCHOOL_WS/submissions" "{\"storybook_id\":\"$created_plain_id\"}")
submission_id=$(echo "$submission_json" | json_get "console.log(p.data.id)")
expect_error 409 state_conflict - POST "/api/workspaces/$SCHOOL_WS/submissions" auth "{\"storybook_id\":\"$created_plain_id\"}"
api GET "/api/workspaces/$SCHOOL_WS/submissions?status=draft&limit=20&offset=0" | json_get "if(!p.data.some((row)=>row.id==='$submission_id' && row.status==='draft')) process.exit(1); console.log('submission_filter_draft=' + p.data.length + '/' + p.meta.total);"
expect_error 400 validation_error status GET "/api/workspaces/$SCHOOL_WS/submissions?status=unknown"
api POST "/api/workspaces/$SCHOOL_WS/submissions/$submission_id/privacy-confirm" | json_get "if(!p.data.privacy_confirmed) process.exit(1); console.log('submission=' + p.data.status);"
api GET "/api/workspaces/$SCHOOL_WS/storybooks/$created_plain_id" | json_get "if(p.data.status!=='submitted' || p.data.visibility!=='market_submission') process.exit(1); console.log('storybook_submission=' + p.data.visibility);"
api GET "/api/workspaces/$SCHOOL_WS/submissions?status=submitted&limit=20&offset=0" | json_get "if(!p.data.some((row)=>row.id==='$submission_id' && row.status==='submitted')) process.exit(1); console.log('submission_filter_submitted=' + p.data.length + '/' + p.meta.total);"
api GET "/api/workspaces/$SCHOOL_WS/submissions?limit=1&offset=0" | json_get "if(!p.meta || p.meta.limit!==1 || p.meta.offset!==0 || p.data.length!==1 || p.meta.total < 2 || p.meta.has_more!==true) process.exit(1); console.log('submission_page=' + p.data.length + '/' + p.meta.total);"
api GET "/api/workspaces/$SCHOOL_WS/submissions?limit=1&offset=999" | json_get "if(!p.meta || p.meta.limit!==1 || p.meta.total < 2 || p.data.length!==0 || p.meta.has_more!==false) process.exit(1); console.log('submission_page_empty=' + p.meta.offset + '/' + p.meta.total);"
operator_submissions_json=$(api GET "/api/operator/submissions")
echo "$operator_submissions_json" | json_get "const item=p.data.find((row)=>row.id==='$submission_id'); if(!item || item.status!=='submitted') process.exit(1); console.log('operator_queue=' + item.title);"
api GET "/api/operator/submissions?status=submitted&limit=20&offset=0" | json_get "if(!p.data.some((row)=>row.id==='$submission_id' && row.status==='submitted')) process.exit(1); console.log('operator_submission_filter_submitted=' + p.data.length + '/' + p.meta.total);"
expect_error 400 validation_error status GET "/api/operator/submissions?status=unknown"
api GET "/api/operator/submissions?limit=1&offset=0" | json_get "if(!p.meta || p.meta.limit!==1 || p.meta.offset!==0 || p.data.length!==1 || p.meta.total < 2 || p.meta.has_more!==true) process.exit(1); console.log('operator_submission_page=' + p.data.length + '/' + p.meta.total);"
api GET "/api/operator/submissions?limit=1&offset=999" | json_get "if(!p.meta || p.meta.limit!==1 || p.meta.total < 2 || p.data.length!==0 || p.meta.has_more!==false) process.exit(1); console.log('operator_submission_page_empty=' + p.meta.offset + '/' + p.meta.total);"
approved_template_json=$(api POST "/api/operator/submissions/$submission_id/approve")
approved_template_id=$(echo "$approved_template_json" | json_get "if(p.data.source_type!=='school_submission') process.exit(1); console.log(p.data.id)")
api GET "/api/operator/submissions?status=listed&limit=20&offset=0" | json_get "if(!p.data.some((row)=>row.id==='$submission_id' && row.status==='listed')) process.exit(1); console.log('operator_submission_filter_listed=' + p.data.length + '/' + p.meta.total);"
updated_template_title="Smoke市场模板编辑$intake_stamp"
api PATCH "/api/operator/marketplace/templates/$approved_template_id" "{\"title\":\"$updated_template_title\",\"summary\":\"运营已优化展示摘要，适合市场复用验证。\",\"age_group\":\"4-5 岁\",\"use_scene\":\"市场复用\",\"supports_customization\":true,\"tags\":[\"运营编辑\",\"市场复用\",\"运营编辑\"]}" | json_get "if(p.data.title !== '$updated_template_title' || p.data.use_scene !== '市场复用' || p.data.tags.filter((tag)=>tag==='运营编辑').length !== 1) process.exit(1); console.log('template_updated=' + p.data.title);"
api GET "/api/marketplace/templates?q=$updated_template_title" | json_get "const item=p.data.find((row)=>row.id==='$approved_template_id'); if(!item || item.summary !== '运营已优化展示摘要，适合市场复用验证。') process.exit(1); console.log('template_update_visible=' + item.title);"
api GET "/api/workspaces/$SCHOOL_WS/storybooks/$created_plain_id" | json_get "if(p.data.status!=='listed' || p.data.visibility!=='market_listed') process.exit(1); console.log('storybook_listed=' + p.data.visibility);"
api GET "/api/marketplace/templates?q=$updated_template_title" | json_get "const item=p.data.find((row)=>row.id==='$approved_template_id'); if(!item) process.exit(1); console.log('template_listed=' + item.title);"
approved_copied_json=$(api POST "/api/workspaces/$SCHOOL_WS/marketplace/templates/$approved_template_id/copy")
approved_copied_book_id=$(echo "$approved_copied_json" | json_get "const hasEditedPage=p.data.pages.some((page)=>page.title==='Smoke 第 1 页' && page.body.includes('练习等待')); const hasEditedRole=p.data.roles.some((role)=>role.name==='Smoke老师形象'); if(p.data.source!=='marketplace' || p.data.title!=='$updated_template_title' || !hasEditedPage || !hasEditedRole) process.exit(1); console.log(p.data.id)")
echo "approved_copied=$approved_copied_book_id"

echo "12. parent intake"
intake_nickname="Smoke家长提交$intake_stamp"
expect_error 400 validation_error workspace_id POST "/api/parent-intakes" noauth "{\"link_token\":\"demo-token\",\"workspace_id\":\"$PERSONAL_WS\",\"child_nickname\":\"Smoke错误空间$intake_stamp\",\"age_group\":\"4-5 岁\",\"interests\":[\"画画\"]}"
expect_error 404 not_found - POST "/api/workspaces/$SCHOOL_WS/parent-intake-links" auth "{\"label\":\"Smoke无效班级链接$intake_stamp\",\"classroom\":\"不存在班级\"}"
intake_link_json=$(api POST "/api/workspaces/$SCHOOL_WS/parent-intake-links" "{\"label\":\"Smoke家长链接$intake_stamp\",\"classroom\":\"小一班\"}")
intake_link_id=$(echo "$intake_link_json" | json_get "if(p.data.workspace_id!=='$SCHOOL_WS' || p.data.status!=='active' || p.data.classroom!=='小一班' || !p.data.url.includes('/link/intake/') || p.data.access_count !== 0 || p.data.last_accessed_at) process.exit(1); console.log(p.data.id)")
intake_link_token=$(echo "$intake_link_json" | json_get "if(!p.data.token) process.exit(1); console.log(p.data.token)")
api GET "/api/workspaces/$SCHOOL_WS/parent-intake-links" | json_get "const item=p.data.find((row)=>row.id==='$intake_link_id' && row.token==='$intake_link_token'); if(!item) process.exit(1); console.log('intake_link=' + item.label);"
api GET "/api/workspaces/$SCHOOL_WS/parent-intake-links?limit=1&offset=0" | json_get "if(!p.meta || p.meta.limit!==1 || p.meta.offset!==0 || p.data.length!==1 || p.meta.total < 1) process.exit(1); console.log('intake_link_page=' + p.data.length + '/' + p.meta.total);"
api GET "/api/workspaces/$SCHOOL_WS/parent-intake-links?status=active&limit=20&offset=0" | json_get "if(!p.data.some((row)=>row.id==='$intake_link_id' && row.status==='active')) process.exit(1); console.log('intake_link_filter_active=' + p.data.length + '/' + p.meta.total);"
api GET "/api/workspaces/$SCHOOL_WS/parent-intake-links?classroom=%E5%B0%8F%E4%B8%80%E7%8F%AD&limit=20&offset=0" | json_get "if(!p.data.some((row)=>row.id==='$intake_link_id' && row.classroom==='小一班')) process.exit(1); console.log('intake_link_filter_classroom=' + p.data.length + '/' + p.meta.total);"
expect_error 400 validation_error status GET "/api/workspaces/$SCHOOL_WS/parent-intake-links?status=unknown"
curl -fsS "$BASE/api/parent-intake-links/$intake_link_token" | json_get "if(p.data.token !== '$intake_link_token' || p.data.workspace_id !== '$SCHOOL_WS' || p.data.workspace_name !== '星星幼儿园' || p.data.classroom !== '小一班' || p.data.status !== 'active') process.exit(1); console.log('intake_link_public=' + p.data.workspace_name);"
api GET "/api/workspaces/$SCHOOL_WS/parent-intake-links" | json_get "const item=p.data.find((row)=>row.id==='$intake_link_id'); if(!item || item.access_count < 1 || !item.last_accessed_at) process.exit(1); console.log('intake_link_access=' + item.access_count);"
expect_error 400 validation_error workspace_id POST "/api/parent-intakes" noauth "{\"link_token\":\"$intake_link_token\",\"workspace_id\":\"$PERSONAL_WS\",\"child_nickname\":\"Smoke错误正式链接$intake_stamp\",\"age_group\":\"4-5 岁\",\"interests\":[\"画画\"]}"
curl -fsS -H "Content-Type: application/json" -X POST "$BASE/api/parent-intakes" -d "{\"link_token\":\"$intake_link_token\",\"workspace_id\":\"$SCHOOL_WS\",\"child_nickname\":\"$intake_nickname\",\"age_group\":\"4-5 岁\",\"interests\":[\"画画\",\"小汽车\"]}" | json_get "if(p.data.status!=='submitted') process.exit(1); console.log(p.data.message);"
api GET "/api/workspaces/$SCHOOL_WS/parent-intakes?limit=1&offset=0" | json_get "if(!p.meta || p.meta.limit!==1 || p.meta.offset!==0 || p.data.length!==1 || p.meta.total < 1) process.exit(1); console.log('parent_intake_page=' + p.data.length + '/' + p.meta.total);"
api GET "/api/workspaces/$SCHOOL_WS/parent-intakes?classroom=%E5%B0%8F%E4%B8%80%E7%8F%AD&limit=20&offset=0" | json_get "if(!p.data.some((row)=>row.child_nickname==='$intake_nickname' && row.classroom==='小一班')) process.exit(1); console.log('parent_intake_filter_classroom=' + p.data.length + '/' + p.meta.total);"
batch_intake_link_json=$(api POST "/api/workspaces/$SCHOOL_WS/parent-intake-links" "{\"label\":\"Smoke批量停用链接$intake_stamp\",\"classroom\":\"小一班\"}")
batch_intake_link_id=$(echo "$batch_intake_link_json" | json_get "if(p.data.status!=='active' || p.data.classroom!=='小一班') process.exit(1); console.log(p.data.id)")
batch_intake_link_token=$(echo "$batch_intake_link_json" | json_get "if(!p.data.token) process.exit(1); console.log(p.data.token)")
batch_global_link_json=$(api POST "/api/workspaces/$SCHOOL_WS/parent-intake-links" "{\"label\":\"Smoke全园保留链接$intake_stamp\"}")
batch_global_link_id=$(echo "$batch_global_link_json" | json_get "if(p.data.status!=='active' || p.data.classroom) process.exit(1); console.log(p.data.id)")
batch_global_link_token=$(echo "$batch_global_link_json" | json_get "if(!p.data.token) process.exit(1); console.log(p.data.token)")
api POST "/api/workspaces/$SCHOOL_WS/parent-intake-links/revoke-active?classroom=%E5%B0%8F%E4%B8%80%E7%8F%AD" | json_get "if(p.data.status!=='revoked' || !p.data.message.includes('已停用')) process.exit(1); console.log('intake_link_batch_revoke_classroom=' + p.data.message);"
curl -fsS "$BASE/api/parent-intake-links/$batch_intake_link_token" | json_get "if(p.data.status !== 'revoked') process.exit(1); console.log('intake_link_batch_classroom_public_revoked=' + p.data.status);"
curl -fsS "$BASE/api/parent-intake-links/$batch_global_link_token" | json_get "if(p.data.status !== 'active') process.exit(1); console.log('intake_link_batch_global_still_active=' + p.data.status);"
api POST "/api/workspaces/$SCHOOL_WS/parent-intake-links/revoke-active" | json_get "if(p.data.status!=='revoked' || !p.data.message.includes('已停用')) process.exit(1); console.log('intake_link_batch_revoke=' + p.data.message);"
api GET "/api/workspaces/$SCHOOL_WS/parent-intake-links?status=active&limit=50&offset=0" | json_get "if(p.data.some((row)=>row.id==='$batch_intake_link_id' || row.id==='$batch_global_link_id')) process.exit(1); console.log('intake_link_batch_removed_from_active=' + p.data.length + '/' + p.meta.total);"
api GET "/api/workspaces/$SCHOOL_WS/parent-intake-links?status=revoked&limit=50&offset=0" | json_get "if(!p.data.some((row)=>row.id==='$batch_intake_link_id' && row.status==='revoked') || !p.data.some((row)=>row.id==='$batch_global_link_id' && row.status==='revoked')) process.exit(1); console.log('intake_link_batch_revoked_visible=' + p.data.length + '/' + p.meta.total);"
curl -fsS "$BASE/api/parent-intake-links/$batch_global_link_token" | json_get "if(p.data.status !== 'revoked') process.exit(1); console.log('intake_link_batch_public_revoked=' + p.data.status);"
revoked_intake_link_json=$(api POST "/api/workspaces/$SCHOOL_WS/parent-intake-links" "{\"label\":\"Smoke撤回家长链接$intake_stamp\"}")
revoked_intake_link_id=$(echo "$revoked_intake_link_json" | json_get "if(p.data.status!=='active') process.exit(1); console.log(p.data.id)")
revoked_intake_link_token=$(echo "$revoked_intake_link_json" | json_get "if(!p.data.token) process.exit(1); console.log(p.data.token)")
api POST "/api/workspaces/$SCHOOL_WS/parent-intake-links/$revoked_intake_link_id/revoke" | json_get "if(p.data.status!=='revoked') process.exit(1); console.log('intake_link_revoked=' + p.data.label);"
api GET "/api/workspaces/$SCHOOL_WS/parent-intake-links?status=revoked&limit=20&offset=0" | json_get "if(!p.data.some((row)=>row.id==='$revoked_intake_link_id' && row.status==='revoked')) process.exit(1); console.log('intake_link_filter_revoked=' + p.data.length + '/' + p.meta.total);"
curl -fsS "$BASE/api/parent-intake-links/$revoked_intake_link_token" | json_get "if(p.data.status !== 'revoked') process.exit(1); console.log('intake_link_public_revoked=' + p.data.status);"
expect_error 404 not_found - POST "/api/parent-intakes" noauth "{\"link_token\":\"$revoked_intake_link_token\",\"workspace_id\":\"$SCHOOL_WS\",\"child_nickname\":\"Smoke撤回后提交$intake_stamp\",\"age_group\":\"4-5 岁\",\"interests\":[\"画画\"]}"
expired_intake_link_json=$(api POST "/api/workspaces/$SCHOOL_WS/parent-intake-links" "{\"label\":\"Smoke过期家长链接$intake_stamp\",\"expires_at\":\"2020-01-01T00:00:00Z\"}")
expired_intake_link_id=$(echo "$expired_intake_link_json" | json_get "if(p.data.status!=='expired' || !p.data.expires_at) process.exit(1); console.log(p.data.id)")
expired_intake_link_token=$(echo "$expired_intake_link_json" | json_get "if(!p.data.token) process.exit(1); console.log(p.data.token)")
api GET "/api/workspaces/$SCHOOL_WS/parent-intake-links" | json_get "const item=p.data.find((row)=>row.id==='$expired_intake_link_id' && row.status==='expired'); if(!item) process.exit(1); console.log('intake_link_expired=' + item.label);"
api GET "/api/workspaces/$SCHOOL_WS/parent-intake-links?status=expired&limit=20&offset=0" | json_get "if(!p.data.some((row)=>row.id==='$expired_intake_link_id' && row.status==='expired')) process.exit(1); console.log('intake_link_filter_expired=' + p.data.length + '/' + p.meta.total);"
curl -fsS "$BASE/api/parent-intake-links/$expired_intake_link_token" | json_get "if(p.data.status !== 'expired') process.exit(1); console.log('intake_link_public_expired=' + p.data.status);"
expect_error 404 not_found - POST "/api/parent-intakes" noauth "{\"link_token\":\"$expired_intake_link_token\",\"workspace_id\":\"$SCHOOL_WS\",\"child_nickname\":\"Smoke过期后提交$intake_stamp\",\"age_group\":\"4-5 岁\",\"interests\":[\"画画\"]}"
intake_id=$(api GET "/api/workspaces/$SCHOOL_WS/parent-intakes" | json_get "const item=p.data.find((row)=>row.child_nickname==='$intake_nickname'); if(!item || item.status!=='submitted') process.exit(1); console.log(item.id);")
confirmed_intake_child_json=$(api POST "/api/workspaces/$SCHOOL_WS/parent-intakes/$intake_id/confirm" '{"focus":"由家长提交资料确认入档","traits":["家长补充"]}')
confirmed_intake_child_id=$(echo "$confirmed_intake_child_json" | json_get "if(p.data.nickname!=='$intake_nickname' || p.data.classroom!=='小一班' || p.data.completeness!==90) process.exit(1); console.log(p.data.id)")
api GET "/api/workspaces/$SCHOOL_WS/parent-intakes" | json_get "const item=p.data.find((row)=>row.id==='$intake_id'); if(!item || item.status!=='confirmed' || item.confirmed_child_id!=='$confirmed_intake_child_id') process.exit(1); console.log('confirmed=' + item.child_nickname);"

echo "13. audit logs"
api GET "/api/workspaces/$SCHOOL_WS/audit-logs?limit=100&offset=0" | json_get "
const actions = new Set(p.data.map((row)=>row.action));
for (const action of [
  'child.created',
  'child.updated',
  'child.archived',
  'child.restored',
  'storybook.created',
  'storybook.duplicated',
  'storybook.updated',
  'storybook.page_updated',
  'storybook.role_updated',
  'storybook.custom_derived',
  'parent_intake.submitted',
  'parent_intake.confirmed',
  'parent_intake_link.created',
  'parent_intake_link.revoked',
  'parent_intake_link.active_revoked',
  'marketplace_template.copied',
  'generation_job.created',
  'generation_job.retried',
  'generation_job.canceled',
  'workspace_member.invited',
  'workspace_member.invitation_revoked',
  'classroom.created',
  'classroom.archived',
  'storybook.export_created',
  'storybook.delivery_privacy_blocked',
  'storybook.share_link_created',
  'storybook.share_link_revoked',
  'marketplace_submission.created',
  'marketplace_submission.privacy_blocked',
  'marketplace_submission.privacy_confirmed'
]) {
  if (!actions.has(action)) {
    console.error('missing audit action from API: ' + action);
    process.exit(1);
  }
}
if (p.data.some((row)=>row.workspace_id && row.workspace_id !== '$SCHOOL_WS')) process.exit(1);
const childUpdated = p.data.find((row)=>row.action==='child.updated' && row.resource_id==='$created_child_id');
if (!childUpdated || childUpdated.metadata_json?.completeness !== 100) process.exit(1);
const storybookMetadataUpdated = p.data.find((row)=>row.action==='storybook.updated' && row.resource_id==='$created_plain_id' && row.metadata_json?.title === '$renamed_plain_title');
if (!storybookMetadataUpdated || storybookMetadataUpdated.metadata_json?.age_group !== '5-6 岁' || storybookMetadataUpdated.metadata_json?.use_scene !== '情绪引导') process.exit(1);
const storybookDeliverable = p.data.find((row)=>row.action==='storybook.updated' && row.resource_id==='$created_plain_id' && row.metadata_json?.status === 'exportable');
if (!storybookDeliverable) process.exit(1);
const childArchived = p.data.find((row)=>row.action==='child.archived' && row.resource_id==='$archived_child_id');
if (!childArchived || childArchived.metadata_json?.status !== 'archived') process.exit(1);
const childRestored = p.data.find((row)=>row.action==='child.restored' && row.resource_id==='$archived_child_id');
if (!childRestored || childRestored.metadata_json?.status !== 'active') process.exit(1);
const customDerived = p.data.find((row)=>row.action==='storybook.custom_derived' && row.resource_id==='$created_custom_id');
if (!customDerived || customDerived.metadata_json?.source_storybook_id !== '$created_plain_id' || customDerived.metadata_json?.target_child_id !== '$created_child_id' || customDerived.metadata_json?.intensity !== 'standard') process.exit(1);
const storybookDuplicated = p.data.find((row)=>row.action==='storybook.duplicated' && row.resource_id==='$duplicated_storybook_id');
if (!storybookDuplicated || storybookDuplicated.metadata_json?.source_storybook_id !== '$created_plain_id' || storybookDuplicated.metadata_json?.status !== 'draft') process.exit(1);
const marketCopied = p.data.find((row)=>row.action==='marketplace_template.copied' && row.resource_id==='$approved_copied_book_id');
if (!marketCopied || marketCopied.metadata_json?.template_id !== '$approved_template_id' || marketCopied.metadata_json?.source_type !== 'school_submission') process.exit(1);
const imageGeneration = p.data.find((row)=>row.action==='generation_job.created' && row.resource_id==='$image_job_id');
if (!imageGeneration || imageGeneration.metadata_json?.job_type !== 'storybook_page_image' || imageGeneration.metadata_json?.page_id !== '$page_id') process.exit(1);
const retriedGeneration = p.data.find((row)=>row.action==='generation_job.retried' && row.resource_id==='$retry_job_id');
if (!retriedGeneration || retriedGeneration.metadata_json?.job_type !== 'storybook_plan' || retriedGeneration.metadata_json?.status !== 'succeeded' || retriedGeneration.metadata_json?.attempt_count < 1) process.exit(1);
const intakeConfirmed = p.data.find((row)=>row.action==='parent_intake.confirmed' && row.resource_id==='$intake_id');
if (!intakeConfirmed || intakeConfirmed.metadata_json?.confirmed_child_id !== '$confirmed_intake_child_id' || intakeConfirmed.metadata_json?.completeness !== 90) process.exit(1);
const intakeSubmitted = p.data.find((row)=>row.action==='parent_intake.submitted' && row.metadata_json?.child_nickname === '$intake_nickname');
if (!intakeSubmitted || intakeSubmitted.metadata_json?.link_token !== '$intake_link_token' || intakeSubmitted.metadata_json?.interest_count !== 2) process.exit(1);
const intakeLinkCreated = p.data.find((row)=>row.action==='parent_intake_link.created' && row.resource_id==='$intake_link_id');
if (!intakeLinkCreated || intakeLinkCreated.metadata_json?.label !== 'Smoke家长链接$intake_stamp' || intakeLinkCreated.metadata_json?.status !== 'active') process.exit(1);
const intakeLinkRevoked = p.data.find((row)=>row.action==='parent_intake_link.revoked' && row.resource_id==='$revoked_intake_link_id');
if (!intakeLinkRevoked || intakeLinkRevoked.metadata_json?.label !== 'Smoke撤回家长链接$intake_stamp' || intakeLinkRevoked.metadata_json?.status !== 'revoked') process.exit(1);
const intakeLinksBatchRevoked = p.data.find((row)=>row.action==='parent_intake_link.active_revoked' && row.metadata_json?.revoked_count >= 1);
if (!intakeLinksBatchRevoked) process.exit(1);
const privacyBlocked = p.data.find((row)=>row.action==='marketplace_submission.privacy_blocked' && row.resource_id==='$risky_submission_id');
if (!privacyBlocked || privacyBlocked.metadata_json?.status !== 'draft' || privacyBlocked.metadata_json?.privacy_confirmed !== false || !privacyBlocked.metadata_json?.risk_labels?.includes('手机号')) process.exit(1);
const deliveryBlocked = p.data.filter((row)=>row.action==='storybook.delivery_privacy_blocked' && row.resource_id==='$created_plain_id');
const deliveryOperations = new Set(deliveryBlocked.map((row)=>row.metadata_json?.operation));
for (const operation of ['export', 'share_link', 'public_export']) {
  if (!deliveryOperations.has(operation)) {
    console.error('missing delivery privacy audit operation: ' + operation);
    process.exit(1);
  }
}
if (!deliveryBlocked.every((row)=>row.metadata_json?.risk_labels?.includes('手机号'))) process.exit(1);
console.log('audit_api=' + p.data.length);
"
api GET "/api/workspaces/$SCHOOL_WS/audit-logs?limit=1&offset=0" | json_get "if(!p.meta || p.meta.limit!==1 || p.meta.offset!==0 || p.data.length!==1 || p.meta.total < 20 || p.meta.has_more!==true) process.exit(1); console.log('audit_page=' + p.data.length + '/' + p.meta.total);"
api GET "/api/workspaces/$SCHOOL_WS/audit-logs?limit=1&offset=999" | json_get "if(!p.meta || p.meta.limit!==1 || p.meta.total < 20 || p.data.length!==0 || p.meta.has_more!==false) process.exit(1); console.log('audit_page_empty=' + p.meta.offset + '/' + p.meta.total);"
expect_error 403 forbidden - GET "/api/workspaces/$TEACHER_WS/audit-logs"
api GET "/api/operator/audit-logs?limit=100&offset=0" | json_get "
const actions = new Set(p.data.map((row)=>row.action));
for (const action of [
  'child.created',
  'child.updated',
  'child.archived',
  'child.restored',
  'storybook.created',
  'storybook.duplicated',
  'storybook.updated',
  'storybook.page_updated',
  'storybook.role_updated',
  'storybook.custom_derived',
  'parent_intake.submitted',
  'parent_intake.confirmed',
  'parent_intake_link.created',
  'parent_intake_link.revoked',
  'parent_intake_link.active_revoked',
  'marketplace_template.copied',
  'generation_job.created',
  'generation_job.retried',
  'workspace_member.invited',
  'workspace_member.invitation_revoked',
  'classroom.created',
  'classroom.archived',
  'storybook.export_created',
  'storybook.delivery_privacy_blocked',
  'storybook.share_link_created',
  'storybook.share_link_revoked',
  'marketplace_submission.created',
  'marketplace_submission.privacy_blocked',
  'marketplace_submission.privacy_confirmed',
  'marketplace_submission.rejected',
  'marketplace_submission.approved',
  'share_link.public_export_created'
]) {
  if (!actions.has(action)) {
    console.error('missing operator audit action: ' + action);
    process.exit(1);
  }
}
if (!p.data.some((row)=>row.workspace_id === null || row.workspace_id === undefined)) process.exit(1);
const rejectedSubmission = p.data.find((row)=>row.action==='marketplace_submission.rejected' && row.resource_id==='$rejected_submission_id');
if (!rejectedSubmission || rejectedSubmission.metadata_json?.status !== 'rejected') process.exit(1);
const updatedTemplate = p.data.find((row)=>row.action==='marketplace_template.updated' && row.resource_id==='$approved_template_id');
if (!updatedTemplate || updatedTemplate.metadata_json?.template_title !== '$updated_template_title') process.exit(1);
console.log('audit_operator=' + p.data.length);
"
api GET "/api/operator/audit-logs?limit=1&offset=0" | json_get "if(!p.meta || p.meta.limit!==1 || p.meta.offset!==0 || p.data.length!==1 || p.meta.total < 20 || p.meta.has_more!==true) process.exit(1); console.log('operator_audit_page=' + p.data.length + '/' + p.meta.total);"
api GET "/api/operator/audit-logs?limit=1&offset=999" | json_get "if(!p.meta || p.meta.limit!==1 || p.meta.total < 20 || p.data.length!==0 || p.meta.has_more!==false) process.exit(1); console.log('operator_audit_page_empty=' + p.meta.offset + '/' + p.meta.total);"
audit_count=$(docker exec -i "$DB_CONTAINER" psql -U postgres -d "$DB_NAME" -Atc "
select count(*)
from audit_logs
where action in (
  'child.created',
  'child.updated',
  'storybook.created',
  'storybook.updated',
  'storybook.page_updated',
  'storybook.role_updated',
  'storybook.custom_derived',
  'parent_intake.submitted',
  'parent_intake.confirmed',
  'parent_intake_link.created',
  'parent_intake_link.revoked',
  'parent_intake_link.active_revoked',
  'marketplace_template.copied',
  'marketplace_template.updated',
  'generation_job.created',
  'generation_job.retried',
  'generation_job.canceled',
  'workspace_member.invited',
  'workspace_member.invitation_revoked',
  'classroom.created',
  'classroom.archived',
  'storybook.export_created',
  'storybook.delivery_privacy_blocked',
  'storybook.share_link_created',
  'storybook.share_link_revoked',
  'marketplace_submission.created',
  'marketplace_submission.privacy_confirmed',
  'marketplace_submission.rejected',
  'marketplace_submission.approved',
  'share_link.public_export_created'
)
and resource_id in (
  nullif('$member_id', '')::uuid,
  nullif('$classroom_id', '')::uuid,
  nullif('$created_child_id', '')::uuid,
  nullif('$created_custom_id', '')::uuid,
  nullif('$created_plain_id', '')::uuid,
  nullif('$copied_book_id', '')::uuid,
  nullif('$approved_copied_book_id', '')::uuid,
  nullif('$plan_job_id', '')::uuid,
  nullif('$roles_job_id', '')::uuid,
  nullif('$pages_job_id', '')::uuid,
  nullif('$applied_roles_job_id', '')::uuid,
  nullif('$applied_pages_job_id', '')::uuid,
  nullif('$customization_plan_job_id', '')::uuid,
  nullif('$image_job_id', '')::uuid,
  nullif('$retry_job_id', '')::uuid,
  nullif('$cancel_job_id', '')::uuid,
  nullif('$page_id', '')::uuid,
  nullif('$role_id', '')::uuid,
  nullif('$intake_id', '')::uuid,
  nullif('$intake_link_id', '')::uuid,
  nullif('$batch_intake_link_id', '')::uuid,
  nullif('$revoked_intake_link_id', '')::uuid,
  nullif('$expired_intake_link_id', '')::uuid,
  nullif('$export_job_id', '')::uuid,
  nullif('$public_export_id', '')::uuid,
  nullif('$share_link_id', '')::uuid,
  nullif('$submission_id', '')::uuid,
  nullif('$approved_template_id', '')::uuid,
  nullif('$rejected_submission_id', '')::uuid
);
")
if [[ "$audit_count" -lt 29 ]]; then
  echo "expected at least 29 audit logs but found $audit_count" >&2
  exit 1
fi
echo "audit_logs=$audit_count"

echo "== smoke ok =="
