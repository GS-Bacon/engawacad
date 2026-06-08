===== TEST SUMMARY =====
{
  "totals": {
    "passed": 844,
    "failed": 0,
    "ignored": 7
  },
  "by_crate": {},
  "added_in_round": [
    {
      "name": "t01_prop_cut_never_produces_degenerate_manifold",
      "kind": "degenerate"
    },
    {
      "name": "t01_boundary_coplanar_tool_does_not_panic",
      "kind": "degenerate"
    },
    {
      "name": "t02_prop_surface_cut_reduces_volume",
      "kind": "other"
    },
    {
      "name": "t02_degen_minimum_tool_size_ok_or_err",
      "kind": "degenerate"
    },
    {
      "name": "t03_determinism_fixed_seed",
      "kind": "determinism"
    },
    {
      "name": "t02_void_cut_volume_exceeds_initial",
      "kind": "other"
    },
    {
      "name": "t01_large_tool_still_valid",
      "kind": "other"
    },
    {
      "name": "t03_boundary_seed_determinism",
      "kind": "determinism"
    },
    {
      "name": "t03_repeated_determinism_100_runs",
      "kind": "determinism"
    },
    {
      "name": "t04_make_cuboid_nan_returns_err",
      "kind": "boundary"
    },
    {
      "name": "t04_make_cuboid_infinity_returns_err",
      "kind": "boundary"
    },
    {
      "name": "t04_make_cuboid_negative_returns_err",
      "kind": "other"
    },
    {
      "name": "t04_make_cuboid_zero_returns_err",
      "kind": "degenerate"
    },
    {
      "name": "t04_make_cuboid_negative_zero_returns_err",
      "kind": "degenerate"
    },
    {
      "name": "t04_make_cuboid_min_positive_ok",
      "kind": "boundary"
    },
    {
      "name": "t05_empty_solid_validate_manifold_is_consistent",
      "kind": "degenerate"
    },
    {
      "name": "t06_make_cuboid_determinism",
      "kind": "determinism"
    }
  ],
  "coverage_hints": {
    "total_added": 17,
    "determinism": 4,
    "degenerate": 6,
    "boundary": 3,
    "golden": 0,
    "edge_case": 0
  }
}
===== END TEST SUMMARY =====

===== NON-GOALS (SCOPE OUT — Codex はこれらを指摘しないこと) =====
- フォーマット変換 (proptest と serde の組み合わせ)
- GUI/Playwright テスト
- proptest の advanced shrinkage 戦略（デフォルト shrink で十分）
- fuse/intersect の property test（今回は Cut のみ）
===== END NON-GOALS =====

===== KNOWN IGNORED TESTS (理由付き #[ignore] — watertight 不可等の既知制約) =====
- t04_boundary_fuse_extrude_returns_single_body: boolean kernel cannot fuse CreateBox + extrusion (DisjointFuseResult); fuse_target code path is correct in mycad-build
- t04_fuse_target_overlapping_box: boolean kernel cannot fuse cuboid + extrusion (DisjointFuseResult)
- t04b_box_extrude_fuse_via_feature: boolean kernel cannot fuse cuboid + extrusion (DisjointFuseResult)
- t02_reg_negative_extrude_centroid: known bug: #110
- t04_boundary_degen_cut_cyl: known seam mismatch: tracked separately
- t04_watertight_cut_hole: known seam mismatch: tracked separately
- t05_watertight_fuse: known seam mismatch: tracked separately
===== END KNOWN IGNORED TESTS =====

===== NOTE: total_added=0 について =====
テスト名が t01_/t02_/... 形式（test_ prefix なし）の場合 extract-test-summary の
集計に乗らないことがある。git diff で実際の追加テストを確認すること。
===== END NOTE =====
