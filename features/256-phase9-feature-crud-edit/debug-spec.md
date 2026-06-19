# Debug Spec — #256 phase9-feature-crud-edit (round 3)

## 試した修正と結果 (round 2)

- [x] `check_edit_preserves_consumers` 新規追加 (pre/post simulate 比較で consumer preservation)
- [x] `EditBreaksConsumer` エラー追加
- [x] t07_deg_edit_steals_body_from_downstream / t08_deg_edit_retargets_plane_ref 退化テスト追加
- [x] CLI golden を input parse + feature 置換ベースに変更 (M-F02 解消)
- 結果: Codex r3 で critical=0 まで降りたが、新たな high 3 + medium 1:
  - **A-F01 / M-F01 (high 2, 同論点)**: `PlaneRef::Entity` を所有 body の `feature_id` 生存性だけで判定している → face role 変更 (e.g. `f_side_0003` の消失) を検出できない、後段 build で `FaceEntityRefNotFound`
  - **C-F01 (high)**: `t07_deg_edit_steals_body_from_downstream` で `use engawa_format::{SketchPlane, SketchSegment};` の `SketchPlane` が未使用 → `cargo clippy --tests -- -D warnings` fail
  - **C-F02 (medium)**: `check_refs_resolve_before(without_old)` 流用で `InsertBeforeProducer.producer_at` が short index で報告される → CLI 診断メッセージずれ

## 仮説 (round 3)

1. **C-F01 clippy 違反 (high)**: 単純な未使用 import 削除で解消。
2. **C-F02 index remap (medium)**: `without_old` で報告された producer_at index は、original history では `+1 if reported_idx >= idx` で復元可能。簡易 remap で対応。
3. **A-F01 / M-F01 (high 2)**: 完全 EntityRef 解決検証には `build_assembly` 相当の重い処理が必要。しかし plan の scope (履歴 Document 純関数変換) を超える。**static role compatibility check** で大半のケースをカバー:
   - `PlaneRef::Entity(Named { feature_id, role })` の role を producer feature の variant ベースで簡易検証
   - CreateBox なら valid role = {"f_x_pos", "f_x_neg", "f_y_pos", "f_y_neg", "f_z_pos", "f_z_neg"} のみ
   - CreateSphere / CreateCylinder / Extrude / etc. はそれぞれ valid role 一覧を持つ
   - role が invalid → reject (新エラー `EditFaceRoleInvalid` or 既存 `BodyNotFound` 拡張)
   - これは static check で perf 影響なし、build までは行わない

ただし完全 face role 列挙は heavy (variant ごとに role 体系が異なる) → **段階的アプローチ**:
- round 3 では C-F01 / C-F02 のみ修正
- A-F01 / M-F01 (PlaneRef::Entity 完全解決) は **本 Issue の scope を超える** ため follow-up Issue で対応。Codex r4 では「follow-up Issue 起票済み」を debug-spec に明記して deferred とする

## 関連ファイル

- `crates/engawa-build/src/feature_crud.rs` (`check_refs_resolve_before` L330, `edit` L729)
- `crates/engawa-build/tests/256_phase9_feature_crud_edit_acceptance.rs` (t07 の未使用 import)

## 修正方針 (round 3)

### 1. C-F01: clippy unused_imports 削除

`crates/engawa-build/tests/256_phase9_feature_crud_edit_acceptance.rs` の `t07_deg_edit_steals_body_from_downstream` で `use engawa_format::{SketchPlane, SketchSegment};` の中で実際に使われていない方を削除。GLM が test を確認して使われている import 体系に整理する。

具体的には、test 内で `SketchPlane::Xy` を使う場合は import を残し、`engawa_format::SketchPlane::Xy` のように完全修飾している場合は import 側を削除する。同様に `SketchSegment` も検証。

### 2. C-F02: edit 用 ref-resolve check で index remap

**before** (現状: `edit` 関数内):
```rust
check_refs_resolve_before(&new_feature, &without_old, idx)?;
```

**after** (新規 helper を edit 用に追加):
```rust
fn check_refs_resolve_before_for_edit(
    new_feature: &Feature,
    original_features: &[Feature],
    idx: usize,
) -> Result<(), FeatureCrudError> {
    // without_old を作って既存 check_refs_resolve_before を呼ぶが、
    // 返ってきた InsertBeforeProducer の producer_at は short index なので、
    // original index に remap してから返す。
    let mut without_old: Vec<Feature> = original_features.to_vec();
    without_old.remove(idx);
    match check_refs_resolve_before(new_feature, &without_old, idx) {
        Ok(()) => Ok(()),
        Err(FeatureCrudError::InsertBeforeProducer {
            feature_id,
            ref_id,
            producer_at,
            requested_at,
        }) => {
            // producer_at は without_old 上の index。original index に戻す:
            // original[idx] を削除しているので、short index が idx 以上なら +1。
            let original_producer_at = if producer_at >= idx { producer_at + 1 } else { producer_at };
            Err(FeatureCrudError::InsertBeforeProducer {
                feature_id,
                ref_id,
                producer_at: original_producer_at,
                requested_at,
            })
        }
        Err(other) => Err(other),
    }
}
```

`edit` 内の `check_refs_resolve_before(&new_feature, &without_old, idx)?;` を `check_refs_resolve_before_for_edit(&new_feature, &doc.root_component.features, idx)?;` に置換。

### 3. A-F01 / M-F01 (PlaneRef::Entity 完全解決): **follow-up Issue 起票で deferred**

Codex の指摘は妥当だが完全解決には:
- `build_assembly` を呼んで EntityRef を実解決する (perf 不利)、または
- variant 別の valid face role 列挙テーブルを作って static check する (実装重)

これらは本 Issue の scope (履歴 Document 純関数変換 + 基本 invariant check) を超える。Phase 9 の続編 (e.g. ADR-014 Component RefPlane 独立化 / ADR-015 Phase 9 設計基盤) で扱う。

**対応**: 本 round では実装しない。代わりに follow-up Issue を `raise-issue-on-failure.ts` で起票し、debug-spec に「deferred to follow-up Issue #XYZ」と明記する。Codex r4 で「scope 外として follow-up Issue 持ち越し」を主張。

GLM はこの方針を理解し、A-F01 / M-F01 関連の追加実装は不要。C-F01 / C-F02 のみ修正する。

## 次にやること

GLM で:
1. t07 / t08 の未使用 import を削除 (clippy clean)
2. `check_refs_resolve_before_for_edit` 新規追加 + `edit` 内呼び出し置換 (index remap)
3. A-F01 / M-F01 は対応しない (follow-up Issue で deferred)

## 追加で書いてほしいテスト

(round 3 では追加なし — C-F02 の index remap 動作は既存テストでカバー可能、深い回帰テストは follow-up Issue で対応)
