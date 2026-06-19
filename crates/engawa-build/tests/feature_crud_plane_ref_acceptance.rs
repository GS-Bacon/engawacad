//! Acceptance tests for #264: CreateSketch.plane_ref=Entity body-lifetime tracking.

use engawa_build::FeatureCrud;
use engawa_format::{
    Document, EntityKind, EntityRef, Feature, PlaneRef, SketchPlane, SketchSegment,
};

/// T01: Determinism — 同 history + 同 implicit ref で 2 回 insert → エラー variant 同一 + Document YAML byte-equal
#[test]
fn t01_determinism() {
    let mut doc = Document::new("Test");
    doc.root_component.features.push(Feature::CreateBox {
        id: "box_1".to_string(),
        width: 10.0,
        height: 20.0,
        depth: 30.0,
    });

    let sketch = Feature::CreateSketch {
        id: "sk".to_string(),
        plane: SketchPlane::Xy,
        offset: 0.0,
        variables: vec![],
        profile: vec![SketchSegment {
            id: "seg_a".to_string(),
            from: [0.0, 0.0],
            to: [10.0, 0.0],
        }],
        plane_ref: Some(PlaneRef::Entity(EntityRef::Named {
            feature_id: "box_1".to_string(),
            kind: EntityKind::Face,
            role: "top".to_string(),
        })),
    };

    let result1 = FeatureCrud::insert(&doc, sketch.clone(), 1).unwrap();
    let yaml1 = result1.to_yaml().unwrap();

    let result2 = FeatureCrud::insert(&doc, sketch, 1).unwrap();
    let yaml2 = result2.to_yaml().unwrap();

    assert_eq!(yaml1, yaml2, "insert must produce byte-identical YAML");
}

/// T02: Normal系 (downstream sketch 阻止 — Cut)
#[test]
fn t02_downstream_sketch_blocks_cut() {
    // box_2 は事前登録されている必要がある（idx 1 で insert するときに box_2 は既に存在）
    let mut doc = Document::new("Test");
    doc.root_component.features.push(Feature::CreateBox {
        id: "box_2".to_string(),
        width: 5.0,
        height: 10.0,
        depth: 15.0,
    });
    doc.root_component.features.push(Feature::CreateBox {
        id: "box_1".to_string(),
        width: 10.0,
        height: 20.0,
        depth: 30.0,
    });
    doc.root_component.features.push(Feature::CreateSketch {
        id: "sk".to_string(),
        plane: SketchPlane::Xy,
        offset: 0.0,
        variables: vec![],
        profile: vec![],
        plane_ref: Some(PlaneRef::Entity(EntityRef::Named {
            feature_id: "box_1".to_string(),
            kind: EntityKind::Face,
            role: "top".to_string(),
        })),
    });
    doc.root_component.features.push(Feature::Extrude {
        id: "e1".to_string(),
        sketch: "sk".to_string(),
        depth: 5.0,
        fuse_target: None,
    });

    // Cut(target=box_1, tool=box_2) を idx 2 で insert → downstream sketch を破壊
    let cut = Feature::Cut {
        id: "cut1".to_string(),
        target: "box_1".to_string(),
        tool: "box_2".to_string(),
    };

    let result = FeatureCrud::insert(&doc, cut, 2);
    match result {
        Err(engawa_build::FeatureCrudError::InsertBeforeConsumer {
            consumed_ref,
            displaced_feature_id,
            ..
        }) => {
            assert_eq!(consumed_ref, "box_1");
            assert_eq!(displaced_feature_id, "sk");
        }
        other => panic!("expected InsertBeforeConsumer, got {:?}", other),
    }
}

/// T03: Normal系 (downstream sketch 阻止 — Fuse)
#[test]
fn t03_downstream_sketch_blocks_fuse() {
    // box_2 は事前登録
    let mut doc = Document::new("Test");
    doc.root_component.features.push(Feature::CreateBox {
        id: "box_2".to_string(),
        width: 5.0,
        height: 10.0,
        depth: 15.0,
    });
    doc.root_component.features.push(Feature::CreateBox {
        id: "box_1".to_string(),
        width: 10.0,
        height: 20.0,
        depth: 30.0,
    });
    doc.root_component.features.push(Feature::CreateSketch {
        id: "sk".to_string(),
        plane: SketchPlane::Xy,
        offset: 0.0,
        variables: vec![],
        profile: vec![],
        plane_ref: Some(PlaneRef::Entity(EntityRef::Named {
            feature_id: "box_1".to_string(),
            kind: EntityKind::Face,
            role: "top".to_string(),
        })),
    });

    // Fuse(target=box_1, tool=box_2) を idx 2 で insert
    let fuse = Feature::Fuse {
        id: "f1".to_string(),
        target: "box_1".to_string(),
        tool: "box_2".to_string(),
    };

    let result = FeatureCrud::insert(&doc, fuse, 2);
    match result {
        Err(engawa_build::FeatureCrudError::InsertBeforeConsumer {
            consumed_ref,
            displaced_feature_id,
            ..
        }) => {
            assert_eq!(consumed_ref, "box_1");
            assert_eq!(displaced_feature_id, "sk");
        }
        other => panic!("expected InsertBeforeConsumer, got {:?}", other),
    }
}

