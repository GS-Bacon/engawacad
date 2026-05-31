#!/usr/bin/env bash
# state.sh — /3ai フロー状態の記録・検証
#   state.sh init   <state.json> <issue> <slug>
#   state.sh set    <state.json> <step> <passed|failed>
#   state.sh get    <state.json> <step>      # 値を stdout（無ければ "none"）
#   state.sh assert <state.json> <step>      # passed なら exit 0、それ以外 exit 1
#   state.sh inc    <state.json> <key>       # ドット区切りネストキー対応
#   state.sh assert-critical-zero <state.json> <verdict.json>  # critical=0 なら exit 0
set -euo pipefail
CMD="$1"; FILE="$2"
case "$CMD" in
  init)
    python3 -c "
import json, sys
json.dump({
  'issue': int(sys.argv[1]), 'slug': sys.argv[2], 'steps': {},
  'loops': {}, 'judgments': [],
  'phases': {'core_impl': {'glm_runs': 0}, 'test_impl': {'glm_runs': 0}}
}, open(sys.argv[3], 'w'))
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
    # ドット区切りネストキーをサポート（例: phases.core_impl.glm_runs）
    # 後方互換: ドットなしは従来の loops.{key} と同等
    python3 -c "
import json, sys
d = json.load(open(sys.argv[1]))
key = sys.argv[2]
if '.' in key:
    parts = key.split('.')
    node = d
    for p in parts[:-1]:
        node = node.setdefault(p, {})
    n = node.get(parts[-1], 0) + 1
    node[parts[-1]] = n
else:
    loops = d.setdefault('loops', {})
    n = loops.get(key, 0) + 1
    loops[key] = n
json.dump(d, open(sys.argv[1], 'w'))
print(n)
" "$FILE" "$3"
    ;;
  assert-critical-zero)
    # state.sh assert-critical-zero <state.json> <verdict.json>
    # verdict.json の severity_counts.critical が 0 なら exit 0、それ以外 exit 1
    python3 -c "
import json, sys
try:
    v = json.load(open(sys.argv[2]))
    critical = v.get('severity_counts', {}).get('critical', 0)
    sys.exit(0 if critical == 0 else 1)
except Exception:
    sys.exit(1)
" "$FILE" "$3"
    ;;
  judge)
    # state.sh judge <state.json> <round> <adopted> <rejected>
    python3 -c "
import json, sys
d = json.load(open(sys.argv[1]))
judgments = d.setdefault('judgments', [])
judgments.append({'round': int(sys.argv[2]), 'adopted': int(sys.argv[3]), 'rejected': int(sys.argv[4])})
json.dump(d, open(sys.argv[1], 'w'))
" "$FILE" "$3" "$4" "$5"
    ;;
  check-full-adoption-warning)
    # exit 1 (=警告) if 直近 2 round 連続で rejected=0
    python3 -c "
import json, sys
d = json.load(open(sys.argv[1]))
judgments = d.get('judgments', [])
if len(judgments) < 2:
    sys.exit(0)
last2 = judgments[-2:]
if all(j['rejected'] == 0 for j in last2):
    sys.exit(1)
sys.exit(0)
" "$FILE"
    ;;
  *)
    echo "unknown cmd: $CMD" >&2
    exit 1
    ;;
esac
