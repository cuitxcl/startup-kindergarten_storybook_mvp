#!/usr/bin/env bash

kindleaf_load_env_files() {
  local root_dir="$1"
  local file line key value

  for file in \
    "$root_dir/.env.local" \
    "$root_dir/.env" \
    "$root_dir/server/.env.local" \
    "$root_dir/server/.env"; do
    [[ -f "$file" ]] || continue
    while IFS= read -r line || [[ -n "$line" ]]; do
      line="${line#"${line%%[![:space:]]*}"}"
      line="${line%"${line##*[![:space:]]}"}"
      [[ -n "$line" && "${line:0:1}" != "#" ]] || continue
      [[ "$line" == export\ * ]] && line="${line#export }"
      [[ "$line" == *=* ]] || continue

      key="${line%%=*}"
      value="${line#*=}"
      key="${key%"${key##*[![:space:]]}"}"
      value="${value#"${value%%[![:space:]]*}"}"
      value="${value%"${value##*[![:space:]]}"}"
      [[ "$key" =~ ^[A-Za-z_][A-Za-z0-9_]*$ ]] || continue

      if [[ "$value" == \"*\" && "$value" == *\" && ${#value} -ge 2 ]]; then
        value="${value:1:${#value}-2}"
      elif [[ "$value" == \'*\' && "$value" == *\' && ${#value} -ge 2 ]]; then
        value="${value:1:${#value}-2}"
      fi

      if [[ -z "${!key+x}" ]]; then
        export "$key=$value"
      fi
    done <"$file"
  done
}

kindleaf_looks_like_placeholder_secret() {
  local normalized
  normalized="$(printf '%s' "$1" | tr '[:upper:]' '[:lower:]' | tr -d '[:space:]' | tr '_' '-')"
  case "$normalized" in
    your-*|replace-*|*-placeholder|placeholder-*|api-key|test-key|demo-key|example-key|change-me|changeme|xxx|xxxx|placeholder)
      return 0
      ;;
    *)
      return 1
      ;;
  esac
}

kindleaf_real_secret_configured() {
  local value="$1"
  [[ -n "${value//[[:space:]]/}" ]] && ! kindleaf_looks_like_placeholder_secret "$value"
}
