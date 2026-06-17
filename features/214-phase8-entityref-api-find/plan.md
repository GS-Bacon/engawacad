# Plan — #214 EntityRef 逆引き API find_face_by_entity_ref (Phase 8 sub-Issue 8a)

## 自律判断ログ (B-3)

自律バッチモード (`batch_arg === null`) で起動。pause_reasons なし・ambiguous false のため Claude 単独で要件確定。曖昧点と判断:

- **戻り値型**: Issue 本文は `Option<FaceIndex>` と表記しているが、`FaceIndex` 型は kernel に未定義 (`grep -rn "type FaceIndex" crates/` で 0 件)。`crates/engawa-kernel/CLAUDE.md` の「Index-based topology — usize で参照」「`Solid::add_face` が `usize` を返す」既存慣行に合わせ `Option<usize>` を採用する。新規 type alias の導入は overengineering で本 Issue のスコープ外 (Out-of-Scope に明記)。
- **Edge/Vertex 逆引きの抽象化**: Issue 本文「trait or 共通ヘルパ」と書かれているが、Phase 9 以降で初めて Edge/Vertex 用が必要になる。今 trait を切ると YAGNI (未使用 trait の維持コスト)。代わりに「将来 `find_<entity>_by_entity_ref` を兄弟メソッドとして並べる」構造で十分。trait は 8 後続 Issue または Phase 9 で再評価する (Out-of-Scope)。
- **マッチ規約**: ADR-005 の `EntityRef::Named { feature_id, kind, role }` 3 つ組が同一なら同一 Face とみなす。kind が `Face` 以外 (`Edge`/`Vertex`) を渡された場合は `None` を返す (`Named.kind != Face` を弾く)。`Derived` variant は Phase 9 以降扱いなので `None` 返却。
- **重複検出**: 同 `EntityRef` を持つ複数 Face は ADR-005 違反だが現状検出機構がない。本 API は **最初に見つかった Face index** を返す (deterministic: vector は決定的順序、線形 scan も決定的)。重複検出は将来 invariant check で対応 (Out-of-Scope)。

## In-Scope / Out-of-Scope

| In-Scope | Out-of-Scope |
|----------|--------------|
| `Solid::find_face_by_entity_ref(&EntityRef) -> Option<usize>` を `crates/engawa-kernel/src/brep/topology.rs` に実装 | `Solid::find_edge_by_entity_ref` / `find_vertex_by_entity_ref` の実装 (sub-Issue 8b 以降または Phase 9) |
| `EntityRef::Named { feature_id, kind, role }` 3 つ組による Face 一意解決 | `EntityRef::Derived` の解決ロジック (Phase 9 以降) |
| `kind != Face` のとき `None` を返す guard | `FaceIndex` type alias の新規導入 (既存 `usize` で十分) |
| 線形 scan 実装 (faces は通常 < 100 件、O(n) で十分) | パフォーマンス最適化 (HashMap キャッシュ等は ADR-008 が独自に検討) |
| 単体テスト 4 ケース (cuboid 上面解決 / 存在しない ref / 同 feature_id 内 role 区別 / 決定性 3 回) | 重複 EntityRef を持つ Solid の検証 (将来 invariant check) |
| Edge/Vertex への将来拡張を妨げない構造 (兄弟メソッド方式) | trait 化 (`EntityNameLookup` trait など) |

## Non-Goals

- `CreateSketch.plane_ref` の format 拡張 → sub-Issue 8b
- ModelFace 上の Sketch 描画 → sub-Issue 8c
- Extrude / ExtrudeCut 機能本体 → sub-Issue 8d / 8e
- `EntityRef::Derived` variant の解決 → Phase 9 以降
- `Edge` / `Vertex` の逆引き本実装 → Phase 9 以降
- `FaceIndex` newtype/alias の導入 → 必要性が出てから別 Issue で再検討

## 実装対象

- Issue: #214
- 影響クレート/ファイル:
  - `crates/engawa-kernel/src/brep/topology.rs` — `Solid` の `impl` ブロックに新規メソッド追加
  - `crates/engawa-kernel/tests/find_face_by_entity_ref_acceptance.rs` — 新規 integration test

### 新規メソッドシグネチャ