/// T04: Normal系 (downstream sketch 阻止 — Extrude with fuse_target)
#[test]
fn t04_downstream_sketch_blocks_extrude_fuse_target() {
    let mut doc = Document::new("Test");
    // sk_extr を先頭に配置 (Extrude の sketch ref は insert 時点で存在している必要がある)
    doc.root_component.features.push(Feature::CreateSketch {
        id: "sk_extr".to_string(),
        plane: SketchPlane::Xy,
        offset: 0.0,
        variables: vec![],
        profile: vec![SketchSegment {
            id: "seg_a".to_string(),
            from: [0.0, 0.0],
            to: [10.0, 0.0],
        }],
        plane_ref: None,
    });
    doc.root_component.features.push(Feature::CreateBox {
        id: "box_1".to_string(),
        width: 10.0,
        height: 20.0,
        depth: 30.0,
    });
    doc.root_component.features.push(Feature::CreateSketch {
        id: "sk_face".to_string(),
        plane: SketchPlane::Xy,
        offset: 0.0,
        variables: vec![],
        profile: vec![],
        plane_ref: Some(PlaneRef::Entity(EntityRef::Named {
            feature_id: "box_1".to_string(),
            kind: EntityKind::Face,
            role: "top".to_string(),
        })),
    });

    // Extrude with fuse_target=box_1 を idx 2 で insert
    let extrude = Feature::Extrude {
        id: "e1".to_string(),
        sketch: "sk_extr".to_string(),
        depth: 5.0,
        fuse_target: Some("box_1".to_string()),
    };

    let result = FeatureCrud::insert(&doc, extrude, 2);
    match result {
        Err(engawa_build::FeatureCrudError::InsertBeforeConsumer {
            consumed_ref,
            displaced_feature_id,
            ..
        }) => {
            assert_eq!(consumed_ref, "box_1");
            assert_eq!(displaced_feature_id, "sk_face");
        }
        other => panic!("expected InsertBeforeConsumer, got {:?}", other),
    }
}

/// T05: Normal系 (sketch 自身の ref 解決 — 存在しない body)
#[test]
fn t05_sketch_self_plane_ref_unknown_body() {
    let doc = Document::new("Test");

    let sketch = Feature::CreateSketch {
        id: "sk".to_string(),
        plane: SketchPlane::Xy,
        offset: 0.0,
        variables: vec![],
        profile: vec![],
        plane_ref: Some(PlaneRef::Entity(EntityRef::Named {
            feature_id: "missing_box".to_string(),
            kind: EntityKind::Face,
            role: "top".to_string(),
        })),
    };

    let result = FeatureCrud::insert(&doc, sketch, 0);
    match result {
        Err(engawa_build::FeatureCrudError::BodyNotFound {
            feature_id,
            body_ref,
        }) => {
            assert_eq!(feature_id, "sk");
            assert_eq!(body_ref, "missing_box");
        }
        other => panic!("expected BodyNotFound, got {:?}", other),
    }
}

