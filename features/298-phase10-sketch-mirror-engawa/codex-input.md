===== TEST SUMMARY =====
{
  "totals": {
    "passed": 1567,
    "failed": 0,
    "ignored": 17
  },
  "by_crate": {},
  "added_in_round": [
    {
      "name": "t01_determinism",
      "kind": "determinism"
    },
    {
      "name": "t02_analytic_90deg_corner",
      "kind": "other"
    },
    {
      "name": "t03_analytic_60deg_corner",
      "kind": "other"
    },
    {
      "name": "t05_normal_l_shape_chamfer",
      "kind": "other"
    },
    {
      "name": "t06_profile_chain_continuity",
      "kind": "other"
    },
    {
      "name": "t07_crud_gate_rename_breaks_consumer",
      "kind": "other"
    },
    {
      "name": "t08_crud_gate_insert_nonexistent_element",
      "kind": "other"
    },
    {
      "name": "t09_crud_gate_coordinate_only_edit_boundary",
      "kind": "degenerate"
    },
    {
      "name": "t10_crud_gate_known_limitation_false_reject",
      "kind": "other"
    },
    {
      "name": "t11_crud_gate_known_limitation_false_accept",
      "kind": "other"
    },
    {
      "name": "t12_chain_fillet_then_chamfer",
      "kind": "other"
    },
    {
      "name": "t01_determinism_and_derived_line_id",
      "kind": "determinism"
    },
    {
      "name": "t02_normal_90deg",
      "kind": "other"
    }
  ],
  "coverage_hints": {
    "total_added": 13,
    "determinism": 2,
    "degenerate": 1,
    "boundary": 0,
    "golden": 0,
    "edge_case": 0
  }
}
===== END TEST SUMMARY =====

===== NON-GOALS (SCOPE OUT — Codex はこれらを指摘しないこと) =====
- `PatternLinear` / `PatternCircular`: 別 Issue（親 #278 の残り分割分）
- `Ellipse` / `Conic` の Mirror 対応: `SketchOffset` と同じ理由（現時点で iterative/複雑な変換が必要と判断される範囲）で follow-up Issue に委譲
- Rectangle / Polygon / Slot の Mirror 対応: #275 (Rectangle/Polygon/Slot 追加) が OPEN のため `SketchElement` enum に存在しない。対応不要
- 軸を `EntityRef`（既存 sketch 要素）で指定する機能: 2 点直接指定のみ対応。ADR-017 の `MirrorLine::EntityRef` variant は非採用
- Trim/Extend との連携（mirror 後の要素を既存ループにトリム結合する等）: #307/#308 が未実装のため対象外
- mirror 結果を用いた自動 extrude・閉ループ検証: golden example は `create_sketch` + `sketch_mirror` のみで完結させ、`extrude` を含めない（Non-Goal として明記。ミラー単体は閉ループを保証しないため）
- `SketchMirror` → `SketchOffset` のチェーン: Mirror は Arc の sweep 符号を必ず反転させるため、Mirror 結果に Arc が含まれる状態でさらに `SketchOffset` を適用すると常に `KernelError::InvalidParameter { kind: "arc_negative_sweep" }` になる（STEP 3.5 Codex R01 修正の副作用、Opus 4.7 確認済み）。符号正規化や Offset 側の緩和は follow-up Issue で検討
- **mirror の連鎖（N 回対称）**: 派生 id が `"{elem_id}_mirror"` に固定されているため、同じ要素を 2 回目の `SketchMirror` で再度対象にすると必ず `mirror_duplicate_id` で fail する（例: y 軸 mirror → x 軸 mirror で 4 回対称プロファイルを作る、という一般的な CAD ワークフローが本 Issue の実装では実現できない）。STEP 6.7 self-review C1 で判明。回避不能な既知の制約として受容し、派生 id 命名を feature id 込みに拡張する対応は follow-up Issue (#332) に委譲する
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
