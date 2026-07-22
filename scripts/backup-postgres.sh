#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
DB_CONTAINER="${DB_CONTAINER:-kindleaf-postgres}"
DB_USER="${DB_USER:-postgres}"
DB_NAME="${DB_NAME:-kindleaf_development}"
BACKUP_DIR="${BACKUP_DIR:-$ROOT_DIR/.tmp/backups}"
STAMP="${STAMP:-$(date +%Y%m%d-%H%M%S)}"
BACKUP_FILE="${BACKUP_FILE:-$BACKUP_DIR/${DB_NAME}-${STAMP}.dump}"

require_command() {
  local name="$1"
  if ! command -v "$name" >/dev/null 2>&1; then
    echo "command not found: $name" >&2
    exit 1
  fi
}

echo "== Kindleaf PostgreSQL backup =="
echo "DB_CONTAINER=$DB_CONTAINER"
echo "DB_NAME=$DB_NAME"
echo "BACKUP_FILE=$BACKUP_FILE"

require_command docker
mkdir -p "$(dirname "$BACKUP_FILE")"

docker exec "$DB_CONTAINER" pg_dump -U "$DB_USER" -d "$DB_NAME" -Fc --no-owner --no-acl >"$BACKUP_FILE"

if [[ ! -s "$BACKUP_FILE" ]]; then
  echo "backup file is empty: $BACKUP_FILE" >&2
  exit 1
fi

docker exec -i "$DB_CONTAINER" pg_restore -l <"$BACKUP_FILE" >/dev/null

bytes="$(wc -c <"$BACKUP_FILE" | tr -d '[:space:]')"
echo "backup_bytes=$bytes"
echo "backup_file=$BACKUP_FILE"
echo "== PostgreSQL backup ok =="
