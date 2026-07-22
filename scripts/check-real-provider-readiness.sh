#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
source "$ROOT_DIR/scripts/load-env.sh"
kindleaf_load_env_files "$ROOT_DIR"

DB_CONTAINER="${DB_CONTAINER:-kindleaf-postgres}"
DB_USER="${DB_USER:-postgres}"
DB_HOST="${DB_HOST:-127.0.0.1}"
DB_PORT="${DB_PORT:-55432}"
API_PORT="${API_PORT:-8081}"
MODE="composite"
ALLOW_MISSING_KEYS=0

usage() {
  cat <<'MESSAGE'
Usage:
  ./scripts/check-real-provider-readiness.sh [--deepseek|--seedream|--composite] [--allow-missing-keys]

Checks local prerequisites for real provider smoke scripts without calling real AI providers.
Obvious placeholder keys such as "your-api-key" are treated as missing keys.

Modes:
  --deepseek    Require DEEPSEEK_API_KEY and check text-provider smoke prerequisites.
  --seedream   Require SEEDREAM_API_KEY or ARK_API_KEY and check image-provider smoke prerequisites.
  --composite  Require DeepSeek + Seedream/ARK keys. This is the default.

Options:
  --allow-missing-keys  Report missing keys as warnings instead of exiting with code 2.
MESSAGE
}

while [[ $# -gt 0 ]]; do
  case "$1" in
    --deepseek)
      MODE="deepseek"
      ;;
    --seedream)
      MODE="seedream"
      ;;
    --composite)
      MODE="composite"
      ;;
    --allow-missing-keys)
      ALLOW_MISSING_KEYS=1
      ;;
    -h|--help)
      usage
      exit 0
      ;;
    *)
      echo "unknown option: $1" >&2
      usage >&2
      exit 1
      ;;
  esac
  shift
done

failures=0
missing_keys=0

ok() {
  echo "ok: $1"
}

warn() {
  echo "warning: $1" >&2
}

fail() {
  echo "failed: $1" >&2
  failures=$((failures + 1))
}

missing_key() {
  echo "missing key: $1" >&2
  missing_keys=$((missing_keys + 1))
}

provider_key_ready() {
  local label="$1"
  local value="$2"
  if [[ -z "${value//[[:space:]]/}" ]]; then
    missing_key "$label"
    return 1
  fi
  if kindleaf_looks_like_placeholder_secret "$value"; then
    missing_key "$label placeholder"
    return 1
  fi
  ok "$label is set"
  return 0
}

require_command() {
  local name="$1"
  if command -v "$name" >/dev/null 2>&1; then
    ok "command $name"
  else
    fail "command not found: $name"
  fi
}

validate_url() {
  local label="$1"
  local value="$2"
  if node -e "new URL(process.argv[1])" "$value" >/dev/null 2>&1; then
    ok "$label=$value"
  else
    fail "$label is not a valid URL: $value"
  fi
}

