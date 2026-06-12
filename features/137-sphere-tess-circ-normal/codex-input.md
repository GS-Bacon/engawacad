===== TEST SUMMARY =====
{
  "totals": {
    "passed": 149,
    "failed": 1,
    "ignored": 6
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
- `Surface::Sphere` enum 構造の変更 (axis フィールド追加等)
- 他の sphere tessellation 関数 (full sphere、untrimmed) の変更
- `tessellate_sphere_face_trimmed` が扱う inner_loop 数の上限変更 (現状の 1 inner_loop 制約を維持)
- 上流の MVP 制約 (surface_intersect の軸 ±Z 限定) の解除
- ADR-009 案 A 実装本体 (交線エッジを周期円として保持する) — 本 Issue とは独立に Phase 4 再訪時に処理
- 共有境界比較テストの境界状態超強化 (#144 が別 Issue で扱う、本 Issue では「sphere の trim 境界のみ」をテスト)
===== END NON-GOALS =====

===== KNOWN IGNORED TESTS (理由付き #[ignore] — watertight 不可等の既知制約) =====
- t04_boundary_fuse_extrude_returns_single_body: boolean kernel cannot fuse CreateBox + extrusion (DisjointFuseResult); fuse_target code path is correct in mycad-build
- t01_determinism: fuzz: run with cargo xtask acceptance --fuzz
- t02_fuzz_no_http_500: fuzz: run with cargo xtask acceptance --fuzz
- t03_degen_special_float_values: fuzz: run with cargo xtask acceptance --fuzz
- t04_degen_unknown_type: fuzz: run with cargo xtask acceptance --fuzz
- t05_degen_null_body: fuzz: run with cargo xtask acceptance --fuzz
===== END KNOWN IGNORED TESTS =====

===== NOTE: total_added=0 について =====
テスト名が t01_/t02_/... 形式（test_ prefix なし）の場合 extract-test-summary の
集計に乗らないことがある。git diff で実際の追加テストを確認すること。
===== END NOTE =====
