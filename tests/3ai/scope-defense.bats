#!/usr/bin/env bats
# #37 scope-defense 機構のユニットテスト

load helpers/common

STATE_SH="$SCRIPTS_DIR/state.sh"
AUTO_SH="$SCRIPTS_DIR/dispatch-codex-auto.sh"

setup() {
  setup_common
  STATE_FILE="$BATS_TMPDIR/state.json"
}

teardown() {
  teardown_common
}

# --- TC7a: state.sh judge が judgments を正しく記録する ---

@test "TC7a: state.sh judge が round/adopted/rejected を judgments 配列に追記する" {
  bash "$STATE_SH" init "$STATE_FILE" 37 scope-defense

  bash "$STATE_SH" judge "$STATE_FILE" 1 3 1

  run python3 -c "
import json, sys
d = json.load(open('$STATE_FILE'))
j = d['judgments'][0]
assert j['round'] == 1 and j['adopted'] == 3 and j['rejected'] == 1, j
print('ok')
"
  [ "$status" -eq 0 ]
  [ "$output" = "ok" ]
}

# --- TC7b: check-full-adoption-warning — 直近 2 round 連続 rejected=0 → exit 1 ---

@test "TC7b: 2 round 連続 rejected=0 のとき check-full-adoption-warning は exit 1" {
  bash "$STATE_SH" init "$STATE_FILE" 37 scope-defense
  bash "$STATE_SH" judge "$STATE_FILE" 1 4 0
  bash "$STATE_SH" judge "$STATE_FILE" 2 3 0

  run bash "$STATE_SH" check-full-adoption-warning "$STATE_FILE"
  [ "$status" -eq 1 ]
}

# --- TC7c: check-full-adoption-warning — 直前に棄却あり → exit 0 ---

@test "TC7c: 直前 round に rejected>0 があれば check-full-adoption-warning は exit 0" {
  bash "$STATE_SH" init "$STATE_FILE" 37 scope-defense
  bash "$STATE_SH" judge "$STATE_FILE" 1 4 0
  bash "$STATE_SH" judge "$STATE_FILE" 2 2 1

  run bash "$STATE_SH" check-full-adoption-warning "$STATE_FILE"
  [ "$status" -eq 0 ]
}

# --- TC7d: check-full-adoption-warning — record が 1 件以下 → exit 0 ---

@test "TC7d: judgments が 1 件以下のとき check-full-adoption-warning は exit 0" {
  bash "$STATE_SH" init "$STATE_FILE" 37 scope-defense
  bash "$STATE_SH" judge "$STATE_FILE" 1 3 0

  run bash "$STATE_SH" check-full-adoption-warning "$STATE_FILE"
  [ "$status" -eq 0 ]
}

# --- TC7e: dispatch-codex-auto.sh が Non-Goals なし plan を拒否する ---

@test "TC7e: plan に ## Non-Goals がない場合 dispatch-codex-auto.sh は exit 1" {
  bash "$STATE_SH" init "$STATE_FILE" 37 scope-defense

  PLAN="$BATS_TMPDIR/plan_no_nongoals.md"
  cat > "$PLAN" <<'EOF'
## 実装対象
- Issue: #37
- 影響ファイル: .claude/skills/3ai/SKILL.md

## 設計方針
- 何かを実装する
EOF

  run bash "$AUTO_SH" \
    --issue 37 --mode design \
    --input "$PLAN" --plan "$PLAN" \
    --state "$STATE_FILE" \
    --result "$BATS_TMPDIR/result.md"

  [ "$status" -eq 1 ]
  [[ "$output" =~ "Non-Goals" ]] || [[ "$stderr" =~ "Non-Goals" ]]
}

# --- TC7f: Non-Goals 抽出 + CODEX_DRY_RUN で SCOPE DEFENSE が stdin に含まれる ---

@test "TC7f: CODEX_DRY_RUN=1 のとき SCOPE DEFENSE ブロックが Codex stdin に注入される" {
  bash "$STATE_SH" init "$STATE_FILE" 37 scope-defense

  PLAN="$BATS_TMPDIR/plan_with_nongoals.md"
  cat > "$PLAN" <<'EOF'
## Non-Goals
- フル退化検出は実装しない
- #34 で対応予定の拡張は含めない

## 実装対象
- Issue: #37
EOF

  # codex モック: 実際には呼ばれない (CODEX_DRY_RUN=1)
  # get-review-config.sh → gh が必要なので gh モックを作る
  cat > "$MOCK_BIN/gh" <<'MOCK'
#!/usr/bin/env bash
# --json labels の呼び出しを処理
if [[ "$*" == *"--json labels"* ]]; then
  echo '[{"name":"type: foundation"}]'
  exit 0
fi
# detect-deliverable.sh の gh issue view も処理
echo '[{"name":"type: foundation"}]'
exit 0
MOCK
  chmod +x "$MOCK_BIN/gh"

  # dispatch-codex.sh は CODEX_DRY_RUN=1 で stdin をダンプして exit 0
  # detect-deliverable.sh が git diff を使うかもしれないので git モック
  install_mock_git "$(pwd)"

  AGENTS_DIR="$(cd "$SCRIPTS_DIR/../agents" && pwd)"

  run env CODEX_DRY_RUN=1 bash "$AUTO_SH" \
    --issue 37 --mode design \
    --input "$PLAN" --plan "$PLAN" \
    --state "$STATE_FILE" \
    --result "$BATS_TMPDIR/result.md"

  [ "$status" -eq 0 ]
  [[ "$output" == *"SCOPE DEFENSE"* ]]
  [[ "$output" == *"フル退化検出は実装しない"* ]]
}
