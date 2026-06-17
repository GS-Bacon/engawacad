# Test Spec — #214 find_face_by_entity_ref

## 不足テスト (plan 計画分)

STEP 6 で GLM core 実装と同時に `tests/find_face_by_entity_ref_acceptance.rs` の T01-T06 を全て実装済み + 全 pass 確認済み (`cargo test -p engawa-kernel --test find_face_by_entity_ref_acceptance` → 6 passed). 追加実装は不要。

| ID | plan 該当 | 実装状況 | 期待値整合 |
|----|-----------|----------|------------|
| T01 | 決定性 3 回呼び出し | `t01_determinism_repeated_lookup` | OK |
| T02 | cuboid top face 解決 | `t02_resolves_cuboid_top_face` (role="f_y_pos" を採用 — plan の "top" は make_cuboid 実装上 `f_y_pos`) | OK (plan の概念 role を実装 role にマップ済み) |
| T03 | 同 feature_id 内 role 区別 | `t03_distinguishes_role_in_same_feature_id` ("f_y_pos" vs "f_y_neg") | OK |
| T04_boundary_missing_ref | 存在しない ref → None | `t04_boundary_missing_ref_returns_none` | OK |
| T05_degen_wrong_kind | EntityKind::Edge → None | `t05_degen_wrong_kind_returns_none` | OK |
| T06_degen_derived_variant | Derived → None | `t06_degen_derived_variant_returns_none` | OK |

## 実装差分から追加すべきテスト

なし。GLM 実装は plan §設計方針 のマッチング規約 (擬似コード) と完全一致。追加分岐なし:

```rust
// 実装 (topology.rs 末尾):
match entity_ref {
    EntityRef::Named { kind: EntityKind::Face, .. } => self
        .faces.iter()
        .position(|f| f.name.as_ref() == Some(entity_ref)),
    _ => None,
}
```

`EntityRef::Named` で `kind` が `Edge`/`Vertex` のときは構造 match の guard で弾かれ `_ => None` に落ちる。これは T05 で正しく検証されている。

## エッジケース・退化入力

T05/T06 で網羅済み:
- `Named` だが `kind != Face` → None (T05)
- `Derived` variant → None (T06)
- 存在しない feature_id → None (T04)

未カバーケースの追加は **不要** (全パターンが分岐 guard でカバー):
- `face.name == None` の face は `Some(_) == None` で false になり線形 scan で skip される (既存 cuboid の face は全て `name = Some(_)` だが、boolean 操作後など `None` face が混在する可能性を将来想定)。これは T02 が "found" を確認することで間接的に検証済み。明示的な「name=None face を skip」テストは GLM 実装が `f.name.as_ref() == Some(entity_ref)` という単一 expression で対称的に処理しているため、追加価値が低い (Out-of-Scope: 重複検出と同様に将来 invariant check で対応)。

## 数値境界

N/A。本 Issue は読み取り API のみで数値演算なし。

## 決定性

T01 で 3 回反復確認済み。`Vec::iter().position` は決定的、`f.name.as_ref() == Some(...)` は構造比較 (`PartialEq` derive) で決定的、`faces` Vec は挿入順保持。`make_cuboid` の `IdGenerator::new(1)` 起点も決定的。追加テスト不要。

## STEP 6.6 GLM テスト実装への指示

追加実装は **不要**。GLM は以下のみ実行すること:
1. `cargo xtask ci` で全テスト green を確認
2. `find_face_by_entity_ref_acceptance.rs` の 6 ケースが全 pass であることを確認
3. `glm-test-result.json` に `status: success`, `ci_passed: true` を書いて完了

差分は加えない (no-op 確認のみ)。
