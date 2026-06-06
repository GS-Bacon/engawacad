# test-spec.md — Issue #58

## 不足テスト（plan 計画分）

| ID | 関数名 | 実装状況 |
|----|--------|---------|
| T01_type_check | vite build / TypeScript type-check | ✅ CI green で確認済み |
| T02_boundary_degen_empty | bodies=[] での crash しないこと | ✅ CI green で確認済み（vite build が通る） |

## 実装差分から追加すべきテスト

- `AxesHelper` の追加は TypeScript 型チェックと vite build で担保済み
- 視覚的確認（軸が実際に表示されているか）は Issue #61 の Playwright テストで実施予定
- Vitest 単体テストは viewer.ts が DOM/WebGL に依存するため追加困難

## エッジケース・退化入力
- bodies=[] 時: `axisSize = 10`（固定値）→ AxesHelper(10) が追加される（CI で確認済み）
- bodies が非空の時: axisSize = Math.max(maxDim * 0.5, 1) → 最小 1 を保証

## 追加すべきエッジケーステスト
なし（ビューア変更は DOM/WebGL 依存で Vitest 単体テスト困難）。