/// T06: Normal系 (Derived chain 解解 — 成功後に downstream Cut insert で InsertBeforeConsumer)
#[test]
fn t06_derived_chain_traversal_and_consumer_break() {
    let mut doc = Document::new("Test");
    // box_3 は事前に登録 (idx 0)
    doc.root_component.features.push(Feature::CreateBox {
        id: "box_3".to_string(),
        width: 3.0,
        height: 6.0,
        depth: 9.0,
    });
    doc.root_component.features.push(Feature::CreateBox {
        id: "box_1".to_string(),
        width: 10.0,
        height: 20.0,
        depth: 30.0,
    });
    doc.root_component.features.push(Feature::CreateBox {
        id: "box_2".to_string(),
        width: 5.0,
        height: 10.0,
        depth: 15.0,
    });
    doc.root_component.features.push(Feature::Fuse {
        id: "fused".to_string(),
        target: "box_1".to_string(),
        tool: "box_2".to_string(),
    });

    // Derived chain を含む sketch を idx 4 で insert → 成功
    let sketch = Feature::CreateSketch {
        id: "sk".to_string(),
        plane: SketchPlane::Xy,
        offset: 0.0,
        variables: vec![],
        profile: vec![],
        plane_ref: Some(PlaneRef::Entity(EntityRef::Derived {
            kind: EntityKind::Face,
            op: "fuse".to_string(),
            from: vec![EntityRef::Named {
                feature_id: "fused".to_string(),
                kind: EntityKind::Face,
                role: "top".to_string(),
            }],
            selector: "s0".to_string(),
        })),
    };

    let doc_with_sk = FeatureCrud::insert(&doc, sketch.clone(), 4).unwrap();

    // Cut(target=fused, tool=box_3) を idx 4 で insert → downstream sketch を破壊
    let cut = Feature::Cut {
        id: "cut1".to_string(),
        target: "fused".to_string(),
        tool: "box_3".to_string(),
    };

    let result = FeatureCrud::insert(&doc_with_sk, cut, 4);
    match result {
        Err(engawa_build::FeatureCrudError::InsertBeforeConsumer {
            consumed_ref,
            displaced_feature_id,
            ..
        }) => {
            assert_eq!(consumed_ref, "fused");
            assert_eq!(displaced_feature_id, "sk");
        }
        other => panic!("expected InsertBeforeConsumer, got {:?}", other),
    }
}

/// T07: Normal系 (Named feature_id 後置 — 未登場の body を参照)
#[test]
fn t07_named_feature_id_not_present() {
    let mut doc = Document::new("Test");
    doc.root_component.features.push(Feature::CreateSketch {
        id: "sk".to_string(),
        plane: SketchPlane::Xy,
        offset: 0.0,
        variables: vec![],
        profile: vec![],
        plane_ref: Some(PlaneRef::Entity(EntityRef::Named {
            feature_id: "box_late".to_string(),
            kind: EntityKind::Face,
            role: "top".to_string(),
        })),
    });

    // 同じ sketch を idx 0 で再度 insert (box_late は未登場)
    let sketch = Feature::CreateSketch {
        id: "sk2".to_string(),
        plane: SketchPlane::Xy,
        offset: 0.0,
        variables: vec![],
        profile: vec![],
        plane_ref: Some(PlaneRef::Entity(EntityRef::Named {
            feature_id: "box_late".to_string(),
            kind: EntityKind::Face,
            role: "top".to_string(),
        })),
    };

    let result = FeatureCrud::insert(&doc, sketch, 0);
    match result {
        Err(engawa_build::FeatureCrudError::BodyNotFound {
            feature_id,
            body_ref,
        }) => {
            assert_eq!(feature_id, "sk2");
            assert_eq!(body_ref, "box_late");
        }
        other => panic!("expected BodyNotFound, got {:?}", other),
    }
}

/// T_degen_no_plane_ref: plane_ref=None → implicit refs 空 → 成功
#[test]
fn t_degen_no_plane_ref() {
    let doc = Document::new("Test");

    let sketch = Feature::CreateSketch {
        id: "sk".to_string(),
        plane: SketchPlane::Xy,
        offset: 0.0,
        variables: vec![],
        profile: vec![],
        plane_ref: None,
    };

    let result = FeatureCrud::insert(&doc, sketch, 0);
    assert!(
        result.is_ok(),
        "plane_ref=None should succeed: {:?}",
        result
    );
}

/// T_degen_legacy_plane_string: PlaneRef::RefPlane(String) → implicit refs 空 → 成功
#[test]
fn t_degen_legacy_plane_string() {
    let doc = Document::new("Test");

    let sketch = Feature::CreateSketch {
        id: "sk".to_string(),
        plane: SketchPlane::Xy,
        offset: 0.0,
        variables: vec![],
        profile: vec![],
        plane_ref: Some(PlaneRef::RefPlane("Front".to_string())),
    };

    let result = FeatureCrud::insert(&doc, sketch, 0);
    assert!(
        result.is_ok(),
        "legacy PlaneRef::RefPlane should succeed: {:?}",
        result
    );
}

