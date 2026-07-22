#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
DB_CONTAINER="${DB_CONTAINER:-kindleaf-postgres}"
DB_USER="${DB_USER:-postgres}"
DB_PASSWORD="${DB_PASSWORD:-postgres}"
DB_HOST="${DB_HOST:-127.0.0.1}"
DB_PORT="${DB_PORT:-55432}"
SOURCE_DB="${SOURCE_DB:-kindleaf_backup_source_$(date +%s)}"
RESTORE_DB="${RESTORE_DB:-kindleaf_backup_restore_$(date +%s)}"
BACKUP_DIR="${BACKUP_DIR:-$ROOT_DIR/.tmp/backups}"
BACKUP_FILE="$BACKUP_DIR/${SOURCE_DB}.dump"
SOURCE_DATABASE_URL="postgres://$DB_USER:$DB_PASSWORD@$DB_HOST:$DB_PORT/$SOURCE_DB"

cleanup() {
  for db in "$SOURCE_DB" "$RESTORE_DB"; do
    docker exec "$DB_CONTAINER" psql -U "$DB_USER" -d postgres -v ON_ERROR_STOP=1 >/dev/null 2>&1 <<SQL || true
select pg_terminate_backend(pid)
from pg_stat_activity
where datname = '$db'
  and pid <> pg_backend_pid();
SQL
    docker exec "$DB_CONTAINER" dropdb --force -U "$DB_USER" "$db" >/dev/null 2>&1 || true
  done
}

count_rows() {
  local db="$1"
  local table="$2"
  docker exec "$DB_CONTAINER" psql -U "$DB_USER" -d "$db" -tAc "select count(*) from $table" | tr -d '[:space:]'
}

echo "== Kindleaf backup/restore check =="
echo "DB_CONTAINER=$DB_CONTAINER"
echo "SOURCE_DB=$SOURCE_DB"
echo "RESTORE_DB=$RESTORE_DB"
echo "BACKUP_FILE=$BACKUP_FILE"

trap cleanup EXIT
mkdir -p "$BACKUP_DIR"

docker exec "$DB_CONTAINER" dropdb -U "$DB_USER" "$SOURCE_DB" >/dev/null 2>&1 || true
docker exec "$DB_CONTAINER" createdb -U "$DB_USER" "$SOURCE_DB"

echo "1. migrate and seed source database"
(
  cd "$ROOT_DIR/server"
  DATABASE_URL="$SOURCE_DATABASE_URL" cargo run --features db -- -e test db migrate
  DATABASE_URL="$SOURCE_DATABASE_URL" cargo run --features db -- db seed
) >/tmp/kindleaf-backup-restore-seed.log 2>&1

source_users="$(count_rows "$SOURCE_DB" users)"
source_workspaces="$(count_rows "$SOURCE_DB" workspaces)"
source_storybooks="$(count_rows "$SOURCE_DB" storybooks)"

echo "2. backup source database"
DB_CONTAINER="$DB_CONTAINER" DB_USER="$DB_USER" DB_NAME="$SOURCE_DB" BACKUP_FILE="$BACKUP_FILE" "$ROOT_DIR/scripts/backup-postgres.sh" >/tmp/kindleaf-backup-restore-backup.log

echo "3. restore into target database"
DB_CONTAINER="$DB_CONTAINER" DB_USER="$DB_USER" DB_NAME="$RESTORE_DB" "$ROOT_DIR/scripts/restore-postgres.sh" "$BACKUP_FILE" >/tmp/kindleaf-backup-restore-restore.log

echo "4. compare core table counts"
restore_users="$(count_rows "$RESTORE_DB" users)"
restore_workspaces="$(count_rows "$RESTORE_DB" workspaces)"
restore_storybooks="$(count_rows "$RESTORE_DB" storybooks)"

if [[ "$source_users" != "$restore_users" || "$source_workspaces" != "$restore_workspaces" || "$source_storybooks" != "$restore_storybooks" ]]; then
  echo "backup restore count mismatch" >&2
  echo "source users/workspaces/storybooks=$source_users/$source_workspaces/$source_storybooks" >&2
  echo "restore users/workspaces/storybooks=$restore_users/$restore_workspaces/$restore_storybooks" >&2
  exit 1
fi

echo "verified_counts=users:$restore_users workspaces:$restore_workspaces storybooks:$restore_storybooks"
echo "== backup/restore check ok =="
