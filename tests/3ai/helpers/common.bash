#!/usr/bin/env bash
# 共通ヘルパー: tmpdir / PATH 上書き / モックスタブ

SCRIPTS_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../../../.claude/skills/3ai/scripts" && pwd)"
HELPERS_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
export SCRIPTS_DIR HELPERS_DIR

setup_common() {
  BATS_TMPDIR="$(mktemp -d)"
  export BATS_TMPDIR
  MOCK_BIN="$BATS_TMPDIR/bin"
  mkdir -p "$MOCK_BIN"
  export PATH="$MOCK_BIN:$PATH"
}

teardown_common() {
  rm -rf "$BATS_TMPDIR"
}

# claude モック: プロンプトを $BATS_TMPDIR/claude.stdin に保存、引数を claude.args に保存
# 呼び出し後、正常終了して空の .raw ファイルを生成する
install_mock_claude() {
  local result_raw="${1:-}"
  cat > "$MOCK_BIN/claude" <<MOCK
#!/usr/bin/env bash
echo "\$@" > "\$BATS_TMPDIR/claude.args"
cat > "\$BATS_TMPDIR/claude.stdin"  # stdin を保存
if [[ -n "$result_raw" ]]; then
  echo "$result_raw"
fi
exit 0
MOCK
  chmod +x "$MOCK_BIN/claude"
}

# cargo モック: cargo xtask ci を失敗させ、固定の ci.log を渡す
install_mock_cargo_fail() {
  local ci_log_content="$1"
  cat > "$MOCK_BIN/cargo" <<MOCK
#!/usr/bin/env bash
if [[ "\$1" == "xtask" && "\$2" == "ci" ]]; then
  echo "$ci_log_content"
  exit 1
fi
exec /usr/bin/cargo "\$@"
MOCK
  chmod +x "$MOCK_BIN/cargo"
}

# cargo モック: cargo xtask ci を成功させる
install_mock_cargo_pass() {
  cat > "$MOCK_BIN/cargo" <<MOCK
#!/usr/bin/env bash
if [[ "\$1" == "xtask" && "\$2" == "ci" ]]; then
  echo "All CI checks passed"
  exit 0
fi
exec /usr/bin/cargo "\$@"
MOCK
  chmod +x "$MOCK_BIN/cargo"
}

# git モック: rev-parse --show-toplevel を返す
install_mock_git() {
  local root_dir="$1"
  cat > "$MOCK_BIN/git" <<MOCK
#!/usr/bin/env bash
if [[ "\$1" == "rev-parse" && "\$2" == "--show-toplevel" ]]; then
  echo "$root_dir"
  exit 0
fi
exec /usr/bin/git "\$@"
MOCK
  chmod +x "$MOCK_BIN/git"
}

# 最小限の .env ファイルを作成（Z_AI_API_KEY をセット）
create_fake_zai_env() {
  local envfile="$BATS_TMPDIR/zai.env"
  echo 'Z_AI_API_KEY=test-key-for-bats' > "$envfile"
  echo "$envfile"
}