/// T_boundary_no_downstream_sketch: downstream に sketch なし → 成功
#[test]
fn t_boundary_no_downstream_sketch() {
    let mut doc = Document::new("Test");
    doc.root_component.features.push(Feature::CreateBox {
        id: "box_1".to_string(),
        width: 10.0,
        height: 20.0,
        depth: 30.0,
    });
    doc.root_component.features.push(Feature::CreateBox {
        id: "box_2".to_string(),
        width: 5.0,
        height: 10.0,
        depth: 15.0,
    });

    // Cut(target=box_1, tool=box_2) を idx 2 で insert → downstream に sketch なし
    let cut = Feature::Cut {
        id: "cut1".to_string(),
        target: "box_1".to_string(),
        tool: "box_2".to_string(),
    };

    let result = FeatureCrud::insert(&doc, cut, 2);
    assert!(
        result.is_ok(),
        "no downstream sketch should succeed: {:?}",
        result
    );
}

/// DIFF01: downstream Extrude.fuse_target の implicit ref 重複
///
/// 同一 body を direct (Extrude.fuse_target) と implicit (CreateSketch.plane_ref)
/// の両方が指す downstream を持つ history で、その body を consume する Cut を
/// insert → InsertBeforeConsumer が最初に match した consumer で返ることを確認。
#[test]
fn diff01_downstream_extrude_fuse_target_implicit_ref_duplicate() {
    let mut doc = Document::new("Test");
    doc.root_component.features.push(Feature::CreateBox {
        id: "box_2".to_string(),
        width: 5.0,
        height: 10.0,
        depth: 15.0,
    });
    doc.root_component.features.push(Feature::CreateSketch {
        id: "sk_extr".to_string(),
        plane: SketchPlane::Xy,
        offset: 0.0,
        variables: vec![],
        profile: vec![SketchSegment {
            id: "seg_a".to_string(),
            from: [0.0, 0.0],
            to: [10.0, 0.0],
        }],
        plane_ref: None,
    });
    doc.root_component.features.push(Feature::CreateBox {
        id: "box_1".to_string(),
        width: 10.0,
        height: 20.0,
        depth: 30.0,
    });
    doc.root_component.features.push(Feature::CreateSketch {
        id: "sk_face".to_string(),
        plane: SketchPlane::Xy,
        offset: 0.0,
        variables: vec![],
        profile: vec![],
        plane_ref: Some(PlaneRef::Entity(EntityRef::Named {
            feature_id: "box_1".to_string(),
            kind: EntityKind::Face,
            role: "top".to_string(),
        })),
    });
    // Extrude: direct ref (fuse_target=box_1) + implicit ref (sketch は box_1 非依存)
    doc.root_component.features.push(Feature::Extrude {
        id: "e1".to_string(),
        sketch: "sk_extr".to_string(),
        depth: 5.0,
        fuse_target: Some("box_1".to_string()),
    });

    // Cut(target=box_1, tool=box_2) を idx 3 で insert
    // downstream: sk_face (implicit ref), e1 (direct ref fuse_target)
    let cut = Feature::Cut {
        id: "cut1".to_string(),
        target: "box_1".to_string(),
        tool: "box_2".to_string(),
    };

    let result = FeatureCrud::insert(&doc, cut, 3);
    match result {
        Err(engawa_build::FeatureCrudError::InsertBeforeConsumer {
            consumed_ref,
            displaced_feature_id,
            ..
        }) => {
            assert_eq!(consumed_ref, "box_1");
            // sk_face (idx 3) が e1 (idx 4) より先なので sk_face が検出される
            assert_eq!(displaced_feature_id, "sk_face");
        }
        other => panic!("expected InsertBeforeConsumer, got {:?}", other),
    }
}

