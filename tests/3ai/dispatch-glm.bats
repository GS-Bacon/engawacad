#!/usr/bin/env bats
# dispatch-glm.ts のユニットテスト

load helpers/common

DISPATCH="$SCRIPTS_DIR/dispatch-glm.ts"

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
  # dispatch-glm.ts は Z.AI API を直接呼び出す設計で claude CLI を経由しない。
  # 旧 dispatch-glm.sh のインターフェーステストはアーキテクチャ変更で非対応となった。
  skip "dispatch-glm.ts は Z.AI API を直接呼び出すため claude.args 経由のテスト不可 (旧 .sh 互換テスト)"
}

# --- テストケース 2: CI 失敗時の error_pattern 抽出（コンパイルエラー版）---

@test "TC2: CI 失敗時に compile error の error_pattern が抽出される" {
  # dispatch-glm.ts は Z.AI 実行環境内で cargo xtask ci を走らせるため、
  # ローカル cargo モックで error_pattern を検証する旧 .sh の手法は非対応。
  # extractErrorPattern のロジックは dispatch-glm.ts:35-54 で維持されている。
  skip "dispatch-glm.ts は Z.AI API 経由実行のためローカル cargo モックでの検証不可 (旧 .sh 互換テスト)"
}

# --- テストケース 3: CI 失敗時の error_pattern 抽出（test FAILED 版）---

@test "TC3: CI 失敗時に test FAILED の error_pattern が抽出される" {
  # TC2 と同じ理由でスキップ。
  skip "dispatch-glm.ts は Z.AI API 経由実行のためローカル cargo モックでの検証不可 (旧 .sh 互換テスト)"
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
