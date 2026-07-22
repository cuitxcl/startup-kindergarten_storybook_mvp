#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
MODE="${1:-auto}"
RUN_FULL="${CHECK_SMART_RUN_FULL:-false}"

usage() {
  cat <<'USAGE'
Usage: scripts/check-smart.sh [auto|fast|api|full|demo|providers|budget|migrations|backup-restore|trial|trial-strict|trial-positive|real-required|operator-readiness|all]

Modes:
  auto        Inspect changed files and run the shortest useful verification path.
  fast        Run script syntax, mock/API guard, frontend API build, fmt, and tests.
  api         Run fast check, then API smoke on a temporary PostgreSQL database.
  full        Run fast check, then full API + UI smoke.
  demo        Run the local demo handoff gate: fast check, then full API + UI smoke.
  providers   Run provider smoke scripts for fake DeepSeek/Seedream paths.
  budget      Run generation budget smoke on a temporary PostgreSQL database.
  migrations  Run migration up/reset/up + repeated seed check.
  backup-restore
              Run PostgreSQL backup/restore verification on temporary databases.
  trial       Run trial-deployment config readiness checks without calling external providers.
  trial-strict
              Run trial-deployment config readiness as a hard gate.
  trial-positive
              Run a no-network positive trial-readiness self-test with safe fake values.
  real-required
              Run real DeepSeek, real Seedream, and real composite provider smoke as required gates.
  operator-readiness
              Run API-level positive readiness smoke with fake keys and no model calls.
  all         Run fast, migrations, API temp-db smoke, provider smoke, budget smoke, readiness checks, and full smoke.

Env:
  CHECK_SMART_RUN_FULL=true  In auto mode, include full UI smoke when frontend UI files changed.
USAGE
}

run_fast() {
  "$ROOT_DIR/scripts/check-fast.sh"
}

run_api() {
  "$ROOT_DIR/scripts/smoke-api-temp-db.sh"
}

run_full() {
  "$ROOT_DIR/scripts/smoke-all.sh"
}

run_providers() {
  "$ROOT_DIR/scripts/smoke-providers.sh"
}

run_budget() {
  "$ROOT_DIR/scripts/smoke-generation-budget.sh"
}

run_trial() {
  "$ROOT_DIR/scripts/check-trial-readiness.sh"
}

run_trial_strict() {
  "$ROOT_DIR/scripts/check-trial-readiness.sh" --strict
}

run_trial_positive() {
  "$ROOT_DIR/scripts/check-trial-readiness-positive.sh"
}

run_real_required() {
  RUN_DEEPSEEK=required \
  RUN_SEEDREAM=required \
  RUN_COMPOSITE=required \
    "$ROOT_DIR/scripts/smoke-real-providers.sh"
}

run_operator_readiness() {
  "$ROOT_DIR/scripts/smoke-operator-readiness.sh"
}

run_migrations() {
  "$ROOT_DIR/scripts/check-migrations.sh"
}

run_backup_restore() {
  "$ROOT_DIR/scripts/check-backup-restore.sh"
}

wait_for_port_free() {
  local port="$1"
  for _ in $(seq 1 40); do
    if ! command -v lsof >/dev/null 2>&1 || ! lsof -nP -iTCP:"$port" -sTCP:LISTEN >/dev/null 2>&1; then
      return 0
    fi
    sleep 0.25
  done
  echo "port did not become free before next check step: $port" >&2
  lsof -nP -iTCP:"$port" -sTCP:LISTEN >&2 || true
  return 1
}

changed_files() {
  if ! git -C "$ROOT_DIR" rev-parse --is-inside-work-tree >/dev/null 2>&1; then
    return 0
  fi

  {
    git -C "$ROOT_DIR" diff --name-only --diff-filter=ACMRTUXB HEAD -- || true
    git -C "$ROOT_DIR" ls-files --others --exclude-standard || true
  } | grep -Ev '^(frontend/(dist|node_modules)/|server/(target|tmp)/|\.tmp/|\.DS_Store$)' | sort -u
}

has_match() {
  local pattern="$1"
  grep -Eq "$pattern" <<<"$CHANGED"
}

