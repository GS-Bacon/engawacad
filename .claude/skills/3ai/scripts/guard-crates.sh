#!/usr/bin/env bash
# guard-crates.sh — PreToolUse フック
# Claude(オーケストレーター)が crates/** を直接編集しようとしたら deny する。
# CAD_WORKER=1 の環境（dispatch-glm.sh 経由の子プロセス）は allow する。
#
# Claude Code から stdin に JSON が渡される:
#   {"tool_name": "Edit", "tool_input": {"file_path": "...", ...}}

# ワーカープロセスは通過させる
if [[ -n "${CAD_WORKER:-}" ]]; then
  exit 0
fi

# stdin から tool_input の file_path / notebook_path を取得
input="$(cat)"

file_path="$(python3 -c "
import json, sys
try:
    d = json.loads(sys.stdin.read())
    ti = d.get('tool_input', {})
    print(ti.get('file_path', ti.get('notebook_path', '')))
except Exception:
    print('')
" <<< "$input" 2>/dev/null || true)"

# crates/ を含むパスなら deny
if echo "$file_path" | grep -qE '(^|/)crates/'; then
  cat <<'MSG'
{"decision":"block","reason":"🔒 crates/** の直接編集は禁止です（/3ai フロー）。\ndispatch-glm.sh 経由で GLM-5.1 に委譲してください。\n誤って自分で実装しないこと（PoorDev 問題の再発防止）。"}
MSG
  exit 2
fi

exit 0
