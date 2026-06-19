## In-Scope / Out-of-Scope

| In-Scope | Out-of-Scope |
|----------|--------------|
| `feature_implicit_body_refs(&Feature) -> Vec<String>` 新規 helper の追加 (CreateSketch.plane_ref=Entity 経路の body provenance 抽出) | 他 Feature variant (Extrude/Cut/Fuse/Intersect 等) の implicit refs (本 Issue では CreateSketch.plane_ref のみ) |
| `collect_named_feature_ids(&EntityRef, &mut Vec<String>)` 新規 helper (Named: feature_id 抽出 / Derived: from を再帰 traverse) | ADR-015 (#246) の `FeatureOp` enum / `Document::apply_op` 一元化案 (ADR-015 確定後に refactor 予定) |
| `check_refs_resolve_before(f, features, at)` への implicit ref チェック追加 (live_bodies_at に未登録 → BodyNotFound / 後置 → InsertBeforeProducer) | format-level validate (`Document::validate`) の拡張 (本 Issue は engawa-build の semantic gate に限定) |
| `check_no_downstream_break(f, features, at)` の downstream 走査拡張 (downstream feature の `feature_consumes` に加え `feature_implicit_body_refs` も収集して consumed body との交差判定) | implicit refs を伴う Update / Delete 操作の対応 (本 Issue は insert のみ。FeatureCrud::insert のみ touch) |
| 既存 test 19 件の回帰維持 + Insert 系新規 acceptance test (integration test as #263 pattern) | Codex C-F02 が言及した build_assembly 側の防御的 fallback (build 時 FaceEntityRefNotFound に追加 metadata) |
| 既存 4 variants エラー型 (SketchNotFound / BodyNotFound / InsertBeforeProducer / InsertBeforeConsumer) の再利用 | 新規エラー variant 追加 (FeatureCrudError は `#[non_exhaustive]` だが、既存と意味論が一致するため再利用する) |

## Non-Goals

- 他 Feature variant (Extrude/Cut/Fuse/Intersect/Cylinder/Sphere) の plane_ref 系 implicit ref 検出 — CreateSketch.plane_ref のみが現状の唯一の implicit body ref パスである (`grep -n "EntityRef\|plane_ref" crates/engawa-format/src/feature.rs` で確認済)
- ADR-015 (#246 needs-human) で議論中の `FeatureOp` enum + `Document::apply_op` 一元化での lifetime 集約案 — ADR-015 確定後に refactor 予定。本 Issue は局所修正にとどめる
- `feature_body_refs` 自体の意味論変更 — `feature_consumes` から呼ばれており、CreateSketch.plane_ref は consume しないため body_refs 側に混ぜると意味論が壊れる。よって新規 helper を分離
- 新規 FeatureCrudError variant 追加 — 既存 4 variants で意味論が成立する (live でない body 参照 → BodyNotFound / consumer による displaced → InsertBeforeConsumer)
- build_assembly 側 (engawa-build/src/build.rs 系) の防御的 fallback — Issue C-F02 の意図は「semantic gate 段階で防ぐ」であり、build 側の挙動は変えない

## 実装対象

Issue: #264
影響クレート/ファイル:
- `crates/engawa-build/src/feature_crud.rs` (新規 helper 2 個 + 2 既存関数の拡張)
- `crates/engawa-build/tests/feature_crud_plane_ref_acceptance.rs` (新規 integration test)

### 変更箇所 1: 新規 helper 2 個を追加 (feature_body_refs の直下に挿入)

**Before** (line 84-93, 直接 body refs のみ):
```rust
/// Extract body IDs referenced by a feature.
fn feature_body_refs(f: &Feature) -> Vec<&str> {
    match f {
        Feature::Extrude { fuse_target, .. } => fuse_target.iter().map(|s| s.as_str()).collect(),
        Feature::ExtrudeCut { target, .. } => vec![target.as_str()],
        Feature::Cut { target, tool, .. } => vec![target.as_str(), tool.as_str()],
        Feature::Fuse { target, tool, .. } => vec![target.as_str(), tool.as_str()],
        Feature::Intersect { target, tool, .. } => vec![target.as_str(), tool.as_str()],
        _ => vec![],
    }
}
```

**After** (line 93 末尾の直下に追加。`feature_body_refs` 本体は無改変):
```rust
/// Extract body IDs implicitly referenced by a feature via topological entity refs.
///
/// Currently only `CreateSketch.plane_ref = PlaneRef::Entity(EntityRef)` is tracked:
/// face-attached sketches carry an implicit lifetime dependency on the body that owns
/// the referenced face. The provenance tree is walked recursively so `EntityRef::Derived`
/// chains resolve back to their `Named.feature_id` leaves.
///
/// Returns owned `String` (vs `&str`) because `Derived` traversal may produce values
/// not directly borrowable from `f` in the future (current impl only borrows, but the
/// owned shape keeps the API stable if Derived ever materialises new strings).
fn feature_implicit_body_refs(f: &Feature) -> Vec<String> {
    let mut refs = Vec::new();
    if let Feature::CreateSketch {
        plane_ref: Some(PlaneRef::Entity(eref)),
        ..
    } = f
    {
        collect_named_feature_ids(eref, &mut refs);
    }
    refs
}

/// Walk an `EntityRef` provenance tree and collect every `Named.feature_id`.
///
/// `EntityRef::Named` is a leaf → push its `feature_id`.
/// `EntityRef::Derived { from, .. }` recurses into each provenance child.
fn collect_named_feature_ids(eref: &EntityRef, acc: &mut Vec<String>) {
    match eref {
        EntityRef::Named { feature_id, .. } => acc.push(feature_id.clone()),
        EntityRef::Derived { from, .. } => {
            for child in from {
                collect_named_feature_ids(child, acc);
            }
        }
    }
}
```

(use 文を `use engawa_format::{Document, EntityRef, Feature, PlaneRef};` に拡張する)

### 変更箇所 2: `check_refs_resolve_before` に implicit ref チェック追加

**Before** (line 236-282 抜粋 body refs ループ末尾):
```rust
            } else {
                return Err(FeatureCrudError::BodyNotFound {
                    feature_id: fid.to_string(),
                    body_ref: body_ref.to_string(),
                });
            }
        }
    }

    Ok(())
}
```

**After** (`Ok(())` の直前に同形のループを追加):
```rust
            } else {
                return Err(FeatureCrudError::BodyNotFound {
                    feature_id: fid.to_string(),
                    body_ref: body_ref.to_string(),
                });
            }
        }
    }

    // Check implicit body refs (e.g. CreateSketch.plane_ref entity provenance).
    // Same semantics as body refs but resolved against live_bodies_at: the face must
    // belong to a body that is live at `at` (i.e. created earlier and not yet consumed).
    for implicit_ref in feature_implicit_body_refs(f) {
        if let Some(&idx) = live_bodies_at.get(&implicit_ref) {
            if idx >= at {
                return Err(FeatureCrudError::InsertBeforeProducer {
                    feature_id: fid.to_string(),
                    ref_id: implicit_ref,
                    producer_at: idx,
                    requested_at: at,
                });
            }
        } else {
            let producer_after = features.iter().enumerate().skip(at).find_map(|(i, feat)| {
                if feat.id() == implicit_ref {
                    match feat {
                        Feature::CreateBox { .. }
                        | Feature::CreateCylinder { .. }
                        | Feature::CreateSphere { .. }
                        | Feature::Extrude { .. }
                        | Feature::ExtrudeCut { .. }
                        | Feature::Cut { .. }
                        | Feature::Fuse { .. }
                        | Feature::Intersect { .. } => Some(i),
                        _ => None,
                    }
                } else {
                    None
                }
            });

            if let Some(producer_idx) = producer_after {
                return Err(FeatureCrudError::InsertBeforeProducer {
                    feature_id: fid.to_string(),
                    ref_id: implicit_ref,
                    producer_at: producer_idx,
                    requested_at: at,
                });
            } else {
                return Err(FeatureCrudError::BodyNotFound {
                    feature_id: fid.to_string(),
                    body_ref: implicit_ref,
                });
            }
        }
    }

    Ok(())
}
```

### 変更箇所 3: `check_no_downstream_break` の downstream consumer 走査を拡張

**Before** (line 297-299 抜粋):
```rust
        for (consumer_idx, consumer_feat) in features.iter().enumerate().skip(at) {
            let consumer_refs = feature_consumes(consumer_feat);
            if consumer_refs.contains(&body_id) {
```

**After** (consumer 側で implicit refs も合算判定):
```rust
        for (consumer_idx, consumer_feat) in features.iter().enumerate().skip(at) {
            let consumer_refs: Vec<&str> = feature_consumes(consumer_feat);
            let implicit_consumer_refs = feature_implicit_body_refs(consumer_feat);

            let direct_match = consumer_refs.contains(&body_id);
            let implicit_match = implicit_consumer_refs.iter().any(|r| r == body_id);
            if direct_match || implicit_match {
```

(以降の re-registered 判定ロジックは無改変。implicit_match 経由でも同じ「body が再登録されたか」を確認し、されていなければ `InsertBeforeConsumer` を返す。`displaced_feature_id` には downstream `consumer_feat.id()` が入る。)

## 設計方針

- **決定性**: `feature_implicit_body_refs` は `Vec<String>` を返す純関数。`collect_named_feature_ids` も DFS 順序固定 (Vec の push 順 = from の iter 順)。同一入力 → 同一順序 → check のエラー pattern も決定的 (T01 で検証)
- **B-rep トポロジー妥当性**: 本 Issue は format-level semantic gate のみ。kernel 側 (engawa-kernel) には触れない → Euler-Poincaré 不変条件は影響なし
- **退化幾何の扱い**: 該当なし (本 Issue は ref tracking のみ。幾何 tolerance を扱わない)
- **derive 規約**: 既存 `EntityRef`/`PlaneRef` の derive は変えない (Debug/Clone/Serialize/Deserialize/JsonSchema/TS, PartialEq は既に付与)。新規型なし
- **エラーハンドリング**: 既存 `FeatureCrudError::BodyNotFound` / `InsertBeforeProducer` / `InsertBeforeConsumer` を再利用 (`#[non_exhaustive]` ではあるが、実用上 4 variants の意味論で過不足なし。新規 variant 追加は scope-creep)
- **workspace.dependencies**: 新規 dep 追加なし (`engawa-format` から `EntityRef`/`PlaneRef` を import する path 拡張のみ)

### 数値モデル

本 Issue は数値モデルを扱わない (semantic gate / ref tracking のみ)。tolerance なし。

## テスト計画（ID 付き）

| ID | 種別 | 内容 | 期待結果 |
|----|------|------|----------|
| T01 | 決定性 | 同 history + 同 implicit ref で 2 回 insert → エラー variant 同一 + Document YAML byte-equal | `assert_eq!` |
| T02 | 正常系 (downstream sketch 阻止) | history = [CreateBox box_1, CreateSketch sk (plane_ref=Entity(Named box_1)), Extrude e1] に Cut(target=box_1, tool=box_2) を idx 1 で insert (box_2 は事前 register) | `Err(InsertBeforeConsumer { consumed_ref: "box_1", displaced_feature_id: "sk", .. })` |
| T03 | 正常系 (Fuse insert) | T02 と同 history で Fuse(target=box_1, tool=box_2) を idx 1 で insert | `Err(InsertBeforeConsumer { consumed_ref: "box_1", displaced_feature_id: "sk", .. })` |
| T04 | 正常系 (Extrude fuse_target insert) | history に CreateBox box_1, CreateSketch sk_face (plane_ref=Entity(Named box_1)), CreateSketch sk_extr (簡素な XY) に対し Extrude { sketch=sk_extr, fuse_target=box_1 } を sk_face の前に insert | `Err(InsertBeforeConsumer { consumed_ref: "box_1", displaced_feature_id: "sk_face", .. })` |
| T05 | 正常系 (sketch 自身の ref 解決) | 空 history に CreateSketch (plane_ref=Entity(Named missing_box)) を insert | `Err(BodyNotFound { feature_id: "sk", body_ref: "missing_box" })` |
| T06 | 正常系 (Derived chain 解決) | history = [CreateBox box_1, CreateBox box_2, Fuse fused (target=box_1, tool=box_2)] に CreateSketch (plane_ref=Entity(Derived { from: [Named(fused)], .. })) を idx 3 で insert → 成功。直後 Cut(target=fused, tool=box_3) を idx 3 で insert すると downstream sketch を破壊 | 1 つ目 `Ok(_)`、2 つ目 `Err(InsertBeforeConsumer)` |
| T07 | 正常系 (Named feature_id 後置) | history = [CreateSketch sk (plane_ref=Entity(Named box_late))] に対し idx 0 で再度同 sketch を insert (box_late は未登場) | `Err(BodyNotFound { feature_id: "sk", body_ref: "box_late" })` (未登場なので producer_after も None) |
| T_degen_no_plane_ref | 退化 (plane_ref=None) | CreateSketch with `plane_ref: None` を insert → implicit refs 空 → 既存挙動どおり通過 | `Ok(_)` |
| T_degen_legacy_plane_string | 退化 (legacy RefPlane(String)) | CreateSketch with `plane_ref=Some(PlaneRef::RefPlane("Front"))` を insert → implicit refs 空 (Entity variant でないため) → 既存挙動どおり通過 | `Ok(_)` |
| T_boundary_no_downstream_sketch | 境界 (downstream sketch なし) | history = [CreateBox box_1] に Cut(target=box_1, tool=box_2) を idx 1 で insert (downstream に sketch なし) → 拡張ロジックが false positive を出さないことを確認 | `Ok(_)` |

注: T01〜T07 は plane_ref entity 経路を含む新規系。T_degen_* / T_boundary_* は既存挙動の回帰確認 (拡張で副作用が出ていないことを保証)。

## 幾何的不変条件チェックリスト

- N/A (本 Issue は Boolean / Partition / Assemble 系ではなく engawa-build/feature_crud の semantic gate 拡張)
