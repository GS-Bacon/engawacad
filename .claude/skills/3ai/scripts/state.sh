#!/usr/bin/env bash
# state.sh — /3ai フロー状態の記録・検証
#   state.sh init   <state.json> <issue> <slug>
#   state.sh set    <state.json> <step> <passed|failed>
#   state.sh get    <state.json> <step>      # 値を stdout（無ければ "none"）
#   state.sh assert <state.json> <step>      # passed なら exit 0、それ以外 exit 1
set -euo pipefail
CMD="$1"; FILE="$2"
case "$CMD" in
  init)
    python3 -c "
import json, sys
json.dump({'issue': int(sys.argv[1]), 'slug': sys.argv[2], 'steps': {}}, open(sys.argv[3], 'w'))
" "$3" "$4" "$FILE"
    ;;
  set)
    python3 -c "
import json, sys
d = json.load(open(sys.argv[1]))
d['steps'][sys.argv[2]] = sys.argv[3]
json.dump(d, open(sys.argv[1], 'w'))
" "$FILE" "$3" "$4"
    ;;
  get)
    python3 -c "
import json, sys
print(json.load(open(sys.argv[1]))['steps'].get(sys.argv[2], 'none'))
" "$FILE" "$3"
    ;;
  assert)
    python3 -c "
import json, sys
sys.exit(0 if json.load(open(sys.argv[1]))['steps'].get(sys.argv[2]) == 'passed' else 1)
" "$FILE" "$3"
    ;;
  inc)
    python3 -c "
import json, sys
d = json.load(open(sys.argv[1]))
loops = d.setdefault('loops', {})
n = loops.get(sys.argv[2], 0) + 1
loops[sys.argv[2]] = n
json.dump(d, open(sys.argv[1], 'w'))
print(n)
" "$FILE" "$3"
    ;;
  *)
    echo "unknown cmd: $CMD" >&2
    exit 1
    ;;
esac
