#!/usr/bin/env bash
set -euo pipefail

current_branch="$(git rev-parse --abbrev-ref HEAD 2>/dev/null || echo '')"
pattern='^[0-9]{8}\(.+\)$'

if [[ "$current_branch" =~ $pattern ]]; then
  exit 0
fi

if git diff --quiet && git diff --cached --quiet; then
  exit 0
fi

date_prefix="$(date +%Y%m%d)"
provided_description="${1:-${AJISAI_BRANCH_DESC:-}}"

if [ -n "$provided_description" ]; then
  description="$provided_description"
else
  first_changed_file="$(git diff --cached --name-only | head -n 1)"
  if [ -z "$first_changed_file" ]; then
    first_changed_file="$(git diff --name-only | head -n 1)"
  fi

  if [ -n "$first_changed_file" ]; then
    base_name="$(basename "$first_changed_file")"
    base_name="${base_name%.*}"
    description="${base_name}-change"
  else
    description="update"
  fi
fi

normalized_description="$(printf '%s' "$description" | tr '[:space:]' '-' | tr -cs '[:alnum:]_-' '-')"
normalized_description="$(printf '%s' "$normalized_description" | sed -E 's/^-+//; s/-+$//; s/-+/-/g')"

if [ -z "$normalized_description" ]; then
  normalized_description="update"
fi

branch_name="${date_prefix}(${normalized_description})"

if git show-ref --verify --quiet "refs/heads/${branch_name}"; then
  suffix=2
  while git show-ref --verify --quiet "refs/heads/${branch_name}-${suffix}"; do
    suffix=$((suffix + 1))
  done
  branch_name="${branch_name}-${suffix}"
fi

git switch -c "$branch_name" >/dev/null
echo "[Ajisai] Created branch automatically: $branch_name"
