# feat(api): POST /api/v0/features で Feature 追加 + 再テッセレーション

## 概要

現在の `mycad-api` は `GET /api/v0/mesh` 1 本のみで、モデルへの書き込み操作が一切できない。
本 Issue で `POST /api/v0/features` を追加し、Feature を `.mycad` に積んで即再テッセレーション結果を返す
書き込みエンドポイントを実装する。これにより UI→サーバ→モデル更新→再描画のループが初めて通る。

前提: mesh-face-ids (#N) が closed であること（`face_ids` なしで API を作ると後で壊れる）。

## In-Scope / Out-of-Scope

| In-Scope | Out-of-Scope |
|---|---|
| `POST /api/v0/features` エンドポイント追加 | WebSocket / リアルタイムプッシュ（ADR-008 Decision 3 で Phase 7 以降と決定） |
| リクエスト body: `Feature` JSON（既存の ts-rs 型） | Feature の削除・並べ替え（undo/redo） |
| `.mycad` をメモリ更新 + ディスク書き戻し | 認証・マルチユーザー |
| レスポンス: 再テッセレーション `Vec<BodyMesh>`（既存型を再利用） | バリデーション（重複 ID 等）の全網羅 |
| `cargo xtask ci` + 統合テスト（POST → mesh が変わる） | |

## Non-Goals

- WebSocket
- Feature 削除・編集エンドポイント
- 認証・マルチユーザー対応
- プレビュー（未確定 Feature の一時テッセレーション）

## 完了条件

- `POST /api/v0/features` に `Feature` JSON を送ると `.mycad` が更新され `Vec<BodyMesh>` が返る
- 連続 POST でモデルが積み上がる（Feature 履歴が累積される）
- `GET /api/v0/mesh` の結果と `POST` 直後のレスポンスが一致する（冪等性）
- `cargo xtask ci` が通る

## 関連 ADR

- ADR-003: Feature コマンド一本化・変更モデル
- ADR-008: Decision 3（書き込み API 契約、transient/committed 境界）

## 実装ヒント

`crates/mycad-api/src/router.rs` に POST ルートを追加。
`handler.rs` で axum の `Json<Feature>` を受け取り、インメモリの `Document` に push → `build_assembly` →
`tessellate_solid_with` → レスポンス。ファイル書き戻しは `serde_yaml::to_string` + `fs::write`。
サーバ状態（開いているファイルパス + Document）は `Arc<Mutex<AppState>>` で保持。
