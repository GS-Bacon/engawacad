===== TEST SUMMARY =====
{
  "totals": {
    "passed": 1149,
    "failed": 0,
    "ignored": 16
  },
  "by_crate": {},
  "added_in_round": [
    {
      "name": "t01_deterministic_evaluation",
      "kind": "determinism"
    },
    {
      "name": "t02_document_only_evaluation",
      "kind": "other"
    },
    {
      "name": "t03_shadowing_document_variable",
      "kind": "other"
    },
    {
      "name": "t05_arithmetic_precedence",
      "kind": "other"
    },
    {
      "name": "t06_parentheses",
      "kind": "other"
    },
    {
      "name": "t07_unary_minus",
      "kind": "boundary"
    },
    {
      "name": "t08_division",
      "kind": "other"
    },
    {
      "name": "t01_determinism",
      "kind": "determinism"
    },
    {
      "name": "t02_document_only",
      "kind": "other"
    },
    {
      "name": "t03_sketch_shadowing",
      "kind": "other"
    },
    {
      "name": "t05_arithmetic",
      "kind": "other"
    },
    {
      "name": "t06_parens",
      "kind": "other"
    },
    {
      "name": "t07_unary_minus",
      "kind": "boundary"
    },
    {
      "name": "t08_division",
      "kind": "other"
    },
    {
      "name": "t09_yaml_roundtrip",
      "kind": "golden"
    },
    {
      "name": "t10_empty_vars_omitted",
      "kind": "degenerate"
    },
    {
      "name": "t11_doc_var_shadowed_not_in_result",
      "kind": "other"
    },
    {
      "name": "t12_shadowing_evaluation_order_stability",
      "kind": "other"
    }
  ],
  "coverage_hints": {
    "total_added": 18,
    "determinism": 2,
    "degenerate": 1,
    "boundary": 2,
    "golden": 1,
    "edge_case": 0
  }
}
===== END TEST SUMMARY =====

===== NON-GOALS (SCOPE OUT — Codex はこれらを指摘しないこと) =====
- string literal 内 `${var}` interpolation (ADR-015 Q2 を別 ADR に倒す)
- 関数呼び出し / 数学関数 (`sin` / `cos` / `sqrt` / `pow` 等)
- Feature の数値 field 内での `${var}` 参照解決 (parser は Variable.expr 内のみで動作)
- 数値以外の値型 (string / boolean / vector)
- Component 階層を越える variable 解決 (children component の variable は parent 不可視)
- Variable persistence schema migration (#239 で `schema_version` 入口は実装済 — 本 Issue は v1 内拡張で互換)
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
