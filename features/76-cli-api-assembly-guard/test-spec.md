# Test Spec — Issue #76: CLI/API アセンブリ拒否ガード削除

## 不足テスト（plan 計画分）
T01〜T05 はすべて `#[ignore]` スケルトンとして作成済みで、GLM 実装時に解除・実装済み。

T01 決定性: CLI export 2回で同一 STL バイト列 — **実装済み**
T02 正常系(CLI): assembly.mycad export → exit 0 + STL 非空 — **実装済み**
T03 正常系(API): assembly.mycad API → HTTP 200 + bodies 非空 — **実装済み**
T04_boundary_empty_assembly: empty doc → HTTP 422 — **実装済み**
T05_boundary_simple_part: features のみも引き続き動作 — **実装済み**

## 実装差分から追加すべきテスト
- 既存の `t09_assembly_unsupported`（mesh_api.rs）が `t09_assembly_supported` 系に更新される必要あり（GLM が実施済み）
- `t16_assembly_unsupported`（export.rs）も同様に更新

## エッジケース・退化入力
- T04_boundary_empty_assembly: features/children/reference が全部ない Document を API に渡す → 422 確認

## 数値境界
- T01: STL バイト列完全一致（文字列比較）
- T02/T03: 非空確認のみ（具体的な三角形数は不問）

## 決定性
T01 で CLI export の完全一致を確認。IdGenerator::new(0) により保証。
