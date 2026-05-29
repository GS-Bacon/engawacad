#!/usr/bin/env bash
# get-review-config.sh <issue_num> <design|final> [--diff-base <branch>] [--plan <file>]
# stdout(eval 用): DELIVERABLE=docs|code  REVIEW_INSTRUCTION=<path>  MAX_LOOPS=<n>
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
AGENTS_DIR="$SCRIPT_DIR/../agents"

ISSUE_NUM="$1"
MODE="$2"
shift 2

DIFF_BASE=""
PLAN_FILE=""

while [[ $# -gt 0 ]]; do
  case "$1" in
    --diff-base) DIFF_BASE="$2"; shift 2 ;;
    --plan)      PLAN_FILE="$2";  shift 2 ;;
    *) echo "Unknown arg: $1" >&2; exit 1 ;;
  esac
done

[[ -z "$ISSUE_NUM" ]] && { echo "ERROR: issue_num required" >&2; exit 1; }
[[ "$MODE" != "design" && "$MODE" != "final" ]] && {
  echo "ERROR: mode must be 'design' or 'final'" >&2; exit 1
}

detect_args=("$ISSUE_NUM")
[[ -n "$DIFF_BASE" ]] && detect_args+=(--diff-base "$DIFF_BASE")
[[ -n "$PLAN_FILE" ]] && detect_args+=(--plan "$PLAN_FILE")

DELIVERABLE="$("$SCRIPT_DIR/detect-deliverable.sh" "${detect_args[@]}")"

case "$MODE:$DELIVERABLE" in
  design:code)
    REVIEW_INSTRUCTION="$AGENTS_DIR/codex-design-reviewer.md"
    MAX_LOOPS=3
    ;;
  design:docs)
    REVIEW_INSTRUCTION="$AGENTS_DIR/codex-design-reviewer-docs.md"
    MAX_LOOPS=2
    ;;
  final:code)
    REVIEW_INSTRUCTION="$AGENTS_DIR/codex-final-reviewer.md"
    MAX_LOOPS=2
    ;;
  final:docs)
    REVIEW_INSTRUCTION="$AGENTS_DIR/codex-final-reviewer-docs.md"
    MAX_LOOPS=1
    ;;
  *)
    echo "ERROR: unexpected mode/deliverable: $MODE/$DELIVERABLE" >&2; exit 1 ;;
esac

# type ラベルから SCOPE_HINT を決定（新規 label 不要: 既存 type: ラベルを流用）
labels="$(gh issue view "$ISSUE_NUM" --json labels -q '[.labels[].name] | join(",")' 2>/dev/null || echo "")"
SCOPE_HINT=""
if echo "$labels" | grep -q 'type: foundation'; then
  SCOPE_HINT="foundation: 拡張・最適化・退化検出の追加は指摘しない。構造の入れ物が成立しているかだけ見ること。完成度ではなく『次の issue が継続できるか』が成否基準。"
elif echo "$labels" | grep -q 'type: refactor'; then
  SCOPE_HINT="refactor: 影響範囲の最小性を最重視。新機能要求・gold plating はしない。"
fi

printf 'DELIVERABLE=%s\nREVIEW_INSTRUCTION=%s\nMAX_LOOPS=%d\n' \
  "$DELIVERABLE" "$REVIEW_INSTRUCTION" "$MAX_LOOPS"
printf 'SCOPE_HINT=%q\n' "$SCOPE_HINT"
