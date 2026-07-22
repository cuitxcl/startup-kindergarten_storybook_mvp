#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
source "$ROOT_DIR/scripts/load-env.sh"
kindleaf_load_env_files "$ROOT_DIR"

STRICT=0

usage() {
  cat <<'MESSAGE'
Usage:
  ./scripts/check-trial-readiness.sh [--strict]

Checks trial-deployment configuration without starting services or calling external AI providers.

Default mode prints warnings for incomplete trial configuration.
--strict turns trial-risk warnings into failures.
MESSAGE
}

while [[ $# -gt 0 ]]; do
  case "$1" in
    --strict)
      STRICT=1
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

warnings=0
failures=0

ok() {
  echo "ok: $1"
}

warn() {
  echo "warning: $1" >&2
  warnings=$((warnings + 1))
}

fail() {
  echo "failed: $1" >&2
  failures=$((failures + 1))
}

risk() {
  if [[ "$STRICT" -eq 1 ]]; then
    fail "$1"
  else
    warn "$1"
  fi
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

validate_non_empty() {
  local label="$1"
  local value="$2"
  if [[ -n "${value//[[:space:]]/}" ]]; then
    ok "$label=$value"
  else
    fail "$label must not be empty"
  fi
}

first_non_empty() {
  local fallback="$1"
  shift
  local value
  for value in "$@"; do
    if [[ -n "${value//[[:space:]]/}" ]]; then
      printf '%s' "$value"
      return
    fi
  done
  printf '%s' "$fallback"
}

check_secret_ready() {
  local label="$1"
  local value="$2"
  local missing_message="$3"
  if [[ -z "${value//[[:space:]]/}" ]]; then
    risk "$missing_message"
  elif kindleaf_looks_like_placeholder_secret "$value"; then
    risk "$label looks like a placeholder; replace it with the real trial secret"
  else
    ok "$label is set"
  fi
}

is_local_url() {
  node - "$1" <<'NODE'
const value = process.argv[2] || "";
try {
  const url = new URL(value);
  process.exit(["localhost", "127.0.0.1", "0.0.0.0"].includes(url.hostname) ? 0 : 1);
} catch {
  process.exit(1);
}
NODE
}

is_placeholder_host_url() {
  node - "$1" <<'NODE'
const value = process.argv[2] || "";
try {
  const hostname = new URL(value).hostname.toLowerCase();
  process.exit(
    hostname === "example.com" ||
    hostname.endsWith(".example.com") ||
    hostname === "example.org" ||
    hostname.endsWith(".example.org") ||
    hostname === "example.net" ||
    hostname.endsWith(".example.net")
      ? 0
      : 1
  );
} catch {
  process.exit(1);
}
NODE
}

contains_local_database() {
  local value="$1"
  [[ "$value" == *"localhost"* || "$value" == *"127.0.0.1"* || "$value" == *"postgres:postgres"* ]]
}

contains_placeholder_database_value() {
  local normalized
  normalized="$(printf '%s' "$1" | tr '[:upper:]' '[:lower:]' | tr -d '[:space:]' | tr '_' '-')"
  [[ "$normalized" == *"change-me"* || "$normalized" == *"replace-"* || "$normalized" == *"your-"* || "$normalized" == *"example.com"* ]]
}

check_storage() {
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
    const file = path.join(dir, `.kindleaf-trial-readiness-${process.pid}`);
    try {
      fs.writeFileSync(file, "ok");
      const bytes = fs.readFileSync(file, "utf8");
      if (bytes !== "ok") {
        throw new Error(`probe content mismatch: ${file}`);
      }
    } finally {
      try {
        fs.unlinkSync(file);
      } catch (error) {
        if (error && error.code !== "ENOENT") throw error;
      }
    }
  }
} catch (error) {
  const detail = error && error.message ? error.message : String(error);
  console.error(`storage probe failed: ${detail}`);
  process.exit(1);
}
console.log(JSON.stringify({ root, exportsDir, imagesDir, exportMaxBytes, imageMaxBytes }));
NODE
}

echo "== Kindleaf trial readiness check =="
echo "root=$ROOT_DIR"
echo "strict=$([[ "$STRICT" -eq 1 ]] && echo true || echo false)"

require_command bash
require_command node
require_command cargo

app_host="${APP_HOST:-http://127.0.0.1}"
database_url="${DATABASE_URL:-postgres://postgres:postgres@localhost:5432/kindleaf_production}"
provider="${KINDLEAF_GENERATION_PROVIDER-mock}"
storage_root="${KINDLEAF_STORAGE_ROOT:-tmp}"
budget_limit="${KINDLEAF_COST_BUDGET_LIMIT_MICROS:-}"
budget_warning="${KINDLEAF_COST_BUDGET_WARNING_PERCENT:-80}"
auth_token_secret="${KINDLEAF_AUTH_TOKEN_SECRET:-}"
auth_token_ttl="${KINDLEAF_AUTH_TOKEN_TTL_SECONDS:-604800}"
max_auth_token_ttl=2592000
deepseek_base_url="$(first_non_empty "https://api.deepseek.com" "${DEEPSEEK_BASE_URL:-}")"
deepseek_endpoint_path="$(first_non_empty "/chat/completions" "${DEEPSEEK_ENDPOINT_PATH:-}")"
deepseek_model="$(first_non_empty "deepseek-v4-flash" "${DEEPSEEK_MODEL:-}")"
seedream_base_url="$(first_non_empty "https://ark.cn-beijing.volces.com" "${SEEDREAM_BASE_URL:-}" "${ARK_BASE_URL:-}")"
seedream_endpoint_path="$(first_non_empty "/api/v3/images/generations" "${SEEDREAM_ENDPOINT_PATH:-}" "${ARK_IMAGE_ENDPOINT_PATH:-}")"
seedream_model="$(first_non_empty "doubao-seedream-5-0-lite" "${SEEDREAM_IMAGE_MODEL:-}" "${ARK_IMAGE_MODEL:-}")"

