# ADR 抜粋（#92 設計レビュー用）

## ADR-008 Decision 2: TriangleMesh に面の安定参照を焼き込む

`TriangleMesh` に `face_ids: Vec<String>` を追加し、各三角形が属する面の `EntityRef` 文字列表現を
triangle ごとに記録する。生 index は露出しない（ADR-005 準拠）。

```rust
pub struct TriangleMesh {
    pub positions: Vec<[f64; 3]>,
    pub normals:   Vec<[f64; 3]>,
    pub indices:   Vec<u32>,
    /// 三角形 i が属する面の安定参照 (EntityRef の文字列表現).
    /// 長さは indices.len() / 3 と一致する.
    pub face_ids:  Vec<String>,
}
```

- ピッキングの前提: raycaster が返すのは三角形 index のみ。三角形→面の対応が無いと面選択不可。
- ADR-005 準拠: 生 Face index を返すと Feature 再生成後に index が変わり参照が壊れる。
- 採用しなかった案: `face_index: Vec<u32>`（生 index）は ADR-005 を破るため不採用。
- 影響範囲: `tessellation/mod.rs` で face_ids 充填、`body_mesh.rs` は変更なし（透過）、`TriangleMesh.ts` は自動再生成。

## ADR-005 Decision 6: 基底名の canonical grammar と表現の分離（F02）

- 内部 canonical name grammar: `<feature_id>;<kind>:<role>`（ハッシュ seed・同一性判定に使う）。
- **F02: この grammar 文字列は `.mycad` 上の wire format ではない。** `.mycad` / serde / TS / JsonSchema の
  on-disk format は構造化形式 `{ feature_id, kind, role }` を使う。canonical name 文字列はビルド時に
  構造化フィールドから導出する。
- 文字集合: 各セグメント `[A-Za-z0-9_-]`、区切り `;` `:` は予約。

## ADR-005 Decision 10: 退化エンティティの命名方針

- 命名は topology validation 後の**非退化**エンティティにのみ付与。退化入力は `KernelError` とし、
  名前集合に現れさせない。

## 本 Issue の文字列表現選択（plan.md 設計方針より）

- `face_ids` の各要素 = `EntityRef::canonical_name()`（例 `N(box_1;F:top)`）。`Face.name == None` は `""`。
- 根拠: `face_ids` は `.mycad` に永続化されない `TriangleMesh` 派生メタデータであり、F02 が制約する
  EntityRef 永続化経路（wire/on-disk）には当たらない。フロントは face_id を不透明トークンとして扱う。
- 逆引き（face_id → face）は #93 の責務。本 Issue 対象外。
