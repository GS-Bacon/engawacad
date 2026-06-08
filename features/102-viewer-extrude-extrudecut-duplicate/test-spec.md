## 不足テスト（plan 計画分）

| ID | 実装状況 | 備考 |
|----|----------|------|
| T01_degen_empty | ✅ `t01_degen_empty_features` | empty_part.mycad fixture を使用 |
| T02_normal_list | ✅ `t02_normal_list_features` | extruded_rect.mycad (sketch_1, extrude_1) |
| T03_boundary_reload | 部分実装 | `t03_features_determinism` として決定性テスト化。「ページリロード後シナリオ」は E2E が必要 |

## 実装差分から追加すべきテスト

- **fetchAllFeatureIds エラーハンドリング**: GET /api/v0/features が 500 を返したとき、fetchAllFeatureIds() が throw するかテスト（web 側、Vitest）
- **usedFeatureIds の完全性**: main.ts 初期化後、`nextId("sketch_", usedFeatureIds)` が既存 sketch_0 を避けて sketch_1 を返すかテスト（web 側、Vitest）

## エッジケース・退化入力

- features が空 (T01) → 実装済み
- features に重複 ID がある場合 → サーバー側で防止しているため N/A

## 数値境界

- 該当なし（ID 文字列のみ）

## 決定性

- 同一ファイルへの2回の GET リクエストが同一 ID 一覧を返す (T03) → 実装済み
