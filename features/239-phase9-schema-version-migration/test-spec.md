# Test Spec — #239 schema_version + MigrationHook entry

## 概要

GLM core (STEP 6) は **inline `mod tests` (document.rs / migration.rs)** に T01 / T_DEG_unknown_version / T_BOUNDARY_current_version / T_DEG_max_u32 / T_TRAIT_migration_hook_signature を実装済み (CI green)。

ただし `crates/engawa-format/tests/schema_version_migration_acceptance.rs` (integration test skeleton) は `todo!()` + `#[ignore]` のまま残っている。

## 不足テスト (plan 計画分)

| ID | 場所 | 現状 |
|----|------|------|
| T01 | inline (document.rs) | 実装済み (`t01_deterministic_from_yaml`) |
| T_DEG_unknown_version | inline (document.rs) | 実装済み (`t_deg_unknown_version_rejected`) |
| T_BOUNDARY_current_version | inline (document.rs) | 実装済み (`t_boundary_current_version_accepted`) |
| T_DEG_max_u32 | inline (document.rs) | 実装済み (`t_deg_max_u32_rejected`) |
| T_TRAIT_migration_hook_signature | inline (migration.rs) | 実装済み |

**T02 / T03 は既存 `test_schema_version_backward_compat` (document.rs) で継続カバー。**

## 実装差分から追加すべきテスト

- **acceptance file (`tests/schema_version_migration_acceptance.rs`) の skeleton を実装に置換** (公開 API 経由の最小 integration coverage):
  - `t01_determinism_from_yaml` — public API `Document::from_yaml` を 2 回呼んで同一結果
  - `t_deg_unknown_version_99` — `Document::from_yaml` が `FormatError::UnknownSchemaVersion { found: 99, current: 1 }` を返す
  - `t_boundary_current_version` — `schema_version: 1` で正常 parse
  - `t_deg_max_u32_version` — `schema_version: u32::MAX` で reject
  - `t_trait_migration_hook_signature` — `engawa_format::MigrationHook` trait のダミー実装が compile + `migrate()` が `Ok` を返す
- 各テストの `#[ignore]` を外し、`todo!()` を実 assertion で置換すること

## エッジケース・退化入力

- (既に inline でカバー済み): version 0 受容、u32::MAX reject、未知 version reject、direct deserialize 経路の reject

## 数値境界

N/A (整数比較のみ)

## 決定性

T01 で 2 回 parse 結果が一致することを inline + acceptance 両方でカバー (二重カバーは害なし、integration boundary の信頼性を上げる)

## 類似ケース (未カバー)

- N/A — バグ修正 Issue ではないため (新機能追加)

## 期待値乖離

なし (plan の T ID 期待値と実装の assertion は一致)
