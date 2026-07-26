===== TEST SUMMARY =====
{
  "totals": {
    "passed": 1479,
    "failed": 0,
    "ignored": 17
  },
  "by_crate": {},
  "added_in_round": [
    {
      "name": "t01_determinism_and_derived_arc_id",
      "kind": "determinism"
    },
    {
      "name": "t04_build_rectangle_corner_fillet",
      "kind": "other"
    },
    {
      "name": "t06_extrude_uses_filleted_profile",
      "kind": "other"
    },
    {
      "name": "t07_closed_loop_wraparound_corner",
      "kind": "other"
    },
    {
      "name": "t08_cw_profile_negative_sweep",
      "kind": "other"
    },
    {
      "name": "t09_profile_chain_continuity",
      "kind": "other"
    },
    {
      "name": "t10_crud_gate_rejects_rename_breaking_fillet",
      "kind": "other"
    },
    {
      "name": "t11_crud_gate_rejects_insert_with_missing_element",
      "kind": "other"
    },
    {
      "name": "t12_crud_gate_rejects_reorder_breaking_adjacency",
      "kind": "other"
    },
    {
      "name": "t01_determinism_and_derived_arc_id",
      "kind": "determinism"
    },
    {
      "name": "t02_normal_90deg",
      "kind": "other"
    },
    {
      "name": "t03_normal_60deg",
      "kind": "other"
    },
    {
      "name": "t09_profile_chain_continuity",
      "kind": "other"
    }
  ],
  "coverage_hints": {
    "total_added": 13,
    "determinism": 2,
    "degenerate": 0,
    "boundary": 0,
    "golden": 0,
    "edge_case": 0
  }
}
===== END TEST SUMMARY =====

===== NON-GOALS (SCOPE OUT — Codex はこれらを指摘しないこと) =====
- Sketch Chamfer (#297)、Sketch Trim / Extend (#307/#308) — 別 Issue で個別実装 (本 Issue はそれらに依存しない、並行実装可)
- Arc-Arc / Line-Arc / Circle / Ellipse / Conic を含む fillet (正接円構築の数式が別、独立 Issue で提案)
- 3要素以上の fillet chain、複数コーナーの一括 fillet
- 一般的な `SelfIntersection` 判定 (本 Issue は `elem1`/`elem2` 自身の長さ制約のみで代替、SCOPE DEFENSE 節参照)
- Fillet 適用後の profile に対する再 fillet (elem1_id/elem2_id は常に Line である前提。既に fillet 済みの Arc を挟んだ状態からの再 fillet は対象外)
- `CURRENT_SCHEMA_VERSION` の bump (#295 Non-Goals と同理由: 加算的 variant 追加は breaking ではない)
- Property test 網羅 (本 Issue は fixed input のテストのみ、Refactor Pass で追加)
- 負 sweep Arc に対する `sketch_offset` 対応 (`offset_arc` の `arc_negative_sweep` 拒否の緩和)。現状 Circle-only guard により到達不能な潜在衝突であり、#295 の guard が外れる Phase 11+ で扱う
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
- t02_topology_pinned_until_220: blocked by #220: degenerate boolean Cut produces non-manifold result
- t03_hole_bottom_vertex_at_inner_depth: blocked by #220: real hole drilling requires kernel boolean fix
- t04_resize_cuboid_same_entityref_hole_still_drilled: blocked by #220: real hole drilling requires kernel boolean fix
- t10_very_small_depth: blocked by #220: cut behavior unreliable; needs manifold contract
- t04_negative_extrude_fuse_integration: known limitation: boolean engine produces non-manifold result for offset-plane fuse
- regen_all_fixtures: run manually to regenerate viewer fixtures after tessellation changes
- t10_blocked_rotation_cut_sphere: blocked: #137 trimmed sphere tessellation
===== END KNOWN IGNORED TESTS =====

===== NOTE: total_added=0 について =====
テスト名が t01_/t02_/... 形式（test_ prefix なし）の場合 extract-test-summary の
集計に乗らないことがある。git diff で実際の追加テストを確認すること。
===== END NOTE =====
