# ADR Context — #94 面ピッキング

## ADR-008 Decision 2: TriangleMesh.face_ids（ピッキングの前提）
- `TriangleMesh.face_ids: Vec<String>` は三角形 i が属する面の EntityRef 文字列表現。
- **長さ不変量**: `face_ids.len() == indices.len() / 3`（三角形ごと1個）。
- 生 index は露出しない（ADR-005 準拠）。無名面(`Face.name == None`)は空文字 `""`。
- ブラウザの raycaster は三角形インデックス(`faceIndex`)のみ返す → `face_ids[faceIndex]` で面参照を解決する設計が前提。

## ADR-008 Decision 3: transient / committed 境界
| 操作 | 分類 | 実装 |
|---|---|---|
| カメラ位置・ズーム・回転 | transient | クライアント内のみ。サーバに送らない |
| **面の選択・ホバーハイライト** | **transient** | **クライアント内のみ** |
| Extrude / ExtrudeCut 実行 | committed | POST /api/v0/features で .mycad に積む |
→ #94 の面選択は **transient**。サーバへ送らず `.mycad` を変更しない。

## ADR-005: トポロジカル・ネーミング（EntityRef）
- 生 Face index をフロントに渡すと Feature 再生成後に index が変わり参照が壊れる。
- `EntityRef`(安定命名)を使うことでトポロジー変化後も面参照が安定する。
- #94 では `face_ids`(= EntityRef 文字列) をそのまま選択キーに使う（生 index 非使用）。

## ADR-004: 数値方針
- #94 は新規 tolerance/ε を導入しない（face_id 解決は厳密整数インデックス）。本 ADR の直接対象外。
