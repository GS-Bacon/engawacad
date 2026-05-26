#!/usr/bin/env bash
# detect-deliverable.sh <issue_num> [--diff-base <branch>] [--plan <file>]
# 出力: "docs" または "code"
#
# 判定順(パス主・ラベル従):
#   1. --diff-base: git diff の全ファイルが docs/*.md のみ → docs
#   2. --plan: plan 宣言パスに crates/.rs → code, docs/.md → docs
#   3. ラベル fallback: code ラベルなし & docs ラベルあり → docs, それ以外 → code
set -euo pipefail

ISSUE_NUM=""
DIFF_BASE=""
PLAN_FILE=""

while [[ $# -gt 0 ]]; do
  case "$1" in
    --diff-base) DIFF_BASE="$2"; shift 2 ;;
    --plan)      PLAN_FILE="$2";  shift 2 ;;
    -*)
      echo "Unknown option: $1" >&2; exit 1 ;;
    *)
      if [[ -z "$ISSUE_NUM" ]]; then ISSUE_NUM="$1"; shift
      else echo "Unknown arg: $1" >&2; exit 1; fi
      ;;
  esac
done

[[ -z "$ISSUE_NUM" ]] && {
  echo "Usage: detect-deliverable.sh <issue_num> [--diff-base <branch>] [--plan <file>]" >&2
  exit 1
}

# 1. diff-base (final モード、authoritative)
if [[ -n "$DIFF_BASE" ]]; then
  changed=$(git diff --name-only "${DIFF_BASE}...HEAD" 2>/dev/null || true)
  if [[ -z "$changed" ]]; then
    echo "docs"; exit 0
  fi
  if echo "$changed" | grep -qvE '^docs/|\.md$'; then
    echo "code"
  else
    echo "docs"
  fi
  exit 0
fi

# 2. plan ファイル (design モード)
if [[ -n "$PLAN_FILE" && -f "$PLAN_FILE" ]]; then
  paths=$(grep -oE '(docs/[^ ]+|crates/[^ ]+|[^ ]+\.md|[^ ]+\.rs)' "$PLAN_FILE" 2>/dev/null || true)
  if [[ -n "$paths" ]]; then
    if echo "$paths" | grep -qE '(^crates/|\.rs$)'; then
      echo "code"; exit 0
    elif echo "$paths" | grep -qE '(^docs/|\.md$)'; then
      echo "docs"; exit 0
    fi
  fi
fi

# 3. ラベル fallback
labels=$(gh issue view "$ISSUE_NUM" --json labels -q '[.labels[].name] | join(",")' 2>/dev/null || echo "")
if echo "$labels" | grep -qE '(kernel|format|cli|viewer)'; then
  echo "code"; exit 0
fi
if echo "$labels" | grep -q 'docs'; then
  echo "docs"; exit 0
fi
echo "code"
