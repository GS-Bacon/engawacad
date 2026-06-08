## 不足テスト（plan 計画分）

| ID | 実装状況 |
|----|----------|
| T01_unit_offset_yz | ❌ 未追加 — faceOffsetFromPlane() を web/src/extrude.test.ts に追加 |
| T02_unit_offset_xy | ❌ 未追加 |
| T03_degen_no_match | ❌ 未追加 |
| T04_boundary_fuse | ❌ skeleton のみ (#[ignore]) — extrude_fuse_acceptance.rs を実装 |

## 実装差分から追加すべきテスト

- T05_offset_zero: offset=0.0 のとき base_plane をそのまま使う（translate されない）
- T06_fuse_target_none: fuse_target=None のとき新ボディが独立して追加される（既存挙動との後方互換）
- T07_fuse_target_not_found: fuse_target が存在しないボディ ID を指す場合 KernelError::BodyNotFound

## エッジケース・退化入力

- faceId が存在しない → faceOffsetFromPlane が 0.0 を返す（T03）
- offset が負の値（未対応 — Out-of-Scope、テスト不要）

## 決定性

- faceOffsetFromPlane(): 最初に一致する三角形の代表頂点座標を使用 → 同一入力は常に同一出力
- buildExtrudeFeatures() に targetBody が追加されたが nextId() は deterministic

## テスト実装優先順位

1. T01〜T03: TypeScript 単体テスト（web/src/extrude.test.ts に追記、GLM が担当）
2. T04: Rust API acceptance テスト（crates/mycad-api/tests/extrude_fuse_acceptance.rs の skeleton 実装、GLM が担当）
3. T05〜T07: optional（GLM が時間内に追加できれば）
