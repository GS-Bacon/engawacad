===== TEST SUMMARY =====
{
  "totals": {
    "passed": 1168,
    "failed": 0,
    "ignored": 16
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
- `FeatureOp` enum 抽象の導入 — ADR-015 (#246) が `needs-human` のため確定後の別 Issue
- 子 Component (`root_component.children[*]`) への insert — Phase 9 最小スコープ外
- inline YAML 引数 — file path のみ
- Undo/Redo / history stack — Phase 9 別 Issue or Phase 10+
- proptest / criterion 導入 — ADR-015 §4 別系統 (Phase 9 入口)
- `schema_version` migration 連動 — 別 Issue (Phase 9 入口)
- **history-dependent semantic validation** (sketch/target/tool ref 存在チェック + body lifetime 検証) — #263 で別 Issue 化 (Codex 7.5 r1 M-F01 を scope-defend)。format-layer `validate()` は duplicate id 等のみ。semantic 違反は `build_assembly` 時に `SketchNotFound` / `BodyNotFound` で発覚する現挙動を受容する
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
