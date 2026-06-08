## 不足テスト（plan 計画分）

| ID | 実装状況 | 備考 |
|----|----------|------|
| T01_normal_cap_z | ✅ GLM が extrude.test.ts に追加 | z方向法線 → "xy" |
| T02_normal_side_y | ✅ | y方向法線 → "xz" |
| T03_normal_side_x | ✅ | x方向法線 → "yz" |
| T04_degen_zero_cross | ✅ | 縮退三角形 → null |
| T05_boundary_no_match | ✅ | faceId 不一致 → null |

## 実装差分から追加すべきテスト

- **既存 planeForFaceId との協調**: planeForFaceId が解決できる face_id では planeForFaceNormal を呼ばないことを確認
- 該当テストは T17 に含まれる（main.ts のロジック）

## エッジケース・退化入力

- 縮退三角形（全頂点同一）→ null: 実装済み
- faceId が存在しない配列 → null: 実装済み

## 数値境界

- クロス積長 ≤ 1e-9 で縮退判定: 実装済み

## 決定性

- 同一入力→同一スケッチ平面: 純粋関数で保証
