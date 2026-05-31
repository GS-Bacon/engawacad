#!/usr/bin/env bash
# dispatch-codex-auto.sh — issue 種別を自動判定して dispatch-codex.sh を呼ぶラッパー
# 使い方:
#   設計レビュー: dispatch-codex-auto.sh --issue <n> --mode design \
#                   --input <plan.md> --plan <plan.md> \
#                   --state <state.json> --result <result.md> \
#                   [--rejection <rejection.md>] \
#                   [--adr-context <adr-context.md>] \
#                   [--judgment-summary <judgment-summary.md>] \
#                   [--plan-snapshot-dir <dir>]
#   最終レビュー: dispatch-codex-auto.sh --issue <n> --mode final \
#                   [--base <branch>] --state <state.json> --result <result.md> \
#                   [--test-summary <test-summary.json>]
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
ADR_CONTEXT_FILE=""
JUDGMENT_SUMMARY_FILE=""
PLAN_SNAPSHOT_DIR=""
TEST_SUMMARY_FILE=""

while [[ $# -gt 0 ]]; do
  case "$1" in
    --issue)            ISSUE_NUM="$2";            shift 2 ;;
    --mode)             MODE="$2";                  shift 2 ;;
    --input)            INPUT_FILE="$2";            shift 2 ;;
    --base)             BASE_BRANCH="$2";           shift 2 ;;
    --plan)             PLAN_FILE="$2";             shift 2 ;;
    --state)            STATE_FILE="$2";            shift 2 ;;
    --result)           RESULT_FILE="$2";           shift 2 ;;
    --rejection)        REJECTION_FILE="$2";        shift 2 ;;
    --adr-context)      ADR_CONTEXT_FILE="$2";      shift 2 ;;
    --judgment-summary) JUDGMENT_SUMMARY_FILE="$2"; shift 2 ;;
    --plan-snapshot-dir) PLAN_SNAPSHOT_DIR="$2";   shift 2 ;;
    --test-summary)     TEST_SUMMARY_FILE="$2";     shift 2 ;;
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

# design モード: plan スナップショット保存（round n 開始前）
if [[ "$MODE" == "design" && -n "$PLAN_SNAPSHOT_DIR" && -n "$PLAN_FILE" ]]; then
  mkdir -p "$PLAN_SNAPSHOT_DIR"
  cp "$PLAN_FILE" "$PLAN_SNAPSHOT_DIR/plan.md.round-${n}"
fi

