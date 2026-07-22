#!/usr/bin/env bash
set -euo pipefail

DB_CONTAINER="${DB_CONTAINER:-kindleaf-postgres}"
DB_USER="${DB_USER:-postgres}"
DB_NAME="${DB_NAME:-kindleaf_restore_$(date +%s)}"
RESTORE_DROP_EXISTING="${RESTORE_DROP_EXISTING:-false}"
RESTORE_CONFIRM="${RESTORE_CONFIRM:-}"
BACKUP_FILE="${1:-${BACKUP_FILE:-}}"

usage() {
  cat <<'MESSAGE'
Usage:
  ./scripts/restore-postgres.sh <backup.dump>

Environment:
  DB_CONTAINER=kindleaf-postgres
  DB_USER=postgres
  DB_NAME=kindleaf_restore_<timestamp>
  RESTORE_DROP_EXISTING=false
  RESTORE_CONFIRM=<DB_NAME>    Required when RESTORE_DROP_EXISTING=true.

By default this restores into a new timestamped database and refuses to overwrite an existing one.
MESSAGE
}

if [[ -z "$BACKUP_FILE" || "$BACKUP_FILE" == "-h" || "$BACKUP_FILE" == "--help" ]]; then
  usage
  exit 2
fi

if [[ ! -s "$BACKUP_FILE" ]]; then
  echo "backup file does not exist or is empty: $BACKUP_FILE" >&2
  exit 1
fi

echo "== Kindleaf PostgreSQL restore =="
echo "DB_CONTAINER=$DB_CONTAINER"
echo "DB_NAME=$DB_NAME"
echo "BACKUP_FILE=$BACKUP_FILE"

if ! command -v docker >/dev/null 2>&1; then
  echo "command not found: docker" >&2
  exit 1
fi

docker exec -i "$DB_CONTAINER" pg_restore -l <"$BACKUP_FILE" >/dev/null

exists="$(docker exec "$DB_CONTAINER" psql -U "$DB_USER" -d postgres -tAc "select 1 from pg_database where datname = '$DB_NAME'" | tr -d '[:space:]')"
if [[ "$exists" == "1" ]]; then
  if [[ "$RESTORE_DROP_EXISTING" != "true" ]]; then
    echo "target database already exists: $DB_NAME" >&2
    echo "Set RESTORE_DROP_EXISTING=true RESTORE_CONFIRM=$DB_NAME to replace it." >&2
    exit 1
  fi
  if [[ "$RESTORE_CONFIRM" != "$DB_NAME" ]]; then
    echo "restore confirmation mismatch; set RESTORE_CONFIRM=$DB_NAME" >&2
    exit 1
  fi
  docker exec "$DB_CONTAINER" psql -U "$DB_USER" -d postgres -v ON_ERROR_STOP=1 >/dev/null <<SQL
select pg_terminate_backend(pid)
from pg_stat_activity
where datname = '$DB_NAME'
  and pid <> pg_backend_pid();
SQL
  docker exec "$DB_CONTAINER" dropdb --force -U "$DB_USER" "$DB_NAME"
fi

docker exec "$DB_CONTAINER" createdb -U "$DB_USER" "$DB_NAME"
docker exec -i "$DB_CONTAINER" pg_restore -U "$DB_USER" -d "$DB_NAME" --no-owner --no-acl <"$BACKUP_FILE"

echo "restored_database=$DB_NAME"
echo "== PostgreSQL restore ok =="