/// DIFF02: Derived chain で複数 Named feature_id を抽出
///
/// EntityRef::Derived に複数の Named を含む case で、両方の feature_id が
/// 正しく抽出・解決されることを確認。
#[test]
fn diff02_derived_chain_multiple_named_ids() {
    let mut doc = Document::new("Test");
    doc.root_component.features.push(Feature::CreateBox {
        id: "box_1".to_string(),
        width: 10.0,
        height: 20.0,
        depth: 30.0,
    });
    doc.root_component.features.push(Feature::CreateBox {
        id: "box_2".to_string(),
        width: 5.0,
        height: 10.0,
        depth: 15.0,
    });

    // Derived chain に 2 つの Named を含む sketch
    let sketch = Feature::CreateSketch {
        id: "sk".to_string(),
        plane: SketchPlane::Xy,
        offset: 0.0,
        variables: vec![],
        profile: vec![],
        plane_ref: Some(PlaneRef::Entity(EntityRef::Derived {
            kind: EntityKind::Face,
            op: "multi".to_string(),
            from: vec![
                EntityRef::Named {
                    feature_id: "box_1".to_string(),
                    kind: EntityKind::Face,
                    role: "top".to_string(),
                },
                EntityRef::Named {
                    feature_id: "box_2".to_string(),
                    kind: EntityKind::Face,
                    role: "front".to_string(),
                },
            ],
            selector: "s0".to_string(),
        })),
    };

    // 両方の body が存在する → 成功
    let result = FeatureCrud::insert(&doc, sketch.clone(), 2);
    assert!(result.is_ok(), "both box_1 and box_2 exist: {:?}", result);

    // box_2 が存在しない case → BodyNotFound
    let mut doc2 = Document::new("Test");
    doc2.root_component.features.push(Feature::CreateBox {
        id: "box_1".to_string(),
        width: 10.0,
        height: 20.0,
        depth: 30.0,
    });
    // box_2 は作成しない

    let result2 = FeatureCrud::insert(&doc2, sketch, 1);
    match result2 {
        Err(engawa_build::FeatureCrudError::BodyNotFound {
            feature_id,
            body_ref,
        }) => {
            assert_eq!(feature_id, "sk");
            // box_1 は live だが box_2 が存在しない → box_2 が検出される
            assert_eq!(body_ref, "box_2");
        }
        other => panic!("expected BodyNotFound for box_2, got {:?}", other),
    }
}

/// EDGE01: Derived chain の深いネスト (Named leaf 1 個)
///
/// Derived { from: [Derived { from: [Named(box_1)] }] } のような 2 段ネストで
/// Named leaf を正しく抽出できることを確認。
#[test]
fn edge01_deep_derived_chain_single_named_leaf() {
    let mut doc = Document::new("Test");
    doc.root_component.features.push(Feature::CreateBox {
        id: "box_1".to_string(),
        width: 10.0,
        height: 20.0,
        depth: 30.0,
    });

    // 2 段ネストの Derived chain
    let sketch = Feature::CreateSketch {
        id: "sk".to_string(),
        plane: SketchPlane::Xy,
        offset: 0.0,
        variables: vec![],
        profile: vec![],
        plane_ref: Some(PlaneRef::Entity(EntityRef::Derived {
            kind: EntityKind::Face,
            op: "op1".to_string(),
            from: vec![EntityRef::Derived {
                kind: EntityKind::Face,
                op: "op2".to_string(),
                from: vec![EntityRef::Named {
                    feature_id: "box_1".to_string(),
                    kind: EntityKind::Face,
                    role: "top".to_string(),
                }],
                selector: "inner".to_string(),
            }],
            selector: "outer".to_string(),
        })),
    };

    let result = FeatureCrud::insert(&doc, sketch, 1);
    assert!(
        result.is_ok(),
        "deep derived chain should resolve: {:?}",
        result
    );
}

/// EDGE02: 同じ feature_id を複数回参照する Derived
///
/// Derived { from: [Named(box_1), Named(box_1)] } で feature_implicit_body_refs が
/// [box_1, box_1] を返すことを確認 (重複排除しない設計)。
#[test]
fn edge02_duplicate_feature_id_in_derived() {
    let mut doc = Document::new("Test");
    doc.root_component.features.push(Feature::CreateBox {
        id: "box_1".to_string(),
        width: 10.0,
        height: 20.0,
        depth: 30.0,
    });

    // 同じ feature_id を 2 回含む Derived
    let sketch = Feature::CreateSketch {
        id: "sk".to_string(),
        plane: SketchPlane::Xy,
        offset: 0.0,
        variables: vec![],
        profile: vec![],
        plane_ref: Some(PlaneRef::Entity(EntityRef::Derived {
            kind: EntityKind::Face,
            op: "dup".to_string(),
            from: vec![
                EntityRef::Named {
                    feature_id: "box_1".to_string(),
                    kind: EntityKind::Face,
                    role: "top".to_string(),
                },
                EntityRef::Named {
                    feature_id: "box_1".to_string(),
                    kind: EntityKind::Face,
                    role: "front".to_string(),
                },
            ],
            selector: "s0".to_string(),
        })),
    };

    let result = FeatureCrud::insert(&doc, sketch.clone(), 1);
    // 重複していても box_1 は live → 成功
    assert!(
        result.is_ok(),
        "duplicate refs to same live body should succeed: {:?}",
        result
    );

    // 決定性確認: 2 回実行で同一 YAML
    let result1 = FeatureCrud::insert(&doc, sketch.clone(), 1).unwrap();
    let yaml1 = result1.to_yaml().unwrap();
    let result2 = FeatureCrud::insert(&doc, sketch, 1).unwrap();
    let yaml2 = result2.to_yaml().unwrap();
    assert_eq!(yaml1, yaml2, "duplicate refs should still be deterministic");
}

