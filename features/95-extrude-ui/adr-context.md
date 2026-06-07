# ADR-008 抜粋（#95 関連）

## Decision 1: 押出 / 押出カットを別 Feature variant とする
- `Feature::Extrude { id, sketch, depth }`（加算）。本体は `make_extrusion` の再利用で実現。
- **新しい幾何実装は不要**。既存の `Feature::Extrude` を再利用すること（API/Feature モデルを拡張しない）。

## Decision 3: 書き込み API の契約
- `POST /api/v0/features`: Feature を **1 個** .mycad に追加し、再テッセレーション結果 `Vec<BodyMesh>` を返す。
- レスポンスは `GET /api/v0/mesh` と同形式。フロントは受け取って Three.js シーンを差し替える。
- transient（クライアント内のみ）: カメラ操作・面選択/ホバー。committed（POST で .mycad に積む）: Extrude / ExtrudeCut。
- Phase 6 は WebSocket 非導入。同期 request/response。POST は .mycad をメモリ更新の上ディスク書き戻し。

## 構造的含意（#95）
- Extrude.sketch は CreateSketch の id 文字列参照。POST は 1 Feature/回。
  → 選択面押出は `create_sketch`(plane+profile) → `extrude` の 2 連続 POST が唯一の経路。
- SketchPlane は xy/xz/yz、Plane::xy/xz/yz は原点固定（オフセット押出不可＝Phase 7）。
