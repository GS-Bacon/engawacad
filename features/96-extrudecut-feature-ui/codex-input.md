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
      "name": "test_extrude_cut_yaml_golden",
      "kind": "golden"
    }
  ],
  "coverage_hints": {
    "total_added": 1,
    "determinism": 0,
    "degenerate": 0,
    "boundary": 0,
    "golden": 1,
    "edge_case": 0
  }
}
===== END TEST SUMMARY =====

===== NON-GOALS (SCOPE OUT — Codex はこれらを指摘しないこと) =====
- ThroughAll フラグ / テーパー角・両側カット
- フィレット・面取り
- undo/redo
- 任意平面/面オフセット sketch、フル面(coplanar)cut
- `MultipleOuterShellsResult` を伴う 2-disjoint 分割 cut(テスト前例なし・本 Issue では生成しない)
===== END NON-GOALS =====

===== NOTE: total_added=0 について =====
テスト名が t01_/t02_/... 形式（test_ prefix なし）の場合 extract-test-summary の
集計に乗らないことがある。git diff で実際の追加テストを確認すること。
===== END NOTE =====
