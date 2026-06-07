# ADR 抜粋（#93 設計レビュー用コンテキスト）

## ADR-005 §F01 — 検証付き mutation の必須化

> **不変条件は mutation を通じても維持すること(F01)**: load path の封鎖だけでは不十分。
> `Document`/`Component` のフィールドを非公開にするか、全 mutation API (add_feature 等) に検証を
> 組み込む(validated builder/setter)。未検証状態を公開 API に決して露出しない。

> **未検証 Document をパブリック API から逃がさない**: 構築済みの `Document` は常に valid であることを
> 不変条件とする。

> **Component 内一意性を必須化(F01)**: `Feature.id` は所属する **Component の feature リスト内**で
> 一意でなければならない。重複・再利用は `FormatError` とする。

→ #93 への含意: POST で Feature を push する際、`candidate = doc.clone()` に push → `candidate.validate()`
（重複 feature_id 等は `FormatError`→422）。検証通過後のみ in-memory doc へ commit。未検証 Document を
レスポンスにもディスクにも露出させない。

## ADR-008 §Decision 3 — 書き込み API の契約

```
POST /api/v0/features    Feature を 1 個 .mycad に追加し、再テッセレーション結果を返す
```

> レスポンスは既存の `GET /api/v0/mesh` と同じ `Vec<BodyMesh>` 形式。
> フロントはレスポンスを受け取り、Three.js シーンを差し替える（WebSocket は Phase 7 以降）。

**transient / committed の境界**:
- カメラ位置・面選択・ホバー = transient（クライアント内のみ、サーバに送らない）
- `Extrude`/`ExtrudeCut` の実行 = committed（`POST /api/v0/features` で `.mycad` に積む）

**push/preview**: Phase 6 では WebSocket 非導入。同期 request/response モデル。プレビューは Phase 7+。

**ファイル永続化**:
> `POST /api/v0/features` は `.mycad` をメモリ上で更新した上でディスクに書き戻す。
> （`mycad view` は単一ファイルを開くシングルユーザーサーバのため、競合問題は発生しない）

→ #93 への含意: 同期 POST → メモリ更新 + ディスク書き戻し。WebSocket・プレビュー・undo/redo は Non-Goal。
レスポンス型は既存 `Vec<BodyMesh>` を再利用（新規 transport 型を作らない）。
