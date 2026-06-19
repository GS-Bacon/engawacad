# Debug Spec for #263 (STEP 7.5 Codex review round 1 fix-dispatch)

## 仮説

`check_refs_resolve_before` の **sketch 後方探索分岐** が、`Feature::CreateSketch` だけでなく任意の feature id にマッチしてしまっている。Architect と migration 両 persona が一致して指摘 (A-F02 / M-F02)。例:

```text
features = [CreateBox { id: "s1", .. }]
insert Extrude { id: "e1", sketch: "s1" } at at=0
→ sketches_at(at=0) は空、live_bodies_at(at=0) は空
→ exists_after は `feat.id() == "s1"` を CreateBox にマッチしてしまい true
→ InsertBeforeProducer { ref_id: "s1", producer_at: 0, requested_at: 0 } を返す
```

しかし正しくは **SketchNotFound { sketch_ref: "s1" }** を返すべき (id "s1" を持つ sketch は存在しないため)。

## 関連ファイル

- `crates/engawa-build/src/feature_crud.rs:207-232` — `check_refs_resolve_before` の sketch refs 分岐
- `crates/engawa-build/tests/feature_crud_acceptance.rs` — 回帰テスト T17 を追加する場所

## 修正方針

### 1. `check_refs_resolve_before` の sketch 後方探索を `Feature::CreateSketch` 限定に変更

**現状の sketch 分岐 (feature_crud.rs:207-232)**:

```rust
} else {
    // Check if sketch exists after insertion point
    let exists_after = features
        .iter()
        .enumerate()
        .any(|(i, feat)| i >= at && feat.id() == sketch_ref);
    if exists_after {
        return Err(FeatureCrudError::InsertBeforeProducer {
            feature_id: fid.to_string(),
            ref_id: sketch_ref.to_string(),
            producer_at: at
                + features
                    .iter()
                    .enumerate()
                    .skip(at)
                    .position(|(_i, feat)| feat.id() == sketch_ref)
                    .unwrap(),
            requested_at: at,
        });
    } else {
        return Err(FeatureCrudError::SketchNotFound {
            feature_id: fid.to_string(),
            sketch_ref: sketch_ref.to_string(),
        });
    }
}
```

**修正後 (body 分岐と同じパターンで `find_map` + variant 限定)**:

```rust
} else {
    // Check if a CreateSketch with this id exists after insertion point.
    // (body refs と同様に variant 限定で誤分類を防ぐ — Codex A-F02/M-F02)
    let producer_after = features
        .iter()
        .enumerate()
        .skip(at)
        .find_map(|(i, feat)| match feat {
            Feature::CreateSketch { id, .. } if id == sketch_ref => Some(i),
            _ => None,
        });

    if let Some(producer_idx) = producer_after {
        return Err(FeatureCrudError::InsertBeforeProducer {
            feature_id: fid.to_string(),
            ref_id: sketch_ref.to_string(),
            producer_at: producer_idx,
            requested_at: at,
        });
    } else {
        return Err(FeatureCrudError::SketchNotFound {
            feature_id: fid.to_string(),
            sketch_ref: sketch_ref.to_string(),
        });
    }
}
```

`find_map(|(i, feat)| ...)` の `i` は `enumerate()` の絶対 index なので `at +` 加算は不要。これにより body 分岐 (250-272 行) と一貫したロジックになる。

### 2. 回帰テスト T17 を追加

`crates/engawa-build/tests/feature_crud_acceptance.rs` に以下を追加 (`#[ignore]` なし、即 active):

```rust
#[test]
fn t17_sketch_back_search_excludes_non_sketch_producer() {
    // Codex review #263 round 1 A-F02/M-F02: mixed-type ID collision で
    // sketch 後方探索が CreateBox にマッチしてはならない。
    let mut doc = Document::new("Test");
    doc.root_component.features.push(Feature::CreateBox {
        id: "s1".to_string(),
        width: 10.0,
        height: 20.0,
        depth: 30.0,
    });
    let feature = Feature::Extrude {
        id: "extrude_1".to_string(),
        sketch: "s1".to_string(),
        depth: 5.0,
        fuse_target: None,
    };
    let result = FeatureCrud::insert(&doc, feature, 0);
    assert!(
        matches!(
            result,
            Err(FeatureCrudError::SketchNotFound {
                ref feature_id,
                ref sketch_ref,
            }) if feature_id == "extrude_1" && sketch_ref == "s1"
        ),
        "expected SketchNotFound but got: {:?}",
        result
    );
}
```

## 試した修正と結果

- (未着手 — 本 round の指示は debug-spec を読んだ GLM が実装する)

## 次にやること

1. 上記方針で `crates/engawa-build/src/feature_crud.rs` の sketch 経路を修正
2. `crates/engawa-build/tests/feature_crud_acceptance.rs` に T17 を追加
3. `cargo xtask ci` を走らせ green を確認 (全 1183+1 テスト pass)
4. 既存の T01〜T16 が回帰しないこと

## 追加で書いてほしいテスト

- T17_sketch_back_search_excludes_non_sketch_producer (上記)

## scope defend / 棄却した指摘

- C-F02 (contrarian, high): `CreateSketch.plane_ref = PlaneRef::Entity(...)` の implicit body lifetime traversal → 本 Issue scope を越えるため別 Issue #264 に切り出し、plan.md Non-Goals に追記済み。本 round の修正対象外。
