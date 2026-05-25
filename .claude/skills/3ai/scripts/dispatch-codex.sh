#!/usr/bin/env bash
# dispatch-codex.sh — Codex (gpt-5.4) レビュー起動スクリプト
# 使い方:
#   設計レビュー: dispatch-codex.sh --mode design --input <plan.md> \
#                   --instruction <reviewer.md> --result <result.md>
#   最終レビュー: dispatch-codex.sh --mode review --base <branch> \
#                   --instruction <reviewer.md> --result <result.md>
set -euo pipefail

MODE=""
INPUT_FILE=""
BASE_BRANCH=""
INSTRUCTION_FILE=""
RESULT_FILE=""

while [[ $# -gt 0 ]]; do
  case "$1" in
    --mode)        MODE="$2";             shift 2 ;;
    --input)       INPUT_FILE="$2";       shift 2 ;;
    --base)        BASE_BRANCH="$2";      shift 2 ;;
    --instruction) INSTRUCTION_FILE="$2"; shift 2 ;;
    --result)      RESULT_FILE="$2";      shift 2 ;;
    *) echo "Unknown arg: $1" >&2; exit 1 ;;
  esac
done

[[ -z "$MODE"             ]] && { echo "ERROR: --mode required (design|review)" >&2; exit 1; }
[[ -z "$INSTRUCTION_FILE" ]] && { echo "ERROR: --instruction required"          >&2; exit 1; }
[[ -z "$RESULT_FILE"      ]] && { echo "ERROR: --result required"               >&2; exit 1; }

INSTR="$(cat "$INSTRUCTION_FILE")"

detect_base() {
  git symbolic-ref refs/remotes/origin/HEAD 2>/dev/null | sed 's@^refs/remotes/origin/@@'
}

echo "=== dispatch-codex: mode=$MODE ===" >&2

case "$MODE" in
  design)
    [[ -z "$INPUT_FILE" ]] && { echo "ERROR: --input required for mode=design" >&2; exit 1; }
    echo "  input: $INPUT_FILE" >&2
    # --output-last-message で最終メッセージ(verdict YAML)のみを結果ファイルに書く。
    # 推論ログ・ファイル読み込み等の冗長出力は .log に退避し、結果ファイルへの混入を防ぐ。
    codex exec \
      -c sandbox_mode="read-only" \
      --output-last-message "$RESULT_FILE" \
      "$INSTR" \
      < "$INPUT_FILE" \
      > "$RESULT_FILE.log" 2>&1
    ;;
  review)
    [[ -z "$BASE_BRANCH" ]] && BASE_BRANCH="$(detect_base)"
    [[ -z "$BASE_BRANCH" ]] && { echo "ERROR: --base unset and origin/HEAD undetectable" >&2; exit 1; }
    echo "  base: $BASE_BRANCH" >&2
    REVIEW_EXIT=0
    git diff "$BASE_BRANCH"...HEAD \
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

EXIT=$?
echo "=== dispatch-codex: done (exit $EXIT), result → $RESULT_FILE ===" >&2
exit $EXIT
