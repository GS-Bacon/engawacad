# #217 test-spec.md

## 不足テスト (plan 計画分)

すべて plan T01〜T05_degen_zero_depth が STEP 6 で実装済み (`crates/engawa-build/tests/face_sketch_extrude_acceptance.rs`)。残不足なし。

## 実装差分から追加すべきテスト

GLM 実装差分: acceptance file (5 関数) + examples_smoke.rs エントリ 1 件 + 新規 example 1 ファイル。crates/ src/ 変更ゼロ (既存実装の pin 専念)。

| ID | 内容 | 期待結果 |
|----|------|---------|
| T06_phase3_extruded_rect_non_regression | Phase 3 の `examples/extruded_rect.engawa` を本 acceptance test 内でも明示的に build → Ok を assert (Phase 3 path 維持の docstring 兼 regression test) | `build_assembly(&doc, ..).is_ok()` |
| T07_pillar_top_face_resolved_via_entity_ref | T02 と同じ build を行い、extrude_1 の最上面 (z=4.5 ± 1e-9) に少なくとも 8 頂点が存在することを assert (8 角形の cap が正しく形成された) | `let top_count = solid.vertices.iter().filter(\|v\| (v.point.z - 4.5).abs() < 1e-9).count(); assert!(top_count >= 8);` |

## エッジケース・退化入力

| ID | 内容 | 期待結果 |
|----|------|---------|
| T08_negative_depth_graceful | inline YAML で extrude depth = -1.0 → `Err(KernelError::InvalidParameter { kind: "depth" })` で graceful error (panic でない) を matches! で pin | `matches!(err, KernelError::InvalidParameter { kind } if *kind == "depth")` |
| T09_face_role_unknown_graceful | inline YAML で `role: "f_xxx"` (存在しない role) を指定 → `Err(KernelError::FaceEntityRefNotFound { .. })` で graceful error | `matches!(err, KernelError::FaceEntityRefNotFound { .. })` |

## 数値境界

| ID | 内容 | 期待結果 |
|----|------|---------|
| T10_very_small_depth | extrude depth = 1e-6 (極小値、`LENGTH_TOLERANCE` 直上) → build 成功し z range が [2.5, 2.500001] に乗る | `build OK && (z_max - 2.500001).abs() < 1e-9` |

## 決定性

| ID | 内容 | 期待結果 |
|----|------|---------|
| T11_two_run_solid_vertex_bytes_match | T02 と同じ build を 2 回行い、extrude_1 の頂点座標 (point.x, y, z) を bit-for-bit 比較 → 完全一致 | `assert_eq!(verts_run1, verts_run2)` (Vec<[f64;3]>) |

## 類似ケース (Bug 修正 Issue のみ — 本 Issue は feature のため N/A)

N/A — 本 Issue は feature 追加。

## 期待値乖離

`check-spec-divergence.ts` の結果: 「git diff main..HEAD で変更された .rs ファイルなし」(本ブランチは cad/217-phase8-modelface-sketch-extrude で `main` 比較が空、また cad ブランチ上で未 commit のため diff 検出ゼロ)。

手動 diff (`git status -s`) で確認した implementation:
- T01: 3-run STL byte eq → 一致 ✓
- T02: bodies.len() == 2, z range [2.5, 4.5], euler_poincare() == 0 → 一致 ✓
- T03: all v.point.z >= 2.5 - 1e-9 → 一致 ✓
- T04: resize 20x20x8, z range [4.0, 6.0] → 一致 ✓
- T05_degen: matches! InvalidParameter kind=depth → 一致 ✓

**乖離なし**。STEP 6.6 へ進む。
