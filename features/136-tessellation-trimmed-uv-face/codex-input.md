===== TEST SUMMARY =====
{
  "totals": {
    "passed": 934,
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
- **#137 (trimmed sphere tessellation)**: 球は φ-θ 二重周期で、本 Issue の単一 u シフト修正では対応不可。別 Issue。
- **earcut アルゴリズム自体の見直し**: earcut への入力 (UV 座標) を正しく整えるのが本 Issue。earcut 自体の不具合 (穴と外周の交差等) は対象外。
- **外周ループ自体の unwrap 再設計**: L443-450 の外周 unwrap は既に動作しており、本 Issue 範囲外。
- **`tessellate_face_uv_grid` の path 分岐見直し**: L551-553 の早期 delegate は変更しない。
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