validate_endpoint_path() {
  local label="$1"
  local value="$2"
  if [[ "$value" == http://* || "$value" == https://* ]]; then
    validate_url "$label" "$value"
    return
  fi
  if [[ "$value" == /* ]]; then
    ok "$label=$value"
  else
    fail "$label must be an absolute path or URL: $value"
  fi
}

check_port_free() {
  local port="$1"
  if command -v lsof >/dev/null 2>&1 && lsof -nP -iTCP:"$port" -sTCP:LISTEN >/dev/null 2>&1; then
    fail "API_PORT $port is already in use"
  else
    ok "API_PORT $port is available"
  fi
}

check_db_container() {
  if ! command -v docker >/dev/null 2>&1; then
    return
  fi
  if ! docker inspect "$DB_CONTAINER" >/dev/null 2>&1; then
    fail "Docker container not found: $DB_CONTAINER"
    return
  fi
  local running
  running="$(docker inspect -f '{{.State.Running}}' "$DB_CONTAINER" 2>/dev/null || true)"
  if [[ "$running" != "true" ]]; then
    fail "Docker container is not running: $DB_CONTAINER"
    return
  fi
  ok "Docker container $DB_CONTAINER is running"
  if docker exec "$DB_CONTAINER" psql -U "$DB_USER" -d postgres -tAc "select 1" >/dev/null 2>&1; then
    ok "PostgreSQL accepts local smoke connections on $DB_HOST:$DB_PORT"
  else
    fail "PostgreSQL check failed in container $DB_CONTAINER"
  fi
}

check_storage_write() {
  KINDLEAF_STORAGE_ROOT="${KINDLEAF_STORAGE_ROOT:-tmp}" \
  KINDLEAF_EXPORTS_DIR="${KINDLEAF_EXPORTS_DIR:-}" \
  KINDLEAF_GENERATED_IMAGES_DIR="${KINDLEAF_GENERATED_IMAGES_DIR:-}" \
  KINDLEAF_EXPORT_MAX_BYTES="${KINDLEAF_EXPORT_MAX_BYTES:-52428800}" \
  KINDLEAF_GENERATED_IMAGE_MAX_BYTES="${KINDLEAF_GENERATED_IMAGE_MAX_BYTES:-15728640}" \
  node <<'NODE'
const fs = require("fs");
const path = require("path");

const root = process.env.KINDLEAF_STORAGE_ROOT || "tmp";
const exportsDir = process.env.KINDLEAF_EXPORTS_DIR || path.join(root, "exports");
const imagesDir = process.env.KINDLEAF_GENERATED_IMAGES_DIR || path.join(root, "generated-images");
const parseLimit = (value, fallback) => {
  const parsed = Number.parseInt(String(value || "").trim(), 10);
  return Number.isFinite(parsed) && parsed >= 0 ? parsed : fallback;
};
const exportMaxBytes = parseLimit(process.env.KINDLEAF_EXPORT_MAX_BYTES, 52428800);
const imageMaxBytes = parseLimit(process.env.KINDLEAF_GENERATED_IMAGE_MAX_BYTES, 15728640);
try {
  for (const dir of [exportsDir, imagesDir]) {
    fs.mkdirSync(dir, { recursive: true });
    const file = path.join(dir, `.kindleaf-preflight-${process.pid}`);
    fs.writeFileSync(file, "ok");
    fs.unlinkSync(file);
  }
} catch (error) {
  const detail = error && error.message ? error.message : String(error);
  console.error(`storage probe failed: ${detail}`);
  process.exit(1);
}
console.log(`ok: storage writable exports=${exportsDir} generated-images=${imagesDir}`);
console.log(`ok: storage limits export_max_bytes=${exportMaxBytes} generated_image_max_bytes=${imageMaxBytes}`);
console.log("ok: storage filename validation=uuid-pdf, mock|seedream-uuid-png");
NODE
}

echo "== Kindleaf real provider readiness check =="
echo "mode=$MODE"
echo "root=$ROOT_DIR"
echo "API_PORT=$API_PORT"
echo "DB_CONTAINER=$DB_CONTAINER"

require_command bash
require_command cargo
require_command node
require_command curl
require_command docker

if [[ "$MODE" == "deepseek" || "$MODE" == "composite" ]]; then
  provider_key_ready "DEEPSEEK_API_KEY" "${DEEPSEEK_API_KEY:-}" || true
  validate_url "DEEPSEEK_BASE_URL" "${DEEPSEEK_BASE_URL:-https://api.deepseek.com}"
  validate_endpoint_path "DEEPSEEK_ENDPOINT_PATH" "${DEEPSEEK_ENDPOINT_PATH:-/chat/completions}"
  ok "DEEPSEEK_MODEL=${DEEPSEEK_MODEL:-deepseek-v4-flash}"
fi

if [[ "$MODE" == "seedream" || "$MODE" == "composite" ]]; then
  seedream_base_url="${SEEDREAM_BASE_URL:-${ARK_BASE_URL:-https://ark.cn-beijing.volces.com}}"
  seedream_endpoint_path="${SEEDREAM_ENDPOINT_PATH:-${ARK_IMAGE_ENDPOINT_PATH:-/api/v3/images/generations}}"
  seedream_model="${SEEDREAM_IMAGE_MODEL:-${ARK_IMAGE_MODEL:-doubao-seedream-5-0-lite}}"
  if [[ -n "${SEEDREAM_API_KEY:-}" ]]; then
    provider_key_ready "SEEDREAM_API_KEY" "${SEEDREAM_API_KEY:-}" || true
  elif [[ -n "${ARK_API_KEY:-}" ]]; then
    provider_key_ready "ARK_API_KEY" "${ARK_API_KEY:-}" || true
  else
    provider_key_ready "SEEDREAM_API_KEY or ARK_API_KEY" "" || true
  fi
  validate_url "Seedream base URL" "$seedream_base_url"
  validate_endpoint_path "Seedream endpoint path" "$seedream_endpoint_path"
  ok "Seedream image model=$seedream_model"
fi

check_port_free "$API_PORT"
check_db_container
check_storage_write || fail "storage write check failed"

if [[ "$missing_keys" -gt 0 && "$ALLOW_MISSING_KEYS" -eq 0 ]]; then
  echo "== readiness failed: missing provider keys ==" >&2
  exit 2
fi

if [[ "$missing_keys" -gt 0 ]]; then
  warn "provider keys are missing, but --allow-missing-keys was set"
fi

if [[ "$failures" -gt 0 ]]; then
  echo "== readiness failed: $failures local prerequisite issue(s) ==" >&2
  exit 1
fi

echo "== real provider readiness ok =="