validate_url "APP_HOST" "$app_host"
if is_local_url "$app_host"; then
  risk "APP_HOST points to a local address; set a real HTTPS host before external trial"
elif is_placeholder_host_url "$app_host"; then
  risk "APP_HOST uses an example domain; replace it with the real trial HTTPS host"
elif [[ "$app_host" == https://* ]]; then
  ok "APP_HOST uses HTTPS"
else
  risk "APP_HOST does not use HTTPS"
fi

if [[ -z "$database_url" ]]; then
  fail "DATABASE_URL is empty"
else
  ok "DATABASE_URL is set"
  if contains_local_database "$database_url"; then
    risk "DATABASE_URL looks like local/default PostgreSQL; use managed or secured database credentials for trial"
  fi
  if contains_placeholder_database_value "$database_url"; then
    risk "DATABASE_URL contains placeholder values; replace host, user, password and database with real trial credentials"
  fi
fi

case "$provider" in
  mock|deepseek|seedream|"")
    ok "KINDLEAF_GENERATION_PROVIDER=${provider:-auto-composite}"
    ;;
  *)
    fail "KINDLEAF_GENERATION_PROVIDER is unsupported: $provider"
    ;;
esac

if [[ "$provider" == "mock" ]]; then
  risk "KINDLEAF_GENERATION_PROVIDER=mock; trial will not use real generation"
fi

check_secret_ready "DEEPSEEK_API_KEY" "${DEEPSEEK_API_KEY:-}" "DEEPSEEK_API_KEY is missing; real text generation cannot run"
validate_url "DEEPSEEK_BASE_URL" "$deepseek_base_url"
validate_endpoint_path "DEEPSEEK_ENDPOINT_PATH" "$deepseek_endpoint_path"
validate_non_empty "DEEPSEEK_MODEL" "$deepseek_model"

if [[ -n "${SEEDREAM_API_KEY:-}" ]]; then
  check_secret_ready "SEEDREAM_API_KEY" "${SEEDREAM_API_KEY:-}" "SEEDREAM_API_KEY is missing; real Seedream image generation cannot run"
elif [[ -n "${ARK_API_KEY:-}" ]]; then
  check_secret_ready "ARK_API_KEY" "${ARK_API_KEY:-}" "ARK_API_KEY is missing; real Seedream image generation cannot run"
else
  risk "SEEDREAM_API_KEY or ARK_API_KEY is missing; real Seedream image generation cannot run"
fi
validate_url "Seedream base URL" "$seedream_base_url"
validate_endpoint_path "Seedream endpoint path" "$seedream_endpoint_path"
validate_non_empty "Seedream image model" "$seedream_model"

if kindleaf_looks_like_placeholder_secret "$auth_token_secret"; then
  risk "KINDLEAF_AUTH_TOKEN_SECRET looks like a placeholder; replace it with a real random secret"
elif [[ "${#auth_token_secret}" -ge 32 ]]; then
  ok "KINDLEAF_AUTH_TOKEN_SECRET is set"
else
  risk "KINDLEAF_AUTH_TOKEN_SECRET is missing or shorter than 32 characters; login will use development dev-token"
fi

if [[ "$auth_token_ttl" =~ ^[0-9]+$ && "$auth_token_ttl" -ge 1 && "$auth_token_ttl" -le "$max_auth_token_ttl" ]]; then
  ok "KINDLEAF_AUTH_TOKEN_TTL_SECONDS=$auth_token_ttl"
else
  risk "KINDLEAF_AUTH_TOKEN_TTL_SECONDS must be an integer from 1 to $max_auth_token_ttl seconds"
fi

storage_json="$(check_storage)" || fail "storage directories are not writable"
if [[ -n "${storage_json:-}" ]]; then
  echo "ok: storage $storage_json"
  if [[ "$storage_root" == "tmp" || "$storage_root" == "/tmp"* ]]; then
    risk "KINDLEAF_STORAGE_ROOT=$storage_root is temporary; use a persistent path for trial"
  fi
fi

if [[ "$budget_warning" =~ ^[0-9]+$ && "$budget_warning" -ge 1 && "$budget_warning" -le 100 ]]; then
  ok "KINDLEAF_COST_BUDGET_WARNING_PERCENT=$budget_warning"
else
  fail "KINDLEAF_COST_BUDGET_WARNING_PERCENT must be an integer from 1 to 100"
fi

if [[ -n "$budget_limit" ]]; then
  if [[ "$budget_limit" =~ ^[0-9]+$ && "$budget_limit" -gt 0 ]]; then
    ok "KINDLEAF_COST_BUDGET_LIMIT_MICROS=$budget_limit"
  else
    fail "KINDLEAF_COST_BUDGET_LIMIT_MICROS must be empty or a positive integer"
  fi
else
  risk "KINDLEAF_COST_BUDGET_LIMIT_MICROS is empty; generation budget is not capped"
fi

if [[ "${KINDLEAF_DEMO_SEED:-}" == "1" || "${KINDLEAF_DEMO_SEED:-}" == "true" ]]; then
  risk "KINDLEAF_DEMO_SEED is enabled; do not seed demo users into a real trial database"
else
  ok "KINDLEAF_DEMO_SEED is not enabled"
fi

if [[ "$failures" -gt 0 ]]; then
  echo "== trial readiness failed: $failures issue(s), $warnings warning(s) ==" >&2
  exit 1
fi

if [[ "$warnings" -gt 0 ]]; then
  echo "== trial readiness completed with $warnings warning(s) =="
else
  echo "== trial readiness ok =="
fi
