#!/usr/bin/env bash
set -euo pipefail

if [ "$#" -lt 1 ]; then
  echo "Usage: npm run branch:new -- \"変更内容\""
  exit 1
fi

date_prefix="$(date +%Y%m%d)"
description="$*"

normalized_description="$(printf '%s' "$description" | tr '[:space:]' '-' | tr -s '-')"
branch_name="${date_prefix}(${normalized_description})"

if ! git check-ref-format --branch "$branch_name" >/dev/null 2>&1; then
  echo "Invalid branch name: $branch_name"
  exit 1
fi

git switch -c "$branch_name"
echo "Created and switched to branch: $branch_name"
