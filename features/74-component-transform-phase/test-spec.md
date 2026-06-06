# Test Spec — Issue #74: Component ツリー走査 + 平行移動 transform 合成

## 不足テスト（plan 計画分）
T01-T07 はすべて GLM コア実装時に #[ignore] なしで実装済み。

## 実装差分から追加すべきテスト
なし — `build_component_tree` の変更は plan の設計通り（accumulated_offset 追加のみ）。
追加の分岐（rotation 非ゼロの無視）はテスト不要（無視が設計意図）。

## エッジケース・退化入力
- T06_boundary_zero_offset_unchanged: 明示ゼロで変化なし (実装済み)
- T07_boundary_large_offset_stays_finite: 1e6 の大 offset でも有限 (実装済み)

## 数値境界
T02/T03: 座標差の絶対値 < 1e-10 で検証（float exact ではなく epsilon）

## 決定性
T01: 同一 Document を 2 回ビルドし、feature_id + 全 vertex 座標を一致確認。(実装済み)
