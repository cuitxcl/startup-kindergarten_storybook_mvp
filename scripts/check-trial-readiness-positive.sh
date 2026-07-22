#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
STORAGE_ROOT="${STORAGE_ROOT:-$ROOT_DIR/.tmp/trial-readiness-positive-storage}"

mkdir -p "$STORAGE_ROOT"

echo "== Kindleaf trial readiness positive check =="
echo "root=$ROOT_DIR"
echo "storage=$STORAGE_ROOT"
echo "This check does not start services or call external AI providers."

APP_HOST="${APP_HOST:-https://trial.kindleaf.test}" \
DATABASE_URL="${DATABASE_URL:-postgres://kindleaf_user:trial-secret-pass@db.kindleaf.test:5432/kindleaf_trial}" \
KINDLEAF_DEMO_SEED="${KINDLEAF_DEMO_SEED:-0}" \
KINDLEAF_AUTH_TOKEN_SECRET="${KINDLEAF_AUTH_TOKEN_SECRET:-kindleaf-trial-readiness-positive-secret-48chars}" \
KINDLEAF_AUTH_TOKEN_TTL_SECONDS="${KINDLEAF_AUTH_TOKEN_TTL_SECONDS:-604800}" \
KINDLEAF_GENERATION_PROVIDER="${KINDLEAF_GENERATION_PROVIDER:-}" \
DEEPSEEK_API_KEY="${DEEPSEEK_API_KEY:-sk-deepseek-trial-readiness-positive}" \
DEEPSEEK_BASE_URL="${DEEPSEEK_BASE_URL:-https://api.deepseek.com}" \
DEEPSEEK_ENDPOINT_PATH="${DEEPSEEK_ENDPOINT_PATH:-/chat/completions}" \
DEEPSEEK_MODEL="${DEEPSEEK_MODEL:-deepseek-v4-flash}" \
SEEDREAM_API_KEY="${SEEDREAM_API_KEY:-}" \
ARK_API_KEY="${ARK_API_KEY:-sk-ark-trial-readiness-positive}" \
SEEDREAM_BASE_URL="${SEEDREAM_BASE_URL:-https://ark.cn-beijing.volces.com}" \
ARK_BASE_URL="${ARK_BASE_URL:-}" \
SEEDREAM_ENDPOINT_PATH="${SEEDREAM_ENDPOINT_PATH:-/api/v3/images/generations}" \
ARK_IMAGE_ENDPOINT_PATH="${ARK_IMAGE_ENDPOINT_PATH:-}" \
SEEDREAM_IMAGE_MODEL="${SEEDREAM_IMAGE_MODEL:-doubao-seedream-5-0-lite}" \
ARK_IMAGE_MODEL="${ARK_IMAGE_MODEL:-}" \
KINDLEAF_COST_CURRENCY="${KINDLEAF_COST_CURRENCY:-USD}" \
KINDLEAF_COST_BUDGET_LIMIT_MICROS="${KINDLEAF_COST_BUDGET_LIMIT_MICROS:-2000000}" \
KINDLEAF_COST_BUDGET_WARNING_PERCENT="${KINDLEAF_COST_BUDGET_WARNING_PERCENT:-80}" \
KINDLEAF_STORAGE_ROOT="$STORAGE_ROOT" \
KINDLEAF_EXPORTS_DIR="${KINDLEAF_EXPORTS_DIR:-}" \
KINDLEAF_GENERATED_IMAGES_DIR="${KINDLEAF_GENERATED_IMAGES_DIR:-}" \
KINDLEAF_EXPORT_MAX_BYTES="${KINDLEAF_EXPORT_MAX_BYTES:-52428800}" \
KINDLEAF_GENERATED_IMAGE_MAX_BYTES="${KINDLEAF_GENERATED_IMAGE_MAX_BYTES:-15728640}" \
  "$ROOT_DIR/scripts/check-trial-readiness.sh" --strict

echo "== trial readiness positive check ok =="
