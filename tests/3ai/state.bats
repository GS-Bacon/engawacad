#!/usr/bin/env bats
# state.sh のユニットテスト（loops.glm_impl のインクリメント）

load helpers/common

STATE_SH="$SCRIPTS_DIR/state.sh"

setup() {
  setup_common
  STATE_FILE="$BATS_TMPDIR/state.json"
}

teardown() {
  teardown_common
}

# --- テストケース 5: loops.glm_impl が 0→1→2 と増加し JSON 構造を維持 ---

@test "TC5: state.sh inc loops.glm_impl が 0→1→2 と単調増加し JSON が壊れない" {
  bash "$STATE_SH" init "$STATE_FILE" 99 test-slug

  # 初期値は loops.glm_impl が無い（または 0）
  run python3 -c "
import json
d = json.load(open('$STATE_FILE'))
val = d.get('loops', {}).get('glm_impl', 0)
print(val)
"
  [ "$output" = "0" ]

  # 1 回インクリメント
  run bash "$STATE_SH" inc "$STATE_FILE" glm_impl
  run python3 -c "
import json
d = json.load(open('$STATE_FILE'))
print(d['loops']['glm_impl'])
"
  [ "$output" = "1" ]

  # 2 回目インクリメント
  bash "$STATE_SH" inc "$STATE_FILE" glm_impl
  run python3 -c "
import json
d = json.load(open('$STATE_FILE'))
print(d['loops']['glm_impl'])
"
  [ "$output" = "2" ]

  # JSON 構造が壊れていない（issue と slug が残っている）
  run python3 -c "
import json
d = json.load(open('$STATE_FILE'))
assert 'issue' in d
assert 'slug' in d
print('ok')
"
  [ "$output" = "ok" ]
}
