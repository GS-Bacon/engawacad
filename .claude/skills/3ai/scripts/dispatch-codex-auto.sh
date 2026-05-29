#!/usr/bin/env bash
# dispatch-codex-auto.sh — issue 種別を自動判定して dispatch-codex.sh を呼ぶラッパー
# 使い方:
#   設計レビュー: dispatch-codex-auto.sh --issue <n> --mode design \
#                   --input <plan.md> --plan <plan.md> \
#                   --state <state.json> --result <result.md> \
#                   [--rejection <rejection.md>]
#   最終レビュー: dispatch-codex-auto.sh --issue <n> --mode final \
#                   [--base <branch>] --state <state.json> --result <result.md>
#
# ループ上限超過時は exit 3 で終了(エスカレーションシグナル)。
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

ISSUE_NUM=""
MODE=""
INPUT_FILE=""
BASE_BRANCH=""
PLAN_FILE=""
STATE_FILE=""
RESULT_FILE=""
REJECTION_FILE=""

while [[ $# -gt 0 ]]; do
  case "$1" in
    --issue)     ISSUE_NUM="$2";     shift 2 ;;
    --mode)      MODE="$2";           shift 2 ;;
    --input)     INPUT_FILE="$2";     shift 2 ;;
    --base)      BASE_BRANCH="$2";    shift 2 ;;
    --plan)      PLAN_FILE="$2";      shift 2 ;;
    --state)     STATE_FILE="$2";     shift 2 ;;
    --result)    RESULT_FILE="$2";    shift 2 ;;
    --rejection) REJECTION_FILE="$2"; shift 2 ;;
    *) echo "Unknown arg: $1" >&2; exit 1 ;;
  esac
done

[[ -z "$ISSUE_NUM"  ]] && { echo "ERROR: --issue required"  >&2; exit 1; }
[[ -z "$MODE"       ]] && { echo "ERROR: --mode required (design|final)" >&2; exit 1; }
[[ -z "$STATE_FILE" ]] && { echo "ERROR: --state required"  >&2; exit 1; }
[[ -z "$RESULT_FILE" ]] && { echo "ERROR: --result required" >&2; exit 1; }

detect_base() {
  git symbolic-ref refs/remotes/origin/HEAD 2>/dev/null | sed 's@^refs/remotes/origin/@@'
}

# final モード: base を解決
if [[ "$MODE" == "final" ]]; then
  [[ -z "$BASE_BRANCH" ]] && BASE_BRANCH="$(detect_base)"
  [[ -z "$BASE_BRANCH" ]] && {
    echo "ERROR: --base unset and origin/HEAD undetectable" >&2; exit 1
  }
fi

# レビュー設定を取得
config_args=("$ISSUE_NUM" "$MODE")
[[ "$MODE" == "final"  && -n "$BASE_BRANCH" ]] && config_args+=(--diff-base "$BASE_BRANCH")
[[ "$MODE" == "design" && -n "$PLAN_FILE"   ]] && config_args+=(--plan "$PLAN_FILE")
eval "$("$SCRIPT_DIR/get-review-config.sh" "${config_args[@]}")"
# → DELIVERABLE, REVIEW_INSTRUCTION, MAX_LOOPS, SCOPE_HINT が展開される

# ループカウント強制
COUNTER="design_loops"
[[ "$MODE" == "final" ]] && COUNTER="final_loops"
n="$("$SCRIPT_DIR/state.sh" inc "$STATE_FILE" "$COUNTER")"
if (( n > MAX_LOOPS )); then
  echo "=== dispatch-codex-auto: ESCALATION ===" >&2
  echo "  issue=$ISSUE_NUM mode=$MODE deliverable=$DELIVERABLE" >&2
  echo "  ループ上限 $MAX_LOOPS 回に達しました(今回 ${n} 回目)。" >&2
  echo "  停止してユーザーにエスカレーションしてください。" >&2
  exit 3
fi
echo "=== dispatch-codex-auto: issue=$ISSUE_NUM mode=$MODE deliverable=$DELIVERABLE loop=${n}/${MAX_LOOPS} ===" >&2

# design モード: Non-Goals 抽出 → extra-input ファイル生成
EXTRA_INPUT_TMP=""
if [[ "$MODE" == "design" && -n "$PLAN_FILE" ]]; then
  [[ -z "$INPUT_FILE" ]] && { echo "ERROR: --input required for mode=design" >&2; exit 1; }

  # Non-Goals セクション欠落チェック
  if ! grep -q '^## Non-Goals' "$PLAN_FILE" 2>/dev/null; then
    echo "ERROR: plan に '## Non-Goals' セクションが必須です。実装しない項目を明記するか '- 該当なし' と書いてください。" >&2
    exit 1
  fi

  # Non-Goals 本文を抽出（次の ## まで）し extra-input ファイルへ
  EXTRA_INPUT_TMP="$(mktemp)"
  awk '/^## Non-Goals/{found=1; next} found && /^## /{exit} found{print}' "$PLAN_FILE" \
    | sed '/^[[:space:]]*$/d' > "$EXTRA_INPUT_TMP"

  # rejection.md が指定されており存在する場合、PRIOR REJECTIONS ブロックを追記
  if [[ -n "$REJECTION_FILE" && -f "$REJECTION_FILE" ]]; then
    {
      printf '\n===== PRIOR REJECTIONS =====\n'
      printf '以下は過去 round で棄却済み。蒸し返さないこと。\n'
      cat "$REJECTION_FILE"
      printf '\n===== END PRIOR REJECTIONS =====\n'
    } >> "$EXTRA_INPUT_TMP"
  fi
fi

# dispatch-codex.sh へ委譲
dispatch_args=(--instruction "$REVIEW_INSTRUCTION" --result "$RESULT_FILE")
[[ -n "${SCOPE_HINT:-}" ]]         && dispatch_args+=(--scope-hint "$SCOPE_HINT")
[[ -n "${EXTRA_INPUT_TMP:-}" ]]    && dispatch_args+=(--extra-input "$EXTRA_INPUT_TMP")
case "$MODE" in
  design)
    dispatch_args+=(--mode design --input "$INPUT_FILE")
    ;;
  final)
    dispatch_args+=(--mode review --base "$BASE_BRANCH")
    ;;
esac

bash "$SCRIPT_DIR/dispatch-codex.sh" "${dispatch_args[@]}"

[[ -n "${EXTRA_INPUT_TMP:-}" ]] && rm -f "$EXTRA_INPUT_TMP"
