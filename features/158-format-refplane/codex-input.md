===== TEST SUMMARY =====
{
  "totals": {
    "passed": 993,
    "failed": 0,
    "ignored": 12
  },
  "by_crate": {},
  "added_in_round": [
    {
      "name": "t01_determinism",
      "kind": "determinism"
    },
    {
      "name": "t05_new_format_e2e",
      "kind": "other"
    },
    {
      "name": "t06_plane_ref_priority_over_plane",
      "kind": "other"
    },
    {
      "name": "t07_legacy_plane_offset_still_works",
      "kind": "other"
    },
    {
      "name": "t10_degen_unknown_plane_ref",
      "kind": "degenerate"
    },
    {
      "name": "t11_boundary_empty_plane_ref_string",
      "kind": "degenerate"
    },
    {
      "name": "test_default_canonical_three_order",
      "kind": "other"
    },
    {
      "name": "test_default_canonical_three_deterministic",
      "kind": "determinism"
    },
    {
      "name": "test_is_default_canonical_three_true",
      "kind": "other"
    },
    {
      "name": "test_is_default_canonical_three_false_wrong_order",
      "kind": "other"
    },
    {
      "name": "test_is_default_canonical_three_false_custom_offset",
      "kind": "other"
    },
    {
      "name": "test_is_default_canonical_three_false_extra_plane",
      "kind": "other"
    },
    {
      "name": "test_is_default_canonical_three_false_missing_plane",
      "kind": "other"
    },
    {
      "name": "test_front_top_right_constructors",
      "kind": "other"
    },
    {
      "name": "test_ref_plane_serialization",
      "kind": "other"
    },
    {
      "name": "test_ref_plane_zero_offset_skipped",
      "kind": "degenerate"
    },
    {
      "name": "test_ref_plane_nonzero_offset_included",
      "kind": "degenerate"
    },
    {
      "name": "t02_document_new_seeds_three_refplanes",
      "kind": "other"
    },
    {
      "name": "t03_from_yaml_seeds_three_refplanes",
      "kind": "golden"
    },
    {
      "name": "t04_legacy_examples_roundtrip",
      "kind": "golden"
    },
    {
      "name": "t08_yaml_golden_new_format",
      "kind": "golden"
    },
    {
      "name": "t09_yaml_golden_document_with_explicit_refplanes",
      "kind": "golden"
    },
    {
      "name": "t12_degen_duplicate_refplane_id",
      "kind": "degenerate"
    },
    {
      "name": "t13_default_three_skips_serialize",
      "kind": "golden"
    },
    {
      "name": "t14_explicit_three_with_custom_offset_serializes",
      "kind": "golden"
    }
  ],
  "coverage_hints": {
    "total_added": 25,
    "determinism": 2,
    "degenerate": 5,
    "boundary": 0,
    "golden": 6,
    "edge_case": 0
  }
}
===== END TEST SUMMARY =====

===== NON-GOALS (SCOPE OUT — Codex はこれらを指摘しないこと) =====
- UI 側 (Three.js 半透明描画 / raycaster クリック検出) は本 Issue では扱わない (#159 で対応)
- 既存 `examples/*.engawa` のフォーマット変更は行わない (round-trip byte-identical を維持)
- `plane_ref` と `plane`/`offset` の両立時に警告ログを出す処理は本 Issue では未対応
- `RefPlane` をユーザーが追加 / 削除する API (Phase 8 で対応)
- `RefPlane.offset` の UI 露出は Phase 7 では行わない (Phase 8 で対応)
- `ExtrudeCut` の `_offset` を捨てているバグの修正は別 Issue
- kernel 層の変更は一切行わない
===== END NON-GOALS =====

===== KNOWN IGNORED TESTS (理由付き #[ignore] — watertight 不可等の既知制約) =====
- t04_boundary_fuse_extrude_returns_single_body: boolean kernel cannot fuse CreateBox + extrusion (DisjointFuseResult); fuse_target code path is correct in engawa-build
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
===== END KNOWN IGNORED TESTS =====

===== NOTE: total_added=0 について =====
テスト名が t01_/t02_/... 形式（test_ prefix なし）の場合 extract-test-summary の
集計に乗らないことがある。git diff で実際の追加テストを確認すること。
===== END NOTE =====
