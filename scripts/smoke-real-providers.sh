#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
source "$ROOT_DIR/scripts/load-env.sh"
kindleaf_load_env_files "$ROOT_DIR"

RUN_DEEPSEEK="${RUN_DEEPSEEK:-auto}"
RUN_SEEDREAM="${RUN_SEEDREAM:-auto}"
RUN_COMPOSITE="${RUN_COMPOSITE:-auto}"

has_deepseek_key() {
  kindleaf_real_secret_configured "${DEEPSEEK_API_KEY:-}"
}

has_seedream_key() {
  kindleaf_real_secret_configured "${SEEDREAM_API_KEY:-}" || kindleaf_real_secret_configured "${ARK_API_KEY:-}"
}

should_run() {
  local flag="$1"
  local available="$2"
  case "$flag" in
    1|true|yes|required)
      [[ "$available" == "1" ]] || return 2
      return 0
      ;;
    0|false|no|skip)
      return 1
      ;;
    auto|"")
      [[ "$available" == "1" ]]
      ;;
    *)
      echo "invalid run flag: $flag" >&2
      return 3
      ;;
  esac
}

wait_for_port_free() {
  local port="$1"
  for _ in $(seq 1 40); do
    if ! command -v lsof >/dev/null 2>&1 || ! lsof -nP -iTCP:"$port" -sTCP:LISTEN >/dev/null 2>&1; then
      return 0
    fi
    sleep 0.25
  done
  echo "port did not become free after smoke step: $port" >&2
  lsof -nP -iTCP:"$port" -sTCP:LISTEN >&2 || true
  return 1
}

deepseek_available=0
seedream_available=0
has_deepseek_key && deepseek_available=1
has_seedream_key && seedream_available=1

echo "== Kindleaf real provider smoke suite =="
echo "This suite may call real providers and consume quota."
echo "Seedream image smoke verifies PNG download, auth boundaries, and generation cost ledger."
echo "DeepSeek key: $([[ "$deepseek_available" == "1" ]] && echo configured || echo missing)"
echo "Seedream/ARK key: $([[ "$seedream_available" == "1" ]] && echo configured || echo missing)"

ran_any=0

echo
echo "0. local prerequisite check"
"$ROOT_DIR/scripts/check-real-provider-readiness.sh" --composite --allow-missing-keys

echo
echo "1. real DeepSeek text"
case "$(should_run "$RUN_DEEPSEEK" "$deepseek_available"; echo $?)" in
  0)
    "$ROOT_DIR/scripts/smoke-real-deepseek-text.sh"
    wait_for_port_free 8081
    ran_any=1
    ;;
  1)
    echo "skip real DeepSeek text smoke"
    ;;
  2)
    echo "missing DEEPSEEK_API_KEY for required DeepSeek smoke" >&2
    exit 2
    ;;
  *)
    exit 1
    ;;
esac

echo
echo "2. real Seedream image"
case "$(should_run "$RUN_SEEDREAM" "$seedream_available"; echo $?)" in
  0)
    "$ROOT_DIR/scripts/smoke-real-seedream-image.sh"
    wait_for_port_free 8081
    ran_any=1
    ;;
  1)
    echo "skip real Seedream image smoke; set SEEDREAM_API_KEY or ARK_API_KEY to enable"
    ;;
  2)
    echo "missing SEEDREAM_API_KEY or ARK_API_KEY for required Seedream smoke" >&2
    exit 2
    ;;
  *)
    exit 1
    ;;
esac

echo
echo "3. real DeepSeek + Seedream composite"
composite_available=0
if [[ "$deepseek_available" == "1" && "$seedream_available" == "1" ]]; then
  composite_available=1
fi
case "$(should_run "$RUN_COMPOSITE" "$composite_available"; echo $?)" in
  0)
    "$ROOT_DIR/scripts/smoke-real-composite-provider.sh"
    wait_for_port_free 8081
    ran_any=1
    ;;
  1)
    echo "skip real composite smoke; configure both DEEPSEEK_API_KEY and SEEDREAM_API_KEY/ARK_API_KEY to enable"
    ;;
  2)
    echo "missing DeepSeek or Seedream/ARK key for required composite smoke" >&2
    exit 2
    ;;
  *)
    exit 1
    ;;
esac

if [[ "$ran_any" == "0" ]]; then
  echo "no real provider smoke was run; configure at least one real provider key" >&2
  exit 2
fi

echo
echo "== real provider smoke suite ok =="
