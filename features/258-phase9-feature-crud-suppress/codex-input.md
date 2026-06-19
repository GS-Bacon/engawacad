===== TEST SUMMARY =====
{
  "totals": {
    "passed": 1280,
    "failed": 0,
    "ignored": 17
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
- Suppress の atomic transaction / undo stack (= 単一 op に閉じる)
- Suppress 状態での再生成性能最適化 (Phase 10+)
- 削除された feature 状態 (= delete #260 とは別)。suppress は **再開 (restore) 可能**な inert 化
- `Document::schema_version` の bump (= ADR-015 確定後の別 Issue)
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

## Round 3 方針表明

### A-F01 (round 2 high) 棄却の根拠
restore 経路の `PlaneRef::Entity` の face role 完全解決検証は **#256 で deferred した #270 と完全同じ論点**。本 Issue は履歴 Document の純関数変換 + 基本 invariant check が scope で、PlaneRef::Entity の build-level 検証は scope 外。#270 に統合する。

### M-F01 (round 2 high) 修正済み
`is_body_producer` helper を追加して empty 成功条件を「active body producer が 0」に拡張 + t11_body_producer_suppressed_with_sketch_builds_empty 回帰テスト追加。

