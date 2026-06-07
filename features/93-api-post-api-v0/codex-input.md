===== TEST SUMMARY =====
{
  "totals": {
    "passed": 781,
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
- WebSocket / プッシュ通知
- Feature 削除・編集・並べ替えエンドポイント
- 認証・マルチユーザー
- root_component 以外（子 component）への Feature 追加
- きめ細かい並行制御（`std::sync::Mutex` で GET/POST を全直列化する。Phase 6 は単一ユーザー前提）
- 重複 feature_id 以外のきめ細かいバリデーション全網羅（既存 `Document::validate` / `FormatError` が拾う範囲のみに依拠。意味的妥当性の追加検証は対象外）
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
