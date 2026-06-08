===== TEST SUMMARY =====
{
  "totals": {
    "passed": 862,
    "failed": 0,
    "ignored": 7
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
- 円弧セグメントと直線セグメントが混在して閉内部ループを形成するケースの修正
- tool ループ側（tool 面が target 面の内部で閉ループを形成するケース）の修正
- STEP ファイル export における pcurve/t_range の厳密な provenance 伝播
- edge naming の provenance 精度向上（inner_boundary_partners は None で OK）
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
