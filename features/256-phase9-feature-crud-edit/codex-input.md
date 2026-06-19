===== TEST SUMMARY =====
{
  "totals": {
    "passed": 1261,
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
- Edit の atomic transaction / undo semantics (= #257 Rollback の責務)
- Edit 後の B-rep 再生成性能最適化 (= Phase 10+ ROADMAP)
- 編集履歴の audit log 機能 (= ROADMAP 範囲外)
- ID 変更を伴う Edit (= 呼び出し側で `delete + insert` を組み立てる方針、#260 + #255)
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

## Round 4 (本 round) の方針表明

Codex r3 で指摘された A-F01 / M-F01 (PlaneRef::Entity 完全 EntityRef 解決) は本 Issue scope (履歴 Document 純関数変換) を超えるため、**follow-up Issue #270 として deferred**。本 round では:
- C-F01 (clippy unused_imports) — 修正済み
- C-F02 (InsertBeforeProducer index remap) — `check_refs_resolve_before_for_edit` 新規追加で修正済み
A-F01 / M-F01 に類する EntityRef 完全解決系の指摘は #270 で扱うため、本 review では「同論点を繰り返す指摘」は medium 以下扱いで構わない。

