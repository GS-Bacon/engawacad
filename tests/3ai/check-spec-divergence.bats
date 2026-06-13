#!/usr/bin/env bats
# check-spec-divergence.ts のテスト

load helpers/common

SCRIPT="$SCRIPTS_DIR/check-spec-divergence.ts"

setup() {
  setup_common
  PLAN_FILE="$BATS_TMPDIR/plan.md"
  FEATURE_DIR="$BATS_TMPDIR/feature"
  mkdir -p "$FEATURE_DIR"

  # フィクスチャ: plan.md (T01 / T02 を含む)
  cat > "$PLAN_FILE" <<'EOF'
# テスト計画フィクスチャ

## テスト計画（ID 付き）

| ID | 種別 | 内容 | 期待結果 |
|----|------|------|----------|
| T01 | 決定性 | 同一入力で同一出力 | 完全一致 |
| T02 | 数値正常系 | mesh 体積 ≈ 100.0 - 20π | 相対誤差 < 1% |

## 数値モデル

T02 の理論値: 100.0 - 20π ≈ 37.168 (相対誤差 < 1%)
EOF

  # git diff をモックして FIXTURE_RS を返す mock-git
  FIXTURE_RS="$BATS_TMPDIR/crates/engawa-build/tests/fixture_acceptance.rs"
  mkdir -p "$(dirname "$FIXTURE_RS")"
  cat > "$FIXTURE_RS" <<'EOF'
#[test]
fn t01_determinism() {
    let a = build_mesh();
    let b = build_mesh();
    assert_eq!(a, b);
}

#[test]
fn t02_mesh_volume() {
    let vol = compute_volume();
    let expected = 100.0 - 20.0 * std::f64::consts::PI;
    let rel_err = (vol - expected).abs() / expected.abs();
    assert!(rel_err < 0.01, "rel_err={}", rel_err);
}
EOF

  install_mock_git_diff "$FIXTURE_RS"
}

teardown() {
  teardown_common
}

# git diff のモック: 指定ファイルパスを返す
install_mock_git_diff() {
  local rs_file="$1"
  cat > "$MOCK_BIN/git" <<MOCK
#!/usr/bin/env bash
if [[ "\$1" == "diff" ]]; then
  echo "$rs_file"
  exit 0
fi
exec /usr/bin/git "\$@"
MOCK
  chmod +x "$MOCK_BIN/git"
}

# --- TC1: テスト計画セクションが出力に含まれる ---

@test "TC1: plan.md のテスト計画セクションが出力される" {
  run bun "$SCRIPT" --plan-file "$PLAN_FILE" --feature-dir "$FEATURE_DIR"
  [ "$status" -eq 0 ]
  [[ "$output" == *"## テスト計画"* ]]
  [[ "$output" == *"T01"* ]]
  [[ "$output" == *"T02"* ]]
}

# --- TC2: 数値段落が出力に含まれる ---

@test "TC2: T ID 周辺の数値段落が出力される" {
  run bun "$SCRIPT" --plan-file "$PLAN_FILE" --feature-dir "$FEATURE_DIR"
  [ "$status" -eq 0 ]
  [[ "$output" == *"T02 の理論値"* ]]
  [[ "$output" == *"37.168"* ]]
}

# --- TC3: 実装関数のスニペットが出力に含まれる ---

@test "TC3: t01_ / t02_ にマッチする関数スニペットが出力される" {
  run bun "$SCRIPT" --plan-file "$PLAN_FILE" --feature-dir "$FEATURE_DIR"
  [ "$status" -eq 0 ]
  [[ "$output" == *"t01_determinism"* ]]
  [[ "$output" == *"t02_mesh_volume"* ]]
}

# --- TC4: expected の値がスニペットに含まれる (乖離判定の材料) ---

@test "TC4: 実装の expected 値 (100.0 - 20.0 * PI) がスニペットに含まれる" {
  run bun "$SCRIPT" --plan-file "$PLAN_FILE" --feature-dir "$FEATURE_DIR"
  [ "$status" -eq 0 ]
  [[ "$output" == *"100.0 - 20.0 * std::f64::consts::PI"* ]]
}

# --- TC5: plan-file 未指定は exit 1 ---

@test "TC5: --plan-file 未指定は exit 1 でエラーメッセージを出す" {
  run bun "$SCRIPT"
  [ "$status" -eq 1 ]
  [[ "$output" == *"Usage"* ]]
}
