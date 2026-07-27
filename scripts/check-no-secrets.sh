#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

if ! git -C "$ROOT_DIR" rev-parse --is-inside-work-tree >/dev/null 2>&1; then
  echo "not inside a git worktree; skipping secret check"
  exit 0
fi

matches="$(
  rg -l --hidden \
    --glob '!.git/' \
    --glob '!frontend/dist/' \
    --glob '!frontend/node_modules/' \
    --glob '!server/target/' \
    --glob '!.tmp/' \
    -e 'sk-[A-Za-z0-9]{30,}' \
    -e '(DEEPSEEK|SEEDREAM|ARK)_API_KEY=(sk-|[0-9a-fA-F]{8}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{12})' \
    "$ROOT_DIR" || true
)"

if [[ -n "$matches" ]]; then
  echo "possible provider secret(s) found in:" >&2
  sed "s#^$ROOT_DIR/##" <<<"$matches" >&2
  echo "secret check failed; move real provider keys to ignored .env.local or server/.env.local" >&2
  exit 1
fi

echo "secret check ok"
