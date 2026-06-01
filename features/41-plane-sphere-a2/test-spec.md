# Test Spec for #41 Plane×Sphere A2 (Cut)

All planned tests T01–T13 are implemented and passing in
`crates/mycad-build/tests/surface_boolean_a2_acceptance.rs`.

## 実装済みテスト (plan 計画分)

| ID | 関数名 | 状態 |
|----|--------|------|
| T01 | t01_determinism | ✓ |
| T02 | t02_volume_sphere_dimple | ✓ |
| T03 | t03_manifold_euler | ✓ |
| T04 | t04_intersection_edge_curve_circle | ✓ |
| T05 | t05_tessellation_plane_with_hole | ✓ |
| T06 | t06_tessellation_sphere_cap | ✓ |
| T07 | t07_degenerate_tangent_noop | ✓ |
| T08 | t08_degenerate_disjoint_noop | ✓ |
| T09 | t09_degenerate_great_circle_rejected | ✓ |
| T10 | t10_multi_plane_intersection_rejected | ✓ |
| T11 | t11_non_z_plane_unsupported | ✓ |
| T12 | t12_golden_yaml | ✓ |
| T13 | t13_example_export_smoke | ✓ |

## 実装差分から追加すべきテスト

- **earcut winding fix** (`tessellate_face_earcut`): annular face (plane with hole) の signed
  volume が負になるかどうかのテスト。T05 の面積検証でカバー済み。
- **sphere cap same_sense=false**: T06 の sphere cap tessellation 面積検証でカバー済み。
- **serde_json float_roundtrip**: T12 の JSON round-trip でカバー済み。
- **T15 partition test (kernel)**: cylinder+sphere geometry を修正 (R=3→R=1 at z=2) して
  multi-plane guard が誤発火しないことを確認済み。

## エッジケース・退化入力

全エッジケース (T07–T11) が実装済み。追加ケースなし。

## 数値境界

- T02: volume ≈ 970.68 ± 1.0 (分散 32 chords の離散化誤差)
- T05: plane with hole area ≈ 100 - 8π ≈ 74.87 ± 1.0
- T06: sphere cap area ≈ 2πRh = 12π ≈ 37.70 ± 2.0

## 決定性

T01 で全フィールド一致を確認済み。
