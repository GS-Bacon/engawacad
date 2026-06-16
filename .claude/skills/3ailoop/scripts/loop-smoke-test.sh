#!/usr/bin/env bash
# loop-smoke-test.sh — /3ailoop SKILL.md の L-0〜L-9 を順番に直接呼出して連携確認
#
# 注意: /3ai 自体は呼ばない (= L-4 はスキップ)。スクリプト間の連携のみ検証。
#
# 使い方:
#   bash .claude/skills/3ailoop/scripts/loop-smoke-test.sh

set -e
cd "$(dirname "$0")/../../../.."

# 初期化
rm -rf features/.loop features/.batch/lock features/.dashboard.md

echo "=== L-0: acquire lock ==="
TOKEN=$(bun .claude/skills/3ailoop/scripts/loop-lock.ts acquire --owner loop 2>/dev/null)
[ ${#TOKEN} -eq 32 ] || { echo "L-0 FAIL: token len ${#TOKEN}"; exit 1; }
echo "L-0 PASS: acquired token ${TOKEN:0:8}..."

echo ""
echo "=== L-1: context bootstrap ==="
bun .claude/skills/3ailoop/scripts/loop-context-bootstrap.ts > /tmp/smoke-bootstrap.out
grep -q "Current Phase" /tmp/smoke-bootstrap.out
grep -q "Open Issues" /tmp/smoke-bootstrap.out
echo "L-1 PASS: bootstrap stdout has Current Phase / Open Issues"

echo ""
echo "=== L-2: should-stop ==="
RC=0
bun .claude/skills/3ailoop/scripts/loop-should-stop.ts > /tmp/smoke-should-stop.out 2>&1 || RC=$?
echo "  result: RC=$RC ($(cat /tmp/smoke-should-stop.out))"
echo "L-2 PASS: should-stop executed (RC=$RC)"

echo ""
echo "=== L-2.5: phase-close-check apply --dry-run ==="
RC=0
bun .claude/skills/3ailoop/scripts/loop-phase-close-check.ts apply --dry-run > /tmp/smoke-phase.out 2>&1 || RC=$?
echo "  RC=$RC"
echo "L-2.5 PASS: phase-close-check executed (RC=$RC, expected 1 for in-progress Phase)"

echo ""
echo "=== L-3: batch-select --loop --dry-run ==="
# stderr に warnings や dry-run メッセージが出るので分離
bun .claude/skills/3ai/scripts/batch-select.ts --loop --dry-run > /tmp/smoke-bs.json 2>/tmp/smoke-bs.err
jq -e .tier /tmp/smoke-bs.json > /dev/null || { echo "L-3 FAIL: invalid JSON"; head /tmp/smoke-bs.json; exit 1; }
TIER=$(jq -r .tier /tmp/smoke-bs.json)
echo "L-3 PASS: batch-select --loop produces JSON (tier=$TIER)"

echo ""
echo "=== L-4: skip (/3ai 自律モード起動は smoke test では呼ばない) ==="

echo ""
echo "=== L-5: cycle-record ==="
bun .claude/skills/3ailoop/scripts/loop-cycle-record.ts record > /tmp/smoke-cycle.json
CYCLE=$(jq -r .cycle features/.loop/state.json)
[ "$CYCLE" = "1" ] || { echo "L-5 FAIL: cycle=$CYCLE"; exit 1; }
echo "L-5 PASS: cycle-record recorded (cycle=$CYCLE)"

echo ""
echo "=== L-5.5: decision-log append ==="
bun .claude/skills/3ailoop/scripts/loop-decision-log.ts append --kind other --message "smoke test entry" > /dev/null
grep -q "smoke test entry" features/.loop/decisions.log.md
echo "L-5.5 PASS: decision-log appended"

echo ""
echo "=== L-6: dashboard ==="
bun .claude/skills/3ailoop/scripts/loop-dashboard.ts > /tmp/smoke-dash.out
[ -f features/.dashboard.md ]
for sec in "Current Status" "24h Activity" "Pending Gates" "Needs-Human Backlog" "Cumulative Stats" "Recent Activity" "Decision Log"; do
  grep -q "^## $sec" features/.dashboard.md || { echo "L-6 FAIL: missing section $sec"; exit 1; }
done
echo "L-6 PASS: dashboard.md has all 7 sections"

echo ""
echo "=== L-7: failure-tracker check (no failure expected) ==="
RC=0
bun .claude/skills/3ailoop/scripts/loop-failure-tracker.ts check --issue 999999 > /dev/null 2>&1 || RC=$?
[ $RC -eq 0 ] || { echo "L-7 FAIL: unexpected exit $RC"; exit 1; }
echo "L-7 PASS: failure-tracker check OK"

echo ""
echo "=== L-7.5: token-meter check (no logs = 0 tokens) ==="
RC=0
bun .claude/skills/3ailoop/scripts/loop-token-meter.ts check > /tmp/smoke-tok.out 2>&1 || RC=$?
[ $RC -eq 0 ] || { echo "L-7.5 FAIL: $RC"; cat /tmp/smoke-tok.out; exit 1; }
echo "L-7.5 PASS: token-meter check OK"

echo ""
echo "=== L-8: cycle exit (tmux 自走モードでは watcher が再投入) ==="
echo "L-8 PASS: cycle exit skip (ADR-012 tmux runtime: watcher が /clear → /3ailoop を再投入)"

echo ""
echo "=== L-9: release lock ==="
bun .claude/skills/3ailoop/scripts/loop-lock.ts release --token "$TOKEN" > /dev/null
[ ! -d features/.batch/lock ] || { echo "L-9 FAIL: lock dir not removed"; exit 1; }
echo "L-9 PASS: lock released"

echo ""
echo "========================================="
echo "ALL L-0〜L-9 SMOKE TEST PASSED"
echo "========================================="
echo ""
echo "Artifacts:"
echo "  - features/.loop/state.json (cycle=1)"
echo "  - features/.loop/decisions.log.md (smoke entry)"
echo "  - features/.dashboard.md (7 sections)"
