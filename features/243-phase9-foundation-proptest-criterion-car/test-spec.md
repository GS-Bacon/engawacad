# test-spec for #243 (light flow)

## 不足テスト (plan 計画分)

GLM core 実装で proptest T01 + T_BOUNDARY + T_DEG の 3 件をすでに `tests/cuboid_determinism_proptest.rs` に実装済 (user 確認済)。Playwright smoke は `web/tests/smoke.spec.ts` 生成済。

## 実装差分から追加すべきテスト

light flow の最小 setup として現状で十分。追加テスト不要。

## エッジケース

`T_BOUNDARY_proptest_minimal_dim` (0.01) と `T_DEG_zero_dim_excluded` (0.0 reject) でカバー済。

## 数値境界

proptest 範囲 0.01..1000.0 で IEEE f64 演算の正常域をカバー。境界下限 (0.01) は inline test で確認済。

## 決定性

T01 proptest が 256 cases で `IdGenerator::new(seed=42)` を fix して 2 回呼び `format!("{:?}", solid)` 一致確認。

## 期待値乖離

なし。GLM core 実装は plan のテスト計画と一致。

## 結論

追加 test 不要。STEP 6.6 では既存テストを再確認するのみで CI green 維持。
