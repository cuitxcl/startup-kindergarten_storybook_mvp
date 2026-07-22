#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
DB_CONTAINER="${DB_CONTAINER:-kindleaf-postgres}"
DB_USER="${DB_USER:-postgres}"
DB_PASSWORD="${DB_PASSWORD:-postgres}"
DB_HOST="${DB_HOST:-127.0.0.1}"
DB_PORT="${DB_PORT:-55432}"
DB_NAME="${DB_NAME:-kindleaf_migration_check_$(date +%s)}"
DATABASE_URL="${DATABASE_URL:-postgres://$DB_USER:$DB_PASSWORD@$DB_HOST:$DB_PORT/$DB_NAME}"
KEEP_DB="${KEEP_DB:-false}"

cleanup() {
  if [[ "$KEEP_DB" != "true" ]]; then
    docker exec "$DB_CONTAINER" dropdb -U "$DB_USER" "$DB_NAME" >/dev/null 2>&1 || true
  fi
}

run_migration() {
  DATABASE_URL="$DATABASE_URL" cargo run --manifest-path "$ROOT_DIR/server/Cargo.toml" -p migration -- "$@"
}

echo "== Kindleaf migration check =="
echo "DB_CONTAINER=$DB_CONTAINER"
echo "DB_NAME=$DB_NAME"

docker exec "$DB_CONTAINER" dropdb -U "$DB_USER" "$DB_NAME" >/dev/null 2>&1 || true
docker exec "$DB_CONTAINER" createdb -U "$DB_USER" "$DB_NAME"
trap cleanup EXIT

echo "1. initial status"
run_migration status

echo "2. migrate up"
run_migration up
run_migration status

echo "3. reset all migrations"
run_migration reset
run_migration status

echo "4. replay migrations"
run_migration up
run_migration status

echo "5. seed twice after replay"
(
  cd "$ROOT_DIR/server"
  DATABASE_URL="$DATABASE_URL" cargo run --features db -- db seed
  DATABASE_URL="$DATABASE_URL" cargo run --features db -- db seed
)

echo "== migration check ok =="
