===== TEST SUMMARY =====
{
  "totals": {
    "passed": 763,
    "failed": 0,
    "ignored": 3
  },
  "by_crate": {},
  "added_in_round": [
    {
      "name": "t01_determinism",
      "kind": "determinism"
    },
    {
      "name": "t02_length_invariant_cuboid",
      "kind": "other"
    },
    {
      "name": "t03_cuboid_face_ids_correct",
      "kind": "other"
    },
    {
      "name": "t04_length_invariant_cylinder_sphere",
      "kind": "other"
    },
    {
      "name": "t05_boundary_unnamed_face",
      "kind": "degenerate"
    },
    {
      "name": "t06_degen_skip_invariant",
      "kind": "degenerate"
    }
  ],
  "coverage_hints": {
    "total_added": 6,
    "determinism": 1,
    "degenerate": 2,
    "boundary": 0,
    "golden": 0,
    "edge_case": 0
  }
}
===== END TEST SUMMARY =====

===== NON-GOALS (SCOPE OUT — Codex はこれらを指摘しないこと) =====
<!-- Out-of-Scope と同内容でも重複 OK。dispatch-codex-auto.ts の guard が参照する。 -->
- viewer-pick の実装（#94）
- 書き込み API（#93）
- face_ids を使った任意の集約・フィルタリング
- EntityRef を新たな wire フォーマットで露出すること（文字列化のみ。`EntityRef` 構造体そのものを TS に出すのは本 Issue 対象外）
===== END NON-GOALS =====

===== KNOWN IGNORED TESTS (理由付き #[ignore] — watertight 不可等の既知制約) =====
- t04_boundary_degen_cut_cyl: known seam mismatch: tracked separately
- t04_watertight_cut_hole: known seam mismatch: tracked separately
- t05_watertight_fuse: known seam mismatch: tracked separately
===== END KNOWN IGNORED TESTS =====

===== NOTE: total_added=0 について =====
テスト名が t01_/t02_/... 形式（test_ prefix なし）の場合 extract-test-summary の
集計に乗らないことがある。git diff で実際の追加テストを確認すること。
===== END NOTE =====
