# test-spec.md — Issue #72: 幾何型の平行移動 transform

## 実装状況サマリ

GLM コア実装は `ci_passed: true` で完了。
T01〜T07 は `crates/mycad-kernel/src/brep/topology.rs` の inline tests として実装済み。
`crates/mycad-kernel/tests/translate_acceptance.rs` はスケルトン状態（`#[ignore] + todo!()`）のまま。

## 不足テスト（plan 計画分）

plan T01〜T07 はすべて inline tests で実装済みだが、acceptance test が未実装。

| ID | 実装場所 | 状態 |
|----|----------|------|
| T01 決定性 | topology.rs inline | ✅ 実装済み |
| T02 基準点移動 | topology.rs inline | ✅ 実装済み |
| T03 軸/半径不変 | topology.rs inline | ✅ 実装済み |
| T04 EntityID 不変 | topology.rs inline | ✅ 実装済み |
| T05 逆変換 | topology.rs inline | ✅ 実装済み |
| T06_boundary_zero_offset | topology.rs inline | ✅ 実装済み |
| T07_degen_large_offset | topology.rs inline | ✅ 実装済み |
| translate_acceptance.rs T01〜T07 | acceptance test | ❌ todo!() のまま |

**STEP 6.6 での対応が必要**: `translate_acceptance.rs` の各テストを inline テストの実装を流用して実装し、`#[ignore]` を外すこと。

## 実装差分から追加すべきテスト

1. **`transform::translate_point` の基本テスト**: `geometry/transform.rs` に inline tests が追加されている（`test_translate_point_identity` / `test_translate_point_basic`）。これは plan に明示的になかったが適切な追加。

2. **`Surface::PartialEq` 追加の副作用確認**: GLM が `Surface` に `PartialEq` を derive 追加した。既存テストで `Surface` の `eq` 比較が意図せず通るようになった箇所がないか確認不要（新規追加のみ、削除なし）。

3. **`Curve::PartialEq` 追加**: 同様に `Curve` にも `PartialEq` が derive 追加された。T06 の `assert_eq!(o.curve, m.curve)` のために必要な変更で正当。

## エッジケース・退化入力

- T06（ゼロ offset）: 実装済み
- T07（巨大 offset 1e9）: 実装済み
- NaN/Inf offset: plan 方針として「propagate する、検証は呼び出し側責務」→ テスト不要

## 数値境界

- 逆変換精度 1e-12: T05 で `assert_relative_eq!(epsilon = 1e-12)` として実装済み

## 決定性

- T01 で 100 回シリアライズ一致を確認済み

## acceptance test 実装指示（STEP 6.6 向け）

`crates/mycad-kernel/tests/translate_acceptance.rs` の各 `#[ignore] + todo!()` を、
`topology.rs` の inline tests と同等の内容で実装して `#[ignore]` を外すこと。

```
t01_determinism_solid_translate_100_runs  → t01_translate_determinism と同等
t02_translate_moves_origin_points         → t02_geometry_translate と同等
t03_translate_axes_radii_invariant        → t03_translate_invariants と同等
t04_entity_id_invariant_after_translate   → t04_entity_id_preserved と同等
t05_translate_inverse_roundtrip           → t05_translate_roundtrip と同等
t06_boundary_zero_offset_no_change        → t06_boundary_zero_offset と同等
t07_degen_large_offset_stays_finite       → t07_large_offset_remains_finite と同等
```

