# Test Spec for #115: api-e2e-assertion-strengthen

## 実装済みテスト（plan 計画分）

| ID | 場所 | 実装内容 | 状態 |
|----|------|----------|------|
| T01 | extrude_cut_acceptance.rs::a01 | unique face_id 数 == 12 | ✅ GLM 実装済 |
| T02 | extrude_cut_acceptance.rs::a01 | mesh_volume(after) > mesh_volume(before) | ✅ GLM 実装済 |
| T03 (S04) | e2e_api_scenarios.rs::s04 | offset=5.0, depth=5.0 → 422 | 既存 pass |
| S04b | e2e_api_scenarios.rs::s04b | offset=5.0, depth=4.9 → 200 | ✅ GLM 追加 |

## 実装差分から追加すべきテスト

なし（face_id 一意性は T01 の unique_faces.len() == 12 で網羅済み）

## エッジケース・退化入力

- 面数: GLM は `6 + 4 + 2 = 12` (外殻6面 + 4側面 + 2キャップ) と判定 → CI で pass 確認済み
- 体積: 内殻 mesh は winding 逆 → per-body abs() で正しく計算

## 数値境界

- depth=4.9 < offset=5.0: S04b で検証済み
- depth=5.0 == offset: S04 で 422 検証済み

## 決定性

既存 S05 が multi-step 決定性を検証済み。追加不要。
