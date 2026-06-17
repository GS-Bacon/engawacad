===== TEST SUMMARY =====
{
  "totals": {
    "passed": 1013,
    "failed": 0,
    "ignored": 22
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
- スケッチ描画 e2e — #216 で対応
- Extrude/ExtrudeCut の Face plane_ref 押出本体 — #217/#218 で対応 (本 Issue は dispatcher の plane 解決のみ)
- 非平面 Face からの平面導出 — Phase 10 以降
- EntityRef::Derived 解決 — Phase 9 以降
- Edge/Vertex の逆引き本実装 — Phase 9 以降
- TypeScript binding (TS) の export — JsonSchema は付与するが ts-rs export は別 Issue で
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
- t01_determinism_face_entity_ref_planeref: STEP 6 で実装後に解除
- t02_legacy_string_planeref_backward_compat: STEP 6 で実装後に解除
- t03_entity_face_planeref_with_extrude: STEP 6 で実装後に解除
- t04_entity_face_planeref_with_extrude_cut: STEP 6 で実装後に解除
- t05_yaml_roundtrip_legacy_string: STEP 6 で実装後に解除
- t06_yaml_roundtrip_entity_ref: STEP 6 で実装後に解除
- t07_degen_unknown_entity_ref: STEP 6 で実装後に解除
- t08_degen_non_planar_face: STEP 6 で実装後に解除
- t09_boundary_derived_entity_ref: STEP 6 で実装後に解除
- t10_inline_golden_roundtrip_new_type: STEP 6 で実装後に解除
- t11_existing_refplane_acceptance_compat: STEP 6 で実装後に解除
- t04_negative_extrude_fuse_integration: known limitation: boolean engine produces non-manifold result for offset-plane fuse
- regen_all_fixtures: run manually to regenerate viewer fixtures after tessellation changes
- t10_blocked_rotation_cut_sphere: blocked: #137 trimmed sphere tessellation
===== END KNOWN IGNORED TESTS =====

===== NOTE: total_added=0 について =====
テスト名が t01_/t02_/... 形式（test_ prefix なし）の場合 extract-test-summary の
集計に乗らないことがある。git diff で実際の追加テストを確認すること。
===== END NOTE =====
