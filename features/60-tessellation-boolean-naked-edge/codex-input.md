===== TEST SUMMARY =====
{
  "totals": {
    "passed": 639,
    "failed": 0,
    "ignored": 3
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
- Boolean テッセレーションの既存バグ修正（#56 以降の残課題は別 Issue で対応）
- `mycad-kernel` の公開 API 追加
- 数値公差のチューニング
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
