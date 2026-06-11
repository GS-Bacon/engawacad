===== TEST SUMMARY =====
{
  "totals": {
    "passed": 925,
    "failed": 0,
    "ignored": 15
  },
  "by_crate": {},
  "added_in_round": [],
  "coverage_hints": {
    "total_added": 0,
    "determinism": 0,
    "degenerate": 0,
    "boundary": 0,
    "golden": 0,
    "edge_case": 0
  }
}
===== END TEST SUMMARY =====

===== NON-GOALS (SCOPE OUT — Codex はこれらを指摘しないこと) =====
- **rotation × Boolean Cut Sphere の組合せテスト**: #136 / #137 が未解決のため、決定性とトポロジーの両方で不安定。本 Issue では `#[ignore = "blocked: #137"]` の skeleton のみ置く。
- **`euler_to_matrix_deg` ラッパの提供**: ADR-004 整合のためのシグネチャ変更は一括差し替え。既存テストの追従修正は本 Issue スコープ内。
- **format 層の `Transform` 型変更**: 既存 `.mycad` ファイル互換のため `rotation: [f64; 3]` (deg) を維持する。
- **deg/rad 識別を型レベルで保証する newtype 導入**: 別 Issue 候補 (ADR-004 補強). 本 Issue では関数境界のコメント明示のみ。
- **examples ファイルに新規 rotation 例を追加**: 本 Issue は配線とテスト追加に絞る。
===== END NON-GOALS =====

===== KNOWN IGNORED TESTS (理由付き #[ignore] — watertight 不可等の既知制約) =====
- t04_boundary_fuse_extrude_returns_single_body: boolean kernel cannot fuse CreateBox + extrusion (DisjointFuseResult); fuse_target code path is correct in mycad-build
- t01_determinism: fuzz: run with cargo xtask acceptance --fuzz
- t02_fuzz_no_http_500: fuzz: run with cargo xtask acceptance --fuzz
- t03_degen_special_float_values: fuzz: run with cargo xtask acceptance --fuzz
- t04_degen_unknown_type: fuzz: run with cargo xtask acceptance --fuzz
- t05_degen_null_body: fuzz: run with cargo xtask acceptance --fuzz
- t04_fuse_target_overlapping_box: boolean kernel cannot fuse cuboid + extrusion (DisjointFuseResult)
- t04b_box_extrude_fuse_via_feature: boolean kernel cannot fuse cuboid + extrusion (DisjointFuseResult)
- t04_negative_extrude_fuse_integration: known limitation: boolean engine produces non-manifold result for offset-plane fuse
- regen_all_fixtures: run manually to regenerate viewer fixtures after tessellation changes
- t10_blocked_rotation_cut_sphere: blocked: #137 trimmed sphere tessellation
- t01_kernel_neg_depth_determinism: STEP 6 で実装後に解除
- t02_kernel_neg_depth_manifold: STEP 6 で実装後に解除
- t_boundary_zero_depth_rejected: STEP 6 で実装後に解除
- t_degen_nonfinite_depth_rejected: STEP 6 で実装後に解除
===== END KNOWN IGNORED TESTS =====

===== NOTE: total_added=0 について =====
テスト名が t01_/t02_/... 形式（test_ prefix なし）の場合 extract-test-summary の
集計に乗らないことがある。git diff で実際の追加テストを確認すること。
===== END NOTE =====
