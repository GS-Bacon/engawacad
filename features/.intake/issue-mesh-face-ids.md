# feat(kernel): TriangleMesh に face_ids(EntityRef) を付与

## 概要

現在の `TriangleMesh` は `positions / normals / indices` のみで、三角形がどの B-rep 面に属するかの情報を持たない。
これはブラウザでの面ピッキング（三角形インデックス → 面の特定）が原理的に不可能な原因。
本 Issue で各三角形の面参照（`EntityRef` の文字列表現）を `face_ids: Vec<String>` として付与する。

## In-Scope / Out-of-Scope

| In-Scope | Out-of-Scope |
|---|---|
| `TriangleMesh` に `face_ids: Vec<String>` フィールド追加 | ブラウザ側のピッキング UI（#viewer-pick） |
| `tessellation/mod.rs` で各三角形に `face_idx` → `EntityRef` 文字列を充填 | 書き込み API（#write-api） |
| `BodyMesh` / `TS` 型の自動再生成（ts-rs 経由） | NURBS / 自由曲面の face 参照 |
| 不変量テスト: `face_ids.len() == triangle_count()` | カメラ・選択状態の永続化 |
| 既存テストのリグレッション確認（CI green） | |

## Non-Goals

- viewer-pick の実装
- 書き込み API
- face_ids を使った任意の集約・フィルタリング

## 完了条件

- `TriangleMesh::face_ids` が存在し、`len() == indices.len() / 3` の不変量がテストで保証される
- 各 `face_id` は `EntityRef::Named` の文字列表現で、再テッセレーション後も同一 face は同一 id を返す（決定性）
- `web/src/generated/TriangleMesh.ts` に `face_ids: Array<string>` が含まれる
- `cargo xtask ci` が通る

## 関連 ADR

- ADR-005: トポロジカル命名（EntityRef — 生 index を露出しない）
- ADR-008: Decision 2（face_ids の契約詳細）

## 実装ヒント

`crates/mycad-kernel/src/tessellation/mod.rs` の `face_idx: usize` を `EntityRef` 文字列に変換して
`TriangleMesh.face_ids` に push する。`EntityRef` の文字列シリアライズは既存の `serde` 実装を利用。
