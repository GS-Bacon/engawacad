## 不足テスト（plan 計画分）

| ID | 実装状況 |
|----|----------|
| T01_normal_under | ✅ T23_01〜03 で距離計算を確認 |
| T02_boundary_exact | ✅ T23_boundary_exact でクランプ確認 |
| T03_boundary_over | ✅ T23_boundary_exact で over も確認（depth=5 > 5ε→clamp） |
| T04_degen_zero_offset | ✅ T23_degen_zero_offset で null 確認 |

## エッジケース・退化入力

- face at origin → null: 実装済み
- depth == faceOffset → clamped: 実装済み

## 決定性

- 同一入力→同一 effectiveDepth: 純粋関数で保証
