# Test Spec — #106 E2E API Scenarios

## 実装済みテスト（S01-S05 全件 pass）

| ID | ファイル | ステータス | 内容 |
|----|---------|-----------|------|
| S01_reload_no_duplicate_id | e2e_api_scenarios.rs | ✅ pass | GET /features → sketch_1/extrude_1 確認; 同 ID POST → 422 + "duplicate" |
| S02_extruded_face_ids_contain_cap | e2e_api_scenarios.rs | ✅ pass | POST sketch+extrude → GET /mesh → f_cap_* or f_side_* 含む |
| S03_extrude_creates_two_bodies | e2e_api_scenarios.rs | ✅ pass | POST sketch+extrude(no fuse) → 2 bodies (box_1, extrude_0) |
| S04_degen_extrudecut_depth_boundary | e2e_api_scenarios.rs | ✅ pass | sketch(offset=5)+extrude_cut(depth=5) → 422 |
| S05_determinism_multi_step | e2e_api_scenarios.rs | ✅ pass | 同一入力シーケンス×2 → 完全同一出力 |

## 不足テスト（plan 計画分）

なし — 全 S ID が実装済みかつ pass。

## 実装差分から追加すべきテスト

- なし（S01-S05 がすべての #102-#105 シナリオをカバー）

## エッジケース・退化入力

- S04 が境界値テスト（depth == face distance → 422）を担当
- S01 が duplicate ID 検出を担当

## 決定性

- S05 が multi-step 決定性を担当（同一出力確認）

## 数値境界

- S04: offset=5.0, depth=5.0 が境界値（coplanar → 422）
- S02: 10×10 プロファイル、depth=5.0

## 既知 `#[ignore]` テスト

なし — 全テストが有効。

## GLM テスト実装フェーズ

全テストが GLM コア実装フェーズで既に実装・pass 済みのため、
テスト追加フェーズ（STEP 6.6）はスキップ可能。