# design モード: Non-Goals 抽出 → extra-input ファイル生成
EXTRA_INPUT_TMP=""
if [[ "$MODE" == "design" && -n "$PLAN_FILE" ]]; then
  [[ -z "$INPUT_FILE" ]] && { echo "ERROR: --input required for mode=design" >&2; exit 1; }

  # Non-Goals セクション欠落チェック
  if ! grep -q '^## Non-Goals' "$PLAN_FILE" 2>/dev/null; then
    echo "ERROR: plan に '## Non-Goals' セクションが必須です。実装しない項目を明記するか '- 該当なし' と書いてください。" >&2
    exit 1
  fi

  EXTRA_INPUT_TMP="$(mktemp)"

  # ① Issue 本文（毎 round）
  {
    printf '===== ISSUE CONTEXT =====\n'
    gh issue view "$ISSUE_NUM" --json title,body \
      -q '"Issue #\(.number // "") \(.title)\n\n\(.body)"' 2>/dev/null \
      || printf '(Issue 取得失敗)'
    printf '\n===== END ISSUE CONTEXT =====\n\n'
  } >> "$EXTRA_INPUT_TMP"

  # ② ADR 抜粋（指定時のみ）
  if [[ -n "$ADR_CONTEXT_FILE" && -f "$ADR_CONTEXT_FILE" ]]; then
    {
      printf '===== ADR EXCERPT =====\n'
      printf '以下は本 Issue が前提とする設計決定（ADR 抜粋）。この方式自体への異議は挙げないこと。\n'
      cat "$ADR_CONTEXT_FILE"
      printf '\n===== END ADR EXCERPT =====\n\n'
    } >> "$EXTRA_INPUT_TMP"
  fi

  # ③ Non-Goals 本文を抽出（次の ## まで）して SCOPE DEFENSE ブロックとして追記
  {
    printf '===== SCOPE DEFENSE =====\n'
    printf '以下は本 Issue のスコープ外。指摘・拡張提案・改善要求の対象としないこと。\n'
    awk '/^## Non-Goals/{found=1; next} found && /^## /{exit} found{print}' "$PLAN_FILE" \
      | sed '/^[[:space:]]*$/d'
    printf '\n===== END SCOPE DEFENSE =====\n\n'
  } >> "$EXTRA_INPUT_TMP"

  # ④ rejection.md（指定時かつ存在時）
  if [[ -n "$REJECTION_FILE" && -f "$REJECTION_FILE" ]]; then
    {
      printf '===== PRIOR REJECTIONS =====\n'
      printf '以下は過去 round で棄却済み。蒸し返さないこと。\n'
      cat "$REJECTION_FILE"
      printf '\n===== END PRIOR REJECTIONS =====\n\n'
    } >> "$EXTRA_INPUT_TMP"
  fi

  # ⑤ PRIOR JUDGMENTS（round 2 以降 + 指定時）
  if (( n >= 2 )) && [[ -n "$JUDGMENT_SUMMARY_FILE" && -f "$JUDGMENT_SUMMARY_FILE" ]]; then
    {
      printf '===== PRIOR JUDGMENTS =====\n'
      printf '前 round で Claude が採用・棄却を判定済みの一覧。採用済み指摘は「足りない」と再指摘しない。棄却済み事項は再度持ち出さない。\n'
      cat "$JUDGMENT_SUMMARY_FILE"
      printf '\n===== END PRIOR JUDGMENTS =====\n\n'
    } >> "$EXTRA_INPUT_TMP"
  fi

  # ⑥ PLAN DIFF（round 2 以降 + スナップショットが存在する場合）
  if (( n >= 2 )) && [[ -n "$PLAN_SNAPSHOT_DIR" ]]; then
    PREV_SNAPSHOT="$PLAN_SNAPSHOT_DIR/plan.md.round-$((n-1))"
    if [[ -f "$PREV_SNAPSHOT" ]]; then
      PLAN_DIFF_OUT="$(diff -u "$PREV_SNAPSHOT" "$PLAN_FILE" || true)"
      if [[ -n "$PLAN_DIFF_OUT" ]]; then
        {
          printf '===== PLAN DIFF =====\n'
          printf '前 round からの plan 変更点（unified diff）。指摘対応として行われた変更箇所への「やり方が違う」指摘は必ず理由を添えること。\n'
          printf '%s\n' "$PLAN_DIFF_OUT"
          printf '===== END PLAN DIFF =====\n\n'
        } >> "$EXTRA_INPUT_TMP"
      fi
    fi
  fi
fi

# final モード: TEST SUMMARY の注入
FINAL_EXTRA_TMP=""
if [[ "$MODE" == "final" && -n "$TEST_SUMMARY_FILE" && -f "$TEST_SUMMARY_FILE" ]]; then
  FINAL_EXTRA_TMP="$(mktemp)"
  {
    printf '===== TEST SUMMARY =====\n'
    printf 'テスト実行結果サマリ（構造化 JSON）。coverage_hints をテスト網羅性評価の入力として使うこと。\n'
    cat "$TEST_SUMMARY_FILE"
    printf '\n===== END TEST SUMMARY =====\n\n'
  } > "$FINAL_EXTRA_TMP"
fi

# dispatch-codex.sh へ委譲
dispatch_args=(--instruction "$REVIEW_INSTRUCTION" --result "$RESULT_FILE")
[[ -n "${SCOPE_HINT:-}" ]] && dispatch_args+=(--scope-hint "$SCOPE_HINT")
case "$MODE" in
  design)
    [[ -n "${EXTRA_INPUT_TMP:-}" ]] && dispatch_args+=(--extra-input "$EXTRA_INPUT_TMP")
    dispatch_args+=(--mode design --input "$INPUT_FILE")
    ;;
  final)
    [[ -n "${FINAL_EXTRA_TMP:-}" ]] && dispatch_args+=(--extra-input "$FINAL_EXTRA_TMP")
    dispatch_args+=(--mode review --base "$BASE_BRANCH")
    ;;
esac

bash "$SCRIPT_DIR/dispatch-codex.sh" "${dispatch_args[@]}"

[[ -n "${EXTRA_INPUT_TMP:-}" ]] && rm -f "$EXTRA_INPUT_TMP"
[[ -n "${FINAL_EXTRA_TMP:-}" ]] && rm -f "$FINAL_EXTRA_TMP"
