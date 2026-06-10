===== TEST SUMMARY =====
{
  "totals": {
    "passed": 0,
    "failed": 0,
    "ignored": 0
  },
  "by_crate": {},
  "added_in_round": [
    {
      "name": "test_adjacent_face_idx_sphere_cap",
      "kind": "other"
    },
    {
      "name": "test_adjacent_face_idx_plane_cap",
      "kind": "other"
    },
    {
      "name": "t01_determinism_cross_face_nu",
      "kind": "determinism"
    },
    {
      "name": "t04_boundary_primitive_cylinder_angular_segments",
      "kind": "degenerate"
    },
    {
      "name": "t05_degen_non_sphere_adjacency_no_panic",
      "kind": "degenerate"
    }
  ],
  "coverage_hints": {
    "total_added": 5,
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
- メッシュ出力の変更: 本変更は挙動保存（box∩cyl→n_u=64, cyl∩sphere→n_u=32 を現行と同値で導出）。positions/indices は不変。
- 性能最適化: `adjacent_face_idx` は O(n) 線形走査のまま（隣接キャッシュは導入しない）。
- 混在キャップ円柱（上=平面 / 下=球）の厳密対応: 現状到達不能。`any(Sphere)` で球優先のみ。該当時は別 Issue。
- Cone キャップ: Out-of-Scope。Plane と同じ `arcs_per_rev` 分岐に落ちる（既存挙動踏襲）。
===== END NON-GOALS =====

===== NOTE: total_added=0 について =====
テスト名が t01_/t02_/... 形式（test_ prefix なし）の場合 extract-test-summary の
集計に乗らないことがある。git diff で実際の追加テストを確認すること。
===== END NOTE =====

===== F01 対応状況（前回 Codex 指摘への回答） =====
前回指摘: "`adjacent_face_idx` を削除しても acceptance テストが通る"
対応: tessellation/mod.rs の #[cfg(test)] mod tests に以下を追加済み（diff に含まれる）

- test_adjacent_face_idx_sphere_cap: cyl∩sphere から cylinder lateral face の
  Circle HE を取り、adjacent_face_idx を直接呼び Surface::Sphere を assert する。
  helper を削除 or None 返しに変えると失敗する。
- test_adjacent_face_idx_plane_cap: box∩cyl から同様、Surface::Plane を assert する。
  helper を削除 or None 返しに変えると失敗する。

これらのテストは今回の diff の tessellation/mod.rs 末尾に存在する。
実行: cargo test -p mycad-kernel --lib -- tessellation::tests::test_adjacent_face_idx
結果: 2 passed, 0 failed（CI green 確認済み）
===== END F01 対応状況 =====
