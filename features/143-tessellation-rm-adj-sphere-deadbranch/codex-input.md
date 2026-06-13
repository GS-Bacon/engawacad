===== TEST SUMMARY =====
{
  "totals": {
    "passed": 953,
    "failed": 0,
    "ignored": 11
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
<!-- 該当なし禁止。dispatch-codex-auto.ts の guard が参照する。 -->

- 共有境界一致の構造的保証 (ADR-009 Phase 2 本体の責務)
- `n_u` ヒューリスティクス全体の撤去 (ADR-009 Phase 2)
- `partition.rs` 側の変更
- `ANGULAR_SEGMENTS_DEFAULT` の意味変更
- `adjacent_face_idx` 関数本体の削除
- mixed-cap cylinder (top=Sphere / bottom=Plane) サポート
- inline テスト `test_adjacent_face_idx_sphere_cap` / `test_adjacent_face_idx_plane_cap` の削除 (これらは `adjacent_face_idx` 関数のキャラクタリゼーションとして温存)
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
===== END KNOWN IGNORED TESTS =====

===== NOTE: total_added=0 について =====
テスト名が t01_/t02_/... 形式（test_ prefix なし）の場合 extract-test-summary の
集計に乗らないことがある。git diff で実際の追加テストを確認すること。
===== END NOTE =====
