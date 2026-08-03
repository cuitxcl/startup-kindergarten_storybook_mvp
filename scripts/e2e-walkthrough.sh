#!/bin/bash
# E2E walkthrough: login -> create storybook -> plan/roles/pages generation -> illustration -> export PDF
set -u
cd "$(dirname "$0")/.."
BASE=http://127.0.0.1:8080
export DATABASE_URL=postgres://postgres:postgres@127.0.0.1:55432/kindleaf_development
export KINDLEAF_DEMO_SEED=1
export KINDLEAF_GENERATION_PROVIDER=mock

jqp() { python3 -c "import sys,json;d=json.load(sys.stdin);$1"; }

cleanup() {
  [ -n "${SRV_PID:-}" ] && kill "$SRV_PID" 2>/dev/null
  pkill -f 'kindleaf-server start' 2>/dev/null
  sleep 1
  for f in .env.local server/.env.local; do
    [ -f "$f.e2e-bak" ] && mv "$f.e2e-bak" "$f"
  done
}
trap cleanup EXIT

# .env.local 会以覆盖方式加载真实 provider 密钥；走查期间临时移开以使用确定性 mock，退出时恢复
for f in .env.local server/.env.local; do
  [ -f "$f" ] && mv "$f" "$f.e2e-bak"
done

# 1. start backend
(cd server && exec ./target/debug/kindleaf-server start -a > /tmp/kindleaf-e2e-server.log 2>&1) &
SRV_PID=$!

ready=""
for i in $(seq 1 60); do
  if curl -s -m 2 "$BASE/api/health" >/dev/null 2>&1; then ready=1; break; fi
  sleep 1
done
[ -z "$ready" ] && { echo "FAIL: backend did not become ready"; tail -20 /tmp/kindleaf-e2e-server.log; exit 1; }
echo "OK: backend ready"

# 2. login
LOGIN=$(curl -s -X POST "$BASE/api/auth/login" -H 'Content-Type: application/json' -d '{"identifier":"lin@example.com","password":"demo"}')
TOKEN=$(echo "$LOGIN" | jqp "print(d.get('token') or d.get('data',{}).get('token') or '')")
[ -z "$TOKEN" ] && { echo "FAIL: login: $LOGIN"; exit 1; }
echo "OK: login"
AUTH="Authorization: Bearer $TOKEN"
CT="Content-Type: application/json"

WS=$(echo "$LOGIN" | jqp "
ws=d.get('workspaces') or d.get('data',{}).get('workspaces') or []
school=[w for w in ws if 'school' in (w.get('slug') or '')]
print((school or ws)[0]['id'])")
echo "OK: workspace=$WS"

# 3. create storybook
BOOK=$(curl -s -X POST "$BASE/api/workspaces/$WS/storybooks" -H "$AUTH" -H "$CT" \
  -d '{"title":"E2E 走查绘本","age_group":"中班","use_scene":"课堂教学","teaching_goal":"学会分享"}')
BOOK_ID=$(echo "$BOOK" | jqp "print(d.get('id') or d.get('data',{}).get('id') or '')")
[ -z "$BOOK_ID" ] && { echo "FAIL: create storybook: $BOOK"; exit 1; }
echo "OK: storybook=$BOOK_ID"

# 4. run plan -> roles -> pages generation jobs
poll_job() { # $1 job id, $2 budget seconds
  local id=$1 n=0
  while [ $n -lt "$2" ]; do
    local J=$(curl -s "$BASE/api/workspaces/$WS/generation-jobs/$id" -H "$AUTH")
    local ST=$(echo "$J" | jqp "print(d.get('status') or d.get('data',{}).get('status') or '')")
    case "$ST" in
      queued|running|"") ;;
      *) echo "$ST"; return 0;;
    esac
    sleep 1; n=$((n+1))
  done
  echo "timeout"
}

BASE_INPUT='{"title":"E2E 走查绘本","theme":"学会分享","age_group":"中班","page_count":4,"use_scene":"课堂教学","style":"温暖水彩"}'
PLAN_INPUT='{"title":"E2E 走查绘本","theme":"学会分享","age_group":"中班","page_count":4,"use_scene":"课堂教学","style":"温暖水彩","plan":{"title":"E2E 走查绘本","theme":"学会分享","summary":"孩子们学习轮流与分享的故事","outline":["引入冲突","朋友表达想法","老师给出办法","大家一起尝试"],"role_requirements":["主角孩子","同伴孩子","老师","关键道具"],"review_points":["情节连贯","角色一致"]}}'

