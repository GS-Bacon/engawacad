# test-spec.md — Issue #42 Plane×Cylinder Boolean A1.1 (Fuse + Intersect)

## 不足テスト（plan 計画分）

plan T01〜T14 はすべて `crates/mycad-build/tests/a1_1_plane_cyl_fuse_isect_acceptance.rs` に実装済みで PASS。
実際の修正: `tessellation/mod.rs` (Fix C: fan winding check, sphere trimmed winding fix), `booleans/assemble.rs` (short-arc fix), `booleans/partition.rs` (upper fragment fix)。不足なし。

## 実装差分から追加すべきテスト

### tessellate_face_fan_from_points の winding check (Fix C)

Fix C: 外境界ループの法線が face.same_sense と逆になっている場合に fan を反転する処理を追加。

追加すべきテスト:
- **fan_winding_cw_box_face**: 直方体の側面（CW ループ）が fan tessellation 後に正しく outward 法線を持つことを確認
- **fan_winding_ccw_face**: CCW ループの face が flip されないことを確認（非退化パス）

### tessellate_sphere_face_trimmed の same_sense 反転

same_sense=false の trimmed sphere 面で winding が正しく反転されることを確認:
- **sphere_trimmed_same_sense_false_outward**: same_sense=false の sphere 面が outward solid normals を持つことを体積で確認（A2 t02 が既にカバー）
- **sphere_trimmed_volume_sign**: 体積計算が正 (abs なし) であることを確認

### assemble.rs の circle_curve_for_edge short-arc 修正

- **short_arc_preference**: 2π より小さい arc が選択されること（接線方向の一致）

### partition.rs の upper fragment

- **upper_fragment_in_fuse**: Fuse 結果の cylinder lateral 上部 (z=5..15) が solid に含まれること（t03 Euler check でカバー済み）

## エッジケース・退化入力

実装差分から見えた分岐:

| ケース | 内容 | 現状 |
|--------|------|------|
| disjoint fuse | 交差なし | t08 でカバー（overlapping で success を確認） |
| empty intersect | 交差なし | t09 でカバー |
| multi-plane | box 両側を円柱が貫通 | t10 でカバー（シングルプレーンのみ success） |
| non-Z plane | XY 以外平面との交差 | t11 でカバー（成功確認） |

## 数値境界

- 体積 tolerance: Fuse ±5.0、Intersect ±2.0 (chord 32 の近似誤差ベース)
- Euler check: V-E+F-L_inner = 2

## 決定性

t01 (Fuse determinism) でカバー済み。Intersect の決定性は t01 の fuse 版と同様に検証可能だが、t14 YAML roundtrip で間接的にカバー。

## GLM へのタスク

以下は STEP 6.6 で GLM が追加する:
1. `fan_winding_cw_box_face` — tessellate_solid の結果から box side face の法線を確認
2. `fan_winding_ccw_face` — cuboid の tessellation が positive volume になることを確認（既存 primitives test で代替可）
3. `sphere_trimmed_volume_sign` — raw signed volume (abs なし) が正であることを確認

Note: GLM は inline test (`#[cfg(test)]`) として `tessellation/mod.rs` か `booleans/` に追加するのが適切。acceptance test への追加は不要（すでに 14 テスト存在する）。
