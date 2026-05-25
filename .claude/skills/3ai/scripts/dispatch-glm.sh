#!/usr/bin/env bash
# dispatch-glm.sh — GLM-5.1 (Z.AI) ワーカー起動スクリプト
# mycad 専用。グローバル /usr/local/bin/glm を触らない。
# 使い方:
#   dispatch-glm.sh --agent <agent.md> --plan-file <plan.md> \
#                   --feature-dir <dir> --result-file <result.json> \
#                   [--max-turns N] [--model GLM-5.1]
set -euo pipefail

AGENT_FILE=""
PLAN_FILE=""
FEATURE_DIR=""
RESULT_FILE=""
MAX_TURNS=80
MODEL="GLM-5.1"

while [[ $# -gt 0 ]]; do
  case "$1" in
    --agent)        AGENT_FILE="$2";    shift 2 ;;
    --plan-file)    PLAN_FILE="$2";     shift 2 ;;
    --feature-dir)  FEATURE_DIR="$2";   shift 2 ;;
    --result-file)  RESULT_FILE="$2";   shift 2 ;;
    --max-turns)    MAX_TURNS="$2";     shift 2 ;;
    --model)        MODEL="$2";         shift 2 ;;
    *) echo "Unknown arg: $1" >&2; exit 1 ;;
  esac
done

[[ -z "$AGENT_FILE"   ]] && { echo "ERROR: --agent required"        >&2; exit 1; }
[[ -z "$PLAN_FILE"    ]] && { echo "ERROR: --plan-file required"     >&2; exit 1; }
[[ -z "$FEATURE_DIR"  ]] && { echo "ERROR: --feature-dir required"   >&2; exit 1; }
[[ -z "$RESULT_FILE"  ]] && { echo "ERROR: --result-file required"   >&2; exit 1; }

# Z.AI 認証キーを読み込む
ZAI_ENV="${ZAI_ENV:-$HOME/AutoClaudeKMP/.env}"
if [[ ! -f "$ZAI_ENV" ]]; then
  echo "ERROR: Z.AI env file not found: $ZAI_ENV" >&2
  echo "  Set ZAI_ENV env var to the path of the .env file containing Z_AI_API_KEY" >&2
  exit 1
fi
# shellcheck source=/dev/null
source "$ZAI_ENV"
[[ -z "${Z_AI_API_KEY:-}" ]] && { echo "ERROR: Z_AI_API_KEY not set in $ZAI_ENV" >&2; exit 1; }

export ANTHROPIC_BASE_URL="https://api.z.ai/api/anthropic"
export ANTHROPIC_AUTH_TOKEN="$Z_AI_API_KEY"
export ANTHROPIC_DEFAULT_OPUS_MODEL="$MODEL"
export ANTHROPIC_DEFAULT_SONNET_MODEL="$MODEL"
export ANTHROPIC_DEFAULT_HAIKU_MODEL="$MODEL"
export API_TIMEOUT_MS="3000000"
export CAD_WORKER=1   # PreToolUse フック除外ゲート
unset CLAUDECODE      # ネスト検知を回避（glm ラッパーと同様）

mkdir -p "$FEATURE_DIR"

# GLM へのプロンプト生成
PROMPT="$(cat <<PROMPT
以下の確定プランに従い実装・テスト・CI を完了させてください。

## 作業ディレクトリ
$FEATURE_DIR

## 確定プラン
$(cat "$PLAN_FILE")

## 完了条件
1. プランに記載された全機能を実装する
2. テスト計画の全ケースを実装し通過させる
3. \`cargo xtask ci\` が green（build / test / clippy -D warnings / fmt --check）
4. エッジケーステスト: "壊しに行く" 敵対ペルソナで境界・退化入力を網羅する
5. 結果を $RESULT_FILE に JSON で書き出す:
   { "status": "success|failed", "ci_passed": true|false, "summary": "...", "failed_reason": "..." }

## 禁止事項
- git commit/push は行わない（オーケストレーターが行う）
- プラン外の機能追加・リファクタは行わない
PROMPT
)"

echo "=== dispatch-glm: starting GLM worker ===" >&2
echo "  agent:   $AGENT_FILE" >&2
echo "  plan:    $PLAN_FILE" >&2
echo "  feature: $FEATURE_DIR" >&2
echo "  result:  $RESULT_FILE" >&2
echo "  model:   $MODEL" >&2

STATUS=0
claude -p "$PROMPT" \
  --append-system-prompt-file "$AGENT_FILE" \
  --allowedTools "Read,Edit,Write,Bash(cargo *),Bash(npm *),Bash(npx *),Bash(node *),Bash(mkdir *),Bash(cat *),Bash(ls *),Bash(find *),Glob,Grep" \
  --max-turns "$MAX_TURNS" \
  --output-format json \
  > "$RESULT_FILE.raw" 2>&1 || STATUS=$?

# A: GLM の自己申告を信用せず、オーケストレーター側で CI を実走して真偽を決める
ROOT="$(git rev-parse --show-toplevel)"
CI_PASSED=false
if (cd "$ROOT" && cargo xtask ci) > "$FEATURE_DIR/ci.log" 2>&1; then
  CI_PASSED=true
fi

# raw から GLM の summary を best-effort 抽出しつつ、status/ci_passed は CI 実走結果を権威とする
python3 - "$RESULT_FILE.raw" "$CI_PASSED" "$STATUS" "$FEATURE_DIR/ci.log" > "$RESULT_FILE" 2>/dev/null <<'PY' || echo '{"status":"failed","ci_passed":false,"summary":"result generation error"}' > "$RESULT_FILE"
import json, sys
raw_path, ci_str, glm_exit, ci_log = sys.argv[1:5]
raw = open(raw_path, encoding="utf-8", errors="replace").read()
ci_passed = ci_str == "true"
summary = ""
for line in reversed(raw.strip().split("\n")):
    try:
        o = json.loads(line)
        if isinstance(o, dict) and "summary" in o:
            summary = o["summary"]; break
    except Exception:
        pass
out = {
    "status": "success" if ci_passed else "failed",
    "ci_passed": ci_passed,
    "summary": summary or ("CI green" if ci_passed else "CI red"),
    "glm_exit": int(glm_exit),
}
if not ci_passed:
    out["failed_reason"] = f"cargo xtask ci failed — see {ci_log}"
print(json.dumps(out, ensure_ascii=False))
PY
