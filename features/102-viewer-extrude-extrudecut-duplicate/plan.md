## In-Scope / Out-of-Scope

| In-Scope | Out-of-Scope |
|----------|--------------|
| GET /api/v0/features — 全 feature ID 一覧を返すエンドポイント実装 | feature の詳細（plane, profile など）を返すこと |
| web/src/api.ts に fetchAllFeatureIds() 追加 | feature の追加・削除操作 |
| web/src/main.ts の usedFeatureIds を全 feature ID で初期化 | ページリロード以外のシナリオ対策 |

## Non-Goals
- GET /api/v0/features で feature の詳細構造を返すこと（ID 一覧のみ）
- feature の削除・更新操作
- Issue #101 (AppState 非公開化) の対応

## 実装対象
- Issue: #102 (Closes #100 も)
- 影響ファイル:
  - `crates/mycad-api/src/handler.rs` — get_features ハンドラー追加
  - `crates/mycad-api/src/router.rs` — GET /features ルート登録
  - `web/src/api.ts` — fetchAllFeatureIds(): Promise<string[]> 追加
  - `web/src/main.ts` — 初期化時に fetchAllFeatureIds() で usedFeatureIds を構築

## 設計方針
- GET /api/v0/features は `{ "ids": ["box_1", "sketch_0", "extrude_0"] }` の形式で返す（または string[] 直接）
- AppState から `doc.root_component.features` を走査して全 feature.id() を返す
- `ensure_loaded()` 失敗時は 500 を返す（既存パターンと同一）
- 決定性要件: N/A（サーバー側管理）
- B-rep 妥当性: N/A（読み取り専用エンドポイント）
- エラーハンドリング: thiserror / ApiError を既存と同様に使用
- workspace.dependencies: 新規追加なし

## テスト計画（ID 付き）
| ID | 種別 | 内容 | 期待結果 |
|----|------|------|----------|
| T01_degen_empty | 境界 | features が空の .mycad でも GET /api/v0/features が成功する | 200 + [] |
| T02_normal_list | 正常系 | simple_box.mycad（box_1 + sketch_0 + extrude_0 を持つ）に対して GET /api/v0/features が全 ID を返す | 200 + ["box_1", "sketch_0", "extrude_0"] |
| T03_boundary_reload | シナリオ | usedFeatureIds に sketch_0 が含まれた状態で nextId("sketch_", ...) が sketch_1 を返す | "sketch_1" |

## 幾何的不変条件チェックリスト
- N/A
- N/A
- N/A
- N/A
