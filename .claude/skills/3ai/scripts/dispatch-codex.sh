#!/usr/bin/env bash
# dispatch-codex.sh — Codex (gpt-5.4) レビュー起動スクリプト
# 使い方:
#   設計レビュー: dispatch-codex.sh --mode design --input <plan.md> \
#                   --instruction <reviewer.md> --result <result.md> \
#                   [--scope-hint <str>] [--extra-input <file>]
#   最終レビュー: dispatch-codex.sh --mode review --base <branch> \
#                   --instruction <reviewer.md> --result <result.md>
# CODEX_DRY_RUN=1 のとき Codex を呼ばず stdin prefix をダンプして exit 0 (テスト用)
set -euo pipefail

MODE=""
INPUT_FILE=""
BASE_BRANCH=""
INSTRUCTION_FILE=""
RESULT_FILE=""
EXTRA_INPUT_FILE=""
SCOPE_HINT=""

while [[ $# -gt 0 ]]; do
  case "$1" in
    --mode)        MODE="$2";             shift 2 ;;
    --input)       INPUT_FILE="$2";       shift 2 ;;
    --base)        BASE_BRANCH="$2";      shift 2 ;;
    --instruction) INSTRUCTION_FILE="$2"; shift 2 ;;
    --result)      RESULT_FILE="$2";      shift 2 ;;
    --extra-input) EXTRA_INPUT_FILE="$2"; shift 2 ;;
    --scope-hint)  SCOPE_HINT="$2";       shift 2 ;;
    *) echo "Unknown arg: $1" >&2; exit 1 ;;
  esac
done

[[ -z "$MODE"             ]] && { echo "ERROR: --mode required (design|review)" >&2; exit 1; }
[[ -z "$INSTRUCTION_FILE" ]] && { echo "ERROR: --instruction required"          >&2; exit 1; }
[[ -z "$RESULT_FILE"      ]] && { echo "ERROR: --result required"               >&2; exit 1; }

INSTR="$(cat "$INSTRUCTION_FILE")"

# stdin prefix 生成: SCOPE PROFILE → extra-input ブロック群 の順で連結
# extra-input は dispatch-codex-auto.sh が構造化済みブロック（ISSUE CONTEXT / SCOPE DEFENSE 等）として生成するのでそのまま流す
build_prefix() {
  local prefix=""
  if [[ -n "$SCOPE_HINT" ]]; then
    prefix+="===== SCOPE PROFILE =====$(printf '\n')${SCOPE_HINT}$(printf '\n')===== END SCOPE PROFILE =====$(printf '\n\n')"
  fi
  if [[ -n "$EXTRA_INPUT_FILE" && -f "$EXTRA_INPUT_FILE" ]]; then
    prefix+="$(cat "$EXTRA_INPUT_FILE")$(printf '\n\n')"
  fi
  printf '%s' "$prefix"
}

detect_base() {
  git symbolic-ref refs/remotes/origin/HEAD 2>/dev/null | sed 's@^refs/remotes/origin/@@'
}

echo "=== dispatch-codex: mode=$MODE ===" >&2

case "$MODE" in
  design)
    [[ -z "$INPUT_FILE" ]] && { echo "ERROR: --input required for mode=design" >&2; exit 1; }
    echo "  input: $INPUT_FILE" >&2
    PREFIX="$(build_prefix)"
    if [[ "${CODEX_DRY_RUN:-}" == "1" ]]; then
      printf '%s' "$PREFIX"
      cat "$INPUT_FILE"
      exit 0
    fi
    # --output-last-message で最終メッセージ(verdict YAML)のみを結果ファイルに書く。
    # 推論ログ・ファイル読み込み等の冗長出力は .log に退避し、結果ファイルへの混入を防ぐ。
    DESIGN_EXIT=0
    { printf '%s' "$PREFIX"; cat "$INPUT_FILE"; } \
      | codex exec \
          -c sandbox_mode="read-only" \
          --output-last-message "$RESULT_FILE" \
          "$INSTR" \
      > "$RESULT_FILE.log" 2>&1 || DESIGN_EXIT=$?
    # final モードと同じ verdict.json を生成して上限到達後の Claude 裁量分岐に使わせる
    python3 - "$RESULT_FILE" > "$RESULT_FILE.verdict.json" 2>/dev/null <<'PY' || echo '{"verdict":"unknown","blocking":-1}' > "$RESULT_FILE.verdict.json"
import sys, re, json
text = open(sys.argv[1], encoding="utf-8", errors="replace").read()
verdicts = re.findall(r'verdict:\s*(pass|fail)', text)
verdict = verdicts[-1] if verdicts else "unknown"
sev = re.findall(r'severity:\s*(critical|high|medium|low)', text)
counts = {s: sev.count(s) for s in ("critical","high","medium","low")}
print(json.dumps({"verdict": verdict, "severity_counts": counts,
                  "blocking": counts["critical"] + counts["high"]}, ensure_ascii=False))
PY
    echo "=== dispatch-codex: done (exit $DESIGN_EXIT), result → $RESULT_FILE ===" >&2
    exit "$DESIGN_EXIT"
    ;;
  review)
    [[ -z "$BASE_BRANCH" ]] && BASE_BRANCH="$(detect_base)"
    [[ -z "$BASE_BRANCH" ]] && { echo "ERROR: --base unset and origin/HEAD undetectable" >&2; exit 1; }
    echo "  base: $BASE_BRANCH" >&2
    PREFIX="$(build_prefix)"
    if [[ "${CODEX_DRY_RUN:-}" == "1" ]]; then
      printf '%s' "$PREFIX"
      git diff "$BASE_BRANCH"...HEAD
      exit 0
    fi
    REVIEW_EXIT=0
    { printf '%s' "$PREFIX"; git diff "$BASE_BRANCH"...HEAD; } \
      | codex exec -c sandbox_mode="read-only" --output-last-message "$RESULT_FILE" "$INSTR" \
      > "$RESULT_FILE.log" 2>&1 || REVIEW_EXIT=$?
    python3 - "$RESULT_FILE" > "$RESULT_FILE.verdict.json" 2>/dev/null <<'PY' || echo '{"verdict":"unknown","blocking":-1}' > "$RESULT_FILE.verdict.json"
import sys, re, json
text = open(sys.argv[1], encoding="utf-8", errors="replace").read()
verdicts = re.findall(r'verdict:\s*(pass|fail)', text)
verdict = verdicts[-1] if verdicts else "unknown"
sev = re.findall(r'severity:\s*(critical|high|medium|low)', text)
counts = {s: sev.count(s) for s in ("critical","high","medium","low")}
print(json.dumps({"verdict": verdict, "severity_counts": counts,
                  "blocking": counts["critical"] + counts["high"]}, ensure_ascii=False))
PY
    echo "=== dispatch-codex: done (exit $REVIEW_EXIT), result → $RESULT_FILE ===" >&2
    exit "$REVIEW_EXIT"
    ;;
  *)
    echo "ERROR: unknown mode '$MODE'. Use 'design' or 'review'" >&2
    exit 1
    ;;
esac