run_auto() {
  CHANGED="$(changed_files)"

  echo "== Kindleaf smart check =="
  echo "root=$ROOT_DIR"
  echo "mode=auto"
  echo

  if [[ -z "$CHANGED" ]]; then
    echo "No changed files detected. Running fast check only."
    run_fast
    return
  fi

  echo "Changed files considered:"
  sed 's/^/  - /' <<<"$CHANGED"
  echo

  if ! has_match '^(frontend/|server/|scripts/|docs/|docker-compose\.yml|QUICKSTART\.md|README\.md|\.env(\.trial)?\.example)'; then
    echo "No project files that affect the app were detected. Skipping checks."
    return
  fi

  if ! has_match '^(frontend/|server/|scripts/|docker-compose\.yml|\.env(\.trial)?\.example)'; then
    echo "Docs-only change detected. Skipping code checks."
    return
  fi

  echo "1. fast check"
  run_fast

  if has_match '^server/migration/|^server/src/models\.rs|^docker-compose\.yml|^server/config/'; then
    echo
    echo "2. migration check"
    run_migrations
  fi

  if has_match '^scripts/(backup-postgres|restore-postgres|check-backup-restore)\.sh|^docker-compose\.yml|^docs/0[6-9]-|^QUICKSTART\.md'; then
    echo
    echo "2b. backup/restore check"
    run_backup_restore
  fi

  if has_match '^server/|^scripts/smoke-api|^scripts/check-migrations|^docker-compose\.yml|^\.env(\.trial)?\.example'; then
    echo
    echo "3. API temp-db smoke"
    run_api
    wait_for_port_free 8081
  fi

  if has_match '^server/src/services/generation_provider\.rs|^scripts/(fake-deepseek|fake-seedream-image|load-env|check-real-provider-readiness|smoke-generation-provider|smoke-generation-provider-failure|smoke-image-provider|smoke-composite-provider|smoke-providers|smoke-real-deepseek-text|smoke-real-seedream-image|smoke-real-composite-provider|smoke-real-providers)\.'; then
    echo
    echo "4. provider smoke"
    echo "Real provider scripts changed: running fake provider smoke for a quota-free safety gate."
    echo "Run ./scripts/smoke-real-providers.sh manually when you intend to consume real provider quota."
    wait_for_port_free 8081
    run_providers
    wait_for_port_free 8081
  fi

  if has_match '^server/src/repositories/generation\.rs|^server/src/repositories/audit\.rs|^server/src/controllers/api\.rs|^scripts/smoke-generation-budget\.sh|^\.env(\.trial)?\.example'; then
    echo
    echo "5. generation budget smoke"
    wait_for_port_free 8081
    run_budget
    wait_for_port_free 8081
  fi

  if has_match '^\.env(\.trial)?\.example|^docker-compose\.yml|^server/config/|^scripts/(check-trial-readiness|check-trial-readiness-positive|load-env|check-real-provider-readiness|smoke-real-[^/]+)\.|^docs/0[6-9]-|^QUICKSTART\.md'; then
    echo
    echo "6. trial readiness"
    run_trial
    echo
    echo "7. trial readiness positive self-test"
    run_trial_positive
  fi

  if has_match '^frontend/src/|^frontend/package|^frontend/vite\.config|^scripts/smoke-ui\.mjs'; then
    if [[ "$RUN_FULL" == "true" ]]; then
      echo
      echo "6. full API + UI smoke"
      wait_for_port_free 8081
      run_full
    else
      echo
      echo "Full UI smoke skipped for speed."
      echo "Run CHECK_SMART_RUN_FULL=true ./scripts/check-smart.sh auto before milestone handoff."
    fi
  fi

  echo
  echo "== smart check ok =="
}

case "$MODE" in
  auto)
    run_auto
    ;;
  fast)
    run_fast
    ;;
  api)
    run_fast
    run_api
    ;;
  full)
    run_fast
    run_full
    ;;
  demo)
    run_fast
    run_full
    ;;
  providers)
    run_providers
    ;;
  budget)
    run_budget
    ;;
  trial)
    run_trial
    ;;
  trial-strict)
    run_trial_strict
    ;;
  trial-positive)
    run_trial_positive
    ;;
  real-required)
    run_real_required
    ;;
  operator-readiness)
    run_operator_readiness
    ;;
  migrations)
    run_migrations
    ;;
  backup-restore)
    run_backup_restore
    ;;
  all)
    run_fast
    run_migrations
    run_backup_restore
    run_api
    run_providers
    run_budget
    run_trial
    run_trial_positive
    wait_for_port_free 8081
    run_operator_readiness
    wait_for_port_free 8081
    run_full
    ;;
  -h|--help|help)
    usage
    ;;
  *)
    usage >&2
    exit 2
    ;;
esac
