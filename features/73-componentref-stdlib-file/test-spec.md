# Test Spec — Issue #73: ComponentRef 参照解決 (stdlib + file)

## 不足テスト（plan 計画分）
plan T01-T08 はすべて GLM が実装済み（#[ignore] 除去済み）。

## 実装差分から追加すべきテスト
なし — 追加の分岐（features 空の純参照コンポーネント、children のみ）は T03/T04 のバリエーション
で カバーされており、新規テスト ID は不要と判断。

## エッジケース・退化入力
- T06_boundary_self_reference: A→A を CircularReference として検出 (実装済み)
- T07_boundary_depth_16_vs_17: 16段 Ok / 17段 Err の境界 (実装済み)
- T08: stdlib_root 未設定時の ReferenceResolution (実装済み。reason に "not found" を含む)

## 数値境界
N/A（本 Issue は幾何演算を含まない）

## 決定性
T01: 同一 Document を 2 回ビルドし、feature_id 列・vertex/edge/face 数・座標を全一致で確認。
(実装済み)
