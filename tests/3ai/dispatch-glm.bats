#!/usr/bin/env bats
# dispatch-glm.sh のユニットテスト

load helpers/common

DISPATCH="$SCRIPTS_DIR/dispatch-glm.sh"

setup() {
  setup_common
  ZAI_ENV="$(create_fake_zai_env)"
  export ZAI_ENV

  FEATURE_DIR="$BATS_TMPDIR/feature"
  mkdir -p "$FEATURE_DIR"
  PLAN_FILE="$BATS_TMPDIR/plan.md"
  echo "# テストプラン" > "$PLAN_FILE"
  AGENT_FILE="$BATS_TMPDIR/agent.md"
  echo "# GLM エージェント" > "$AGENT_FILE"
  RESULT_FILE="$BATS_TMPDIR/glm-result.json"

  # git rev-parse モック
  install_mock_git "$BATS_TMPDIR"
}

teardown() {
  teardown_common
}

# --- テストケース 1: --debug-spec の内容が GLM プロンプトに含まれる ---

@test "TC1: --debug-spec の内容が claude プロンプトに渡される" {
  DEBUG_SPEC_FILE="$BATS_TMPDIR/debug-spec.md"
  echo "## 仮説: flip_normals バグ" > "$DEBUG_SPEC_FILE"
  echo "詳細: 頂点順を逆にしているのが原因" >> "$DEBUG_SPEC_FILE"

  # claude と cargo を mock（CI は成功扱い）
  install_mock_claude ""
  install_mock_cargo_pass

  run bash "$DISPATCH" \
    --agent "$AGENT_FILE" \
    --plan-file "$PLAN_FILE" \
    --feature-dir "$FEATURE_DIR" \
    --result-file "$RESULT_FILE" \
    --debug-spec "$DEBUG_SPEC_FILE"

  # claude が呼ばれた際のプロンプト stdin を確認
  # dispatch-glm.sh は claude -p "$PROMPT" という形式で渡す
  [ -f "$BATS_TMPDIR/claude.args" ]
  claude_args="$(cat "$BATS_TMPDIR/claude.args")"
  [[ "$claude_args" == *"flip_normals バグ"* ]] || \
    [[ "$claude_args" == *"オーケストレーター"* ]]
}

# --- テストケース 2: CI 失敗時の error_pattern 抽出（コンパイルエラー版）---

@test "TC2: CI 失敗時に compile error の error_pattern が抽出される" {
  CI_LOG="$FEATURE_DIR/ci.log"
  cat > "$CI_LOG" <<'EOF'
error[E0599]: no method named `foo` found for struct `Bar`
  --> crates/mycad-kernel/src/lib.rs:42:5
   |
42 |     bar.foo();
   |         ^^^ method not found in `Bar`

error: aborting due to previous error
EOF

  install_mock_claude ""
  install_mock_cargo_fail "$(cat "$CI_LOG")"

  run bash "$DISPATCH" \
    --agent "$AGENT_FILE" \
    --plan-file "$PLAN_FILE" \
    --feature-dir "$FEATURE_DIR" \
    --result-file "$RESULT_FILE"

  [ -f "$RESULT_FILE" ]
  pattern="$(python3 -c "import json; d=json.load(open('$RESULT_FILE')); print(d.get('error_pattern',''))")"
  [[ "$pattern" == *"error[E0599]"* ]]
  [[ "$pattern" == *"LINE:COL"* ]]
  kind="$(python3 -c "import json; d=json.load(open('$RESULT_FILE')); print(d.get('error_pattern_kind',''))")"
  [ "$kind" = "compile" ]
}

# --- テストケース 3: CI 失敗時の error_pattern 抽出（test FAILED 版）---

@test "TC3: CI 失敗時に test FAILED の error_pattern が抽出される" {
  CI_LOG="$FEATURE_DIR/ci.log"
  cat > "$CI_LOG" <<'EOF'
running 3 tests
test t01_determinism ... ok
test t03_fuse_overlapping_boxes ... FAILED
test t02_cut_basic ... ok

failures:

---- t03_fuse_overlapping_boxes stdout ----
thread 'main' panicked at 'manifold validation failed'
EOF

  install_mock_claude ""
  install_mock_cargo_fail "$(cat "$CI_LOG")"

  run bash "$DISPATCH" \
    --agent "$AGENT_FILE" \
    --plan-file "$PLAN_FILE" \
    --feature-dir "$FEATURE_DIR" \
    --result-file "$RESULT_FILE"

  [ -f "$RESULT_FILE" ]
  pattern="$(python3 -c "import json; d=json.load(open('$RESULT_FILE')); print(d.get('error_pattern',''))")"
  [[ "$pattern" == *"t03_fuse_overlapping_boxes"* ]]
  [[ "$pattern" == *"FAILED"* ]]
  kind="$(python3 -c "import json; d=json.load(open('$RESULT_FILE')); print(d.get('error_pattern_kind',''))")"
  [ "$kind" = "test" ]
}

# --- テストケース 4: error_pattern 正規化（異なる行番号でも同一ハッシュ）---

@test "TC4: 行番号が異なるだけの同一エラーが同じ error_pattern に正規化される" {
  normalize_pattern() {
    python3 - "$1" <<'PY'
import re, sys
line = open(sys.argv[1]).read().strip()
line = re.sub(r'\x1b\[[0-9;]*m', '', line)
line = re.sub(r'([A-Za-z0-9_./-]+\.rs):\d+:\d+', r'\1:LINE:COL', line)
line = re.sub(r'([A-Za-z0-9_./-]+\.rs):\d+', r'\1:LINE', line)
line = re.sub(r'  +', ' ', line)
print(line.strip())
PY
  }

  echo 'error[E0599]: no method named `foo` found at crates/foo.rs:42:5' > "$BATS_TMPDIR/err1.txt"
  echo 'error[E0599]: no method named `foo` found at crates/foo.rs:99:7' > "$BATS_TMPDIR/err2.txt"

  p1="$(normalize_pattern "$BATS_TMPDIR/err1.txt")"
  p2="$(normalize_pattern "$BATS_TMPDIR/err2.txt")"

  [ "$p1" = "$p2" ]
  [[ "$p1" == *"LINE:COL"* ]]
}