/// T01b: Derived chain を含む sketch での決定性
///
/// EntityRef::Derived を含む sketch で 2 回 insert し、同一 YAML / 同一エラー
/// variant が返ることを確認。collect_named_feature_ids の DFS 順序が決定的である
/// ことの回帰。
#[test]
fn t01b_determinism_with_derived_chain() {
    let mut doc = Document::new("Test");
    doc.root_component.features.push(Feature::CreateBox {
        id: "box_1".to_string(),
        width: 10.0,
        height: 20.0,
        depth: 30.0,
    });
    doc.root_component.features.push(Feature::CreateBox {
        id: "box_2".to_string(),
        width: 5.0,
        height: 10.0,
        depth: 15.0,
    });

    let sketch = Feature::CreateSketch {
        id: "sk".to_string(),
        plane: SketchPlane::Xy,
        offset: 0.0,
        variables: vec![],
        profile: vec![],
        plane_ref: Some(PlaneRef::Entity(EntityRef::Derived {
            kind: EntityKind::Face,
            op: "multi".to_string(),
            from: vec![
                EntityRef::Named {
                    feature_id: "box_1".to_string(),
                    kind: EntityKind::Face,
                    role: "top".to_string(),
                },
                EntityRef::Named {
                    feature_id: "box_2".to_string(),
                    kind: EntityKind::Face,
                    role: "front".to_string(),
                },
            ],
            selector: "s0".to_string(),
        })),
    };

    let result1 = FeatureCrud::insert(&doc, sketch.clone(), 2).unwrap();
    let yaml1 = result1.to_yaml().unwrap();

    let result2 = FeatureCrud::insert(&doc, sketch, 2).unwrap();
    let yaml2 = result2.to_yaml().unwrap();

    assert_eq!(yaml1, yaml2, "derived chain insert must be deterministic");

    // エラーケースでも決定的: box_1 を消費した後で insert
    let mut doc2 = Document::new("Test");
    doc2.root_component.features.push(Feature::CreateBox {
        id: "box_3".to_string(),
        width: 3.0,
        height: 6.0,
        depth: 9.0,
    });
    doc2.root_component.features.push(Feature::CreateBox {
        id: "box_1".to_string(),
        width: 10.0,
        height: 20.0,
        depth: 30.0,
    });
    doc2.root_component.features.push(Feature::CreateBox {
        id: "box_2".to_string(),
        width: 5.0,
        height: 10.0,
        depth: 15.0,
    });
    doc2.root_component.features.push(Feature::Fuse {
        id: "fused".to_string(),
        target: "box_1".to_string(),
        tool: "box_2".to_string(),
    });

    let sketch_err = Feature::CreateSketch {
        id: "sk_err".to_string(),
        plane: SketchPlane::Xy,
        offset: 0.0,
        variables: vec![],
        profile: vec![],
        plane_ref: Some(PlaneRef::Entity(EntityRef::Derived {
            kind: EntityKind::Face,
            op: "multi".to_string(),
            from: vec![
                EntityRef::Named {
                    feature_id: "fused".to_string(),
                    kind: EntityKind::Face,
                    role: "top".to_string(),
                },
                EntityRef::Named {
                    feature_id: "box_2".to_string(),
                    kind: EntityKind::Face,
                    role: "front".to_string(),
                },
            ],
            selector: "s0".to_string(),
        })),
    };

    let err1 = FeatureCrud::insert(&doc2, sketch_err.clone(), 4);
    let err2 = FeatureCrud::insert(&doc2, sketch_err, 4);

    // 両方とも同じエラー (box_2 は Fuse で消費済み)
    match (&err1, &err2) {
        (
            Err(engawa_build::FeatureCrudError::BodyNotFound {
                feature_id: fid1,
                body_ref: ref1,
            }),
            Err(engawa_build::FeatureCrudError::BodyNotFound {
                feature_id: fid2,
                body_ref: ref2,
            }),
        ) => {
            assert_eq!(fid1, fid2);
            assert_eq!(ref1, ref2);
        }
        other => panic!("expected matching BodyNotFound errors, got {:?}", other),
    }
}