run_job() { # $1 job_type, $2 input_json, $3 budget seconds
  local JT=$1 INPUT=$2
  local JOB=$(curl -s -X POST "$BASE/api/workspaces/$WS/generation-jobs" -H "$AUTH" -H "$CT" \
    -d "{\"job_type\":\"$JT\",\"storybook_id\":\"$BOOK_ID\",\"input_json\":$INPUT}")
  local JOB_ID=$(echo "$JOB" | jqp "print(d.get('id') or d.get('data',{}).get('id') or '')")
  [ -z "$JOB_ID" ] && { echo "FAIL: create $JT job: $JOB"; exit 1; }
  local ST=$(poll_job "$JOB_ID" "$3")
  echo "job $JT -> $ST"
  [ "$ST" != "succeeded" ] && { echo "FAIL: $JT ended with $ST"; exit 1; }
  LAST_JOB_ID=$JOB_ID
}

run_job storybook_plan "$BASE_INPUT" 90
run_job storybook_roles "$PLAN_INPUT" 90

# 分页任务要求 illustration_prompt 含已确认角色姓名：从角色任务产出中提取角色，构造 confirmed_roles
ROLES_JOB=$(curl -s "$BASE/api/workspaces/$WS/generation-jobs/$LAST_JOB_ID" -H "$AUTH")
PAGES_INPUT=$(echo "$ROLES_JOB" | python3 -c "
import sys, json
d = json.load(sys.stdin)
out = d.get('output_json') or d.get('data', {}).get('output_json') or {}
roles = out.get('roles') or []
confirmed = [
    {
        'name': r.get('name') or '角色',
        'role_type': r.get('role_type') or r.get('roleType') or '主角',
        'appearance': r.get('appearance') or '温暖绘本风格',
        'story_function': r.get('story_function') or r.get('storyFunction') or '',
        'needs_consistency': True,
    }
    for r in roles
]
base = {'title': 'E2E 走查绘本', 'theme': '学会分享', 'age_group': '中班', 'page_count': 4, 'use_scene': '课堂教学', 'style': '温暖水彩',
        'plan': {'title': 'E2E 走查绘本', 'theme': '学会分享', 'summary': '孩子们学习轮流与分享的故事',
                 'outline': ['引入冲突', '朋友表达想法', '老师给出办法', '大家一起尝试'],
                 'role_requirements': ['主角孩子', '同伴孩子', '老师', '关键道具'], 'review_points': ['情节连贯', '角色一致']}}
base['confirmed_roles'] = confirmed
print(json.dumps(base, ensure_ascii=False))
")
echo "pages input roles: $(echo "$PAGES_INPUT" | jqp "print([r['name'] for r in d['confirmed_roles']])")"
run_job storybook_pages "$PAGES_INPUT" 90

# 5. illustration for first page
BOOK2=$(curl -s "$BASE/api/workspaces/$WS/storybooks/$BOOK_ID" -H "$AUTH")
PAGE_ID=$(echo "$BOOK2" | jqp "
pages=d.get('pages') or d.get('data',{}).get('pages') or []
print(pages[0]['id'] if pages else '')")
[ -z "$PAGE_ID" ] && { echo "FAIL: no pages after storybook_pages"; echo "$BOOK2" | head -c 500; exit 1; }
echo "OK: page=$PAGE_ID"

IMG_JOB=$(curl -s -X POST "$BASE/api/workspaces/$WS/storybooks/$BOOK_ID/pages/$PAGE_ID/image-tasks" -H "$AUTH" -H "$CT" \
  -d '{"prompt":"小兔子分享胡萝卜，温暖水彩风格","reference_role_ids":[],"reference_image_urls":[]}')
IMG_JOB_ID=$(echo "$IMG_JOB" | jqp "print(d.get('id') or d.get('data',{}).get('id') or '')")
[ -z "$IMG_JOB_ID" ] && { echo "FAIL: image task: $IMG_JOB"; exit 1; }
ST=$(poll_job "$IMG_JOB_ID" 120)
echo "job page_illustration -> $ST"
[ "$ST" != "succeeded" ] && { echo "FAIL: illustration ended with $ST"; exit 1; }

curl -s "$BASE/api/workspaces/$WS/generation-jobs/$IMG_JOB_ID/image" -H "$AUTH" -o /tmp/kindleaf-e2e-img.bin
IMG_SIZE=$(stat -f%z /tmp/kindleaf-e2e-img.bin)
IMG_HEAD=$(head -c 8 /tmp/kindleaf-e2e-img.bin | od -An -tx1 | tr -d ' \n')
echo "image bytes=$IMG_SIZE head=$IMG_HEAD"
[ "${IMG_HEAD:0:8}" != "89504e47" ] && { echo "FAIL: not a PNG image"; exit 1; }

# 6. 模拟老师复核：修正分页插图描述，带入已确认角色名称（质量检查要求）
BOOK3=$(curl -s "$BASE/api/workspaces/$WS/storybooks/$BOOK_ID" -H "$AUTH")
read -r FIX_PROMPT PAGE_IDS <<<"$(echo "$BOOK3" | python3 -c "
import sys, json
d = json.load(sys.stdin); b = d.get('data') or d
roles = b.get('roles') or []
names = '、'.join(r['name'] for r in roles)
pages = sorted(b.get('pages') or [], key=lambda p: p.get('page_number') or 0)
prompt = f'幼儿园教室里，{names}一起游戏，温暖水彩风格' if names else '幼儿园教室场景，温暖水彩风格'
print(prompt + ' ' + ' '.join(p['id'] for p in pages))
")"
echo "fix prompt: $FIX_PROMPT"
for PID in $PAGE_IDS; do
  FIX_RESP=$(curl -s -X PATCH "$BASE/api/workspaces/$WS/storybooks/$BOOK_ID/pages/$PID" -H "$AUTH" -H "$CT" \
    --data-binary "$(FIX_PROMPT="$FIX_PROMPT" python3 -c "import json,os;print(json.dumps({'illustration_prompt': os.environ['FIX_PROMPT']}, ensure_ascii=False))")")
  echo "$FIX_RESP" | grep -q '"error"' && { echo "FAIL: fix page $PID: $FIX_RESP"; exit 1; }
done
echo "OK: 已修正 $(echo $PAGE_IDS | wc -w | tr -d ' ') 个分页的插图描述"

# 7. 复核后按合法状态机逐级推进到可交付（H6 之后向导只到 editing）
for NEXT in roles_pending editing exportable; do
  PATCH_RESP=$(curl -s -X PATCH "$BASE/api/workspaces/$WS/storybooks/$BOOK_ID" -H "$AUTH" -H "$CT" -d "{\"status\":\"$NEXT\"}")
  NEW_STATUS=$(echo "$PATCH_RESP" | jqp "print(d.get('status') or d.get('data',{}).get('status') or '')")
  echo "storybook status -> $NEW_STATUS"
  [ "$NEW_STATUS" != "$NEXT" ] && { echo "FAIL: transition to $NEXT: $PATCH_RESP"; exit 1; }
done

# 7. export PDF
EXP=$(curl -s -X POST "$BASE/api/workspaces/$WS/storybooks/$BOOK_ID/exports" -H "$AUTH")
EXP_ID=$(echo "$EXP" | jqp "print(d.get('id') or d.get('data',{}).get('id') or '')")
[ -z "$EXP_ID" ] && { echo "FAIL: create export: $EXP"; exit 1; }
n=0; EST=""
while [ $n -lt 120 ]; do
  E=$(curl -s "$BASE/api/workspaces/$WS/storybooks/$BOOK_ID/exports/$EXP_ID" -H "$AUTH")
  EST=$(echo "$E" | jqp "print(d.get('status') or d.get('data',{}).get('status') or '')")
  case "$EST" in queued|running|"") ;; *) break;; esac
  sleep 1; n=$((n+1))
done
echo "export -> $EST"
[ "$EST" != "succeeded" ] && { echo "FAIL: export ended with $EST: $E"; exit 1; }

curl -s "$BASE/api/workspaces/$WS/storybooks/$BOOK_ID/exports/$EXP_ID/download" -H "$AUTH" -o /tmp/kindleaf-e2e-book.pdf
PDF_SIZE=$(stat -f%z /tmp/kindleaf-e2e-book.pdf)
PDF_HEAD=$(head -c 5 /tmp/kindleaf-e2e-book.pdf)
echo "pdf bytes=$PDF_SIZE head=$PDF_HEAD"
[ "$PDF_HEAD" != "%PDF-" ] && { echo "FAIL: not a PDF"; exit 1; }

echo "ALL_E2E_PASS"
