#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

echo "== Kindleaf fast check =="
echo "root=$ROOT_DIR"

echo
echo "1. shell script syntax"
bash -n \
  "$ROOT_DIR/scripts/check-fast.sh" \
  "$ROOT_DIR/scripts/check-smart.sh" \
  "$ROOT_DIR/scripts/check-migrations.sh" \
  "$ROOT_DIR/scripts/backup-postgres.sh" \
  "$ROOT_DIR/scripts/restore-postgres.sh" \
  "$ROOT_DIR/scripts/check-backup-restore.sh" \
  "$ROOT_DIR/scripts/smoke-all.sh" \
  "$ROOT_DIR/scripts/smoke-api.sh" \
  "$ROOT_DIR/scripts/smoke-api-temp-db.sh" \
  "$ROOT_DIR/scripts/smoke-operator-readiness.sh" \
  "$ROOT_DIR/scripts/check-trial-readiness.sh" \
  "$ROOT_DIR/scripts/check-trial-readiness-positive.sh" \
  "$ROOT_DIR/scripts/smoke-generation-provider.sh" \
  "$ROOT_DIR/scripts/smoke-generation-provider-failure.sh" \
  "$ROOT_DIR/scripts/smoke-generation-budget.sh" \
  "$ROOT_DIR/scripts/check-real-provider-readiness.sh" \
  "$ROOT_DIR/scripts/smoke-real-deepseek-text.sh" \
  "$ROOT_DIR/scripts/smoke-image-provider.sh" \
  "$ROOT_DIR/scripts/smoke-real-seedream-image.sh" \
  "$ROOT_DIR/scripts/smoke-composite-provider.sh" \
  "$ROOT_DIR/scripts/smoke-providers.sh" \
  "$ROOT_DIR/scripts/smoke-real-composite-provider.sh" \
  "$ROOT_DIR/scripts/smoke-real-providers.sh" \
  "$ROOT_DIR/scripts/load-env.sh"

echo
echo "2. node script syntax"
node --check "$ROOT_DIR/scripts/audit-frontend-mock-usage.mjs"
node --check "$ROOT_DIR/scripts/fake-deepseek.mjs"
node --check "$ROOT_DIR/scripts/fake-seedream-image.mjs"
node --check "$ROOT_DIR/scripts/smoke-ui.mjs"
node --check "$ROOT_DIR/scripts/validate-png.mjs"

echo
echo "3. docker compose config"
if command -v docker >/dev/null 2>&1; then
  docker compose -f "$ROOT_DIR/docker-compose.yml" config >/tmp/kindleaf-fast-compose.yml
  docker compose -f "$ROOT_DIR/docker-compose.yml" --profile app config >/tmp/kindleaf-fast-compose-app.yml
  echo "compose ok"
else
  echo "docker not found; skipping compose config check"
fi

echo
echo "4. frontend mock/API guard"
"$ROOT_DIR/scripts/audit-frontend-mock-usage.mjs" --strict >/tmp/kindleaf-fast-mock-audit.log
tail -1 /tmp/kindleaf-fast-mock-audit.log

echo
echo "5. frontend API build"
npm --prefix "$ROOT_DIR/frontend" run build:api

echo
echo "6. backend format"
cargo fmt --manifest-path "$ROOT_DIR/server/Cargo.toml" --all -- --check

echo
echo "7. backend tests"
cargo test --manifest-path "$ROOT_DIR/server/Cargo.toml" --features db --quiet

echo
echo "== fast check ok =="