```rust
impl Solid {
    /// Find a face whose `name` matches the given `EntityRef::Named { feature_id, kind: Face, role }`.
    ///
    /// Returns `Some(face_index)` for the first matching face, or `None` if:
    /// - `entity_ref` is not `EntityRef::Named` (i.e., `Derived`),
    /// - `entity_ref.kind != EntityKind::Face`,
    /// - no face's `name` matches.
    ///
    /// Deterministic: same input always yields the same index (linear scan over
    /// the insertion-ordered `faces` vector).
    pub fn find_face_by_entity_ref(&self, entity_ref: &EntityRef) -> Option<usize> {
        // implementation in STEP 6
    }
}
```

新規ファイル `tests/find_face_by_entity_ref_acceptance.rs` の skeleton は STEP 5.5 で生成 (テスト計画 T01〜T05 に対応する `#[ignore]` 付き関数を列挙)。既存関数の修正なし、新規追加のみ。

## 設計方針

- **決定性要件**: `faces: Vec<Face>` は挿入順保持の `Vec` で、`add_face` は決定的 ID 割り当て (`IdGenerator`) のため、同 `Solid` に対する同 input の line scan は同 index を返す。テスト T01 で 3 回繰り返しを検証。
- **B-rep トポロジー妥当性**: 読み取り専用 API のため Euler-Poincaré に影響なし。N/A。
- **退化幾何の扱い**: 入力 `EntityRef` の妥当性は `EntityRef::try_named` が事前検証済み (format crate の責務)。本 API は妥当な `EntityRef` を受け取る前提。`face.name == None` の face は match 対象外 (struct 比較で `Some(_) == None` は false)。
- **derive 規約**: 新規型を追加しないため不要。
- **エラーハンドリング**: `Result` ではなく `Option` を返す (検索系 API の慣行、`HashMap::get` と同じ)。`thiserror` 不要。
- **workspace.dependencies**: 新規依存追加なし。`engawa_format::{EntityRef, EntityKind}` は既に `crates/engawa-kernel/Cargo.toml` で参照済 (`primitives/cuboid.rs` 他で使用)。

### マッチング規約 (実装疑似コード)

```rust
match entity_ref {
    EntityRef::Named { kind: EntityKind::Face, .. } => {
        self.faces.iter().position(|f| f.name.as_ref() == Some(entity_ref))
    }
    _ => None,  // Derived variant, または kind != Face は弾く
}
```

`EntityRef` は `PartialEq` 派生済み (`feature.rs:27`) のため `==` で構造一致比較できる。

## テスト計画（ID 付き）

| ID | 種別 | 内容 | 期待結果 |
|----|------|------|----------|
| T01 | 決定性 | cuboid 生成 → `find_face_by_entity_ref` を 3 回呼んで同じ index が返るか | `assert_eq!(idx1, idx2); assert_eq!(idx2, idx3)` |
| T02 | 正常系 | cuboid の上面 (role="top") の `EntityRef` を取り出し `find_face_by_entity_ref` に渡すと、その index で得た face の name が一致 | `solid.faces[returned_idx].name == Some(ref_top)` |
| T03 | 正常系 | 同一 `feature_id="cuboid"` 内で `role="top"` と `role="bot"` の 2 ref が**別の** index を返す | `assert_ne!(top_idx, bot_idx)` |
| T04_boundary_missing_ref | 境界 | 存在しない feature_id の `EntityRef` で None | `assert_eq!(result, None)` |
| T05_degen_wrong_kind | 退化 | `EntityKind::Edge` を渡しても `None` (Face 以外は弾く) | `assert_eq!(result, None)` |
| T06_degen_derived_variant | 退化 | `EntityRef::Derived { ... }` を渡すと `None` (Named 以外は弾く) | `assert_eq!(result, None)` |

退化/境界ケース ID: `T04_boundary_missing_ref`, `T05_degen_wrong_kind`, `T06_degen_derived_variant` (3 件、最低 1 件の要件を満たす)。

## 幾何的不変条件チェックリスト

- [N/A] partition 出力の polygon 頂点順 — 本 Issue は読み取り API のみ
- [N/A] outer_loop 2D 向き — 同上
- [N/A] flip_normals / same_sense — 同上
- [N/A] pslg_subdivide の出力向き — 同上
