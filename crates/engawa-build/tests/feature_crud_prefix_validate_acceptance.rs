//! Acceptance tests for #265: pre-existing history の broken ref を defensive validate.

use engawa_build::FeatureCrud;
use engawa_format::{
    Document, EntityKind, EntityRef, Feature, PlaneRef, SketchPlane, SketchSegment,
};

/// T01: 決定性 — broken prefix history + 同 insert を 2 回 → エラー variant 同一 + Document YAML byte-equal
#[test]
fn t01_determinism() {
    let mut doc = Document::new("Test");
    doc.root_component.features.push(Feature::CreateBox {
        id: "box_b1".to_string(),
        width: 10.0,
        height: 10.0,
        depth: 10.0,
    });
    doc.root_component.features.push(Feature::CreateBox {
        id: "box_b2".to_string(),
        width: 5.0,
        height: 5.0,
        depth: 5.0,
    });
    doc.root_component.features.push(Feature::Cut {
        id: "c1".to_string(),
        target: "box_b1".to_string(),
        tool: "missing".to_string(),
    });

    let feature = Feature::Cut {
        id: "f1".to_string(),
        target: "c1".to_string(),
        tool: "box_b2".to_string(),
    };

    let err1 = FeatureCrud::insert(&doc, feature.clone(), 3);
    let err2 = FeatureCrud::insert(&doc, feature, 3);

    // 同一エラー variant
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

/// T02: 正常系 (Cut tool broken — c1 が skip → f1 の tool ref=c1 が BodyNotFound)
#[test]
fn t02_broken_cut_tool_skips_output() {
    let mut doc = Document::new("Test");
    doc.root_component.features.push(Feature::CreateBox {
        id: "box_b1".to_string(),
        width: 10.0,
        height: 10.0,
        depth: 10.0,
    });
    doc.root_component.features.push(Feature::CreateBox {
        id: "box_b2".to_string(),
        width: 5.0,
        height: 5.0,
        depth: 5.0,
    });
    doc.root_component.features.push(Feature::Cut {
        id: "c1".to_string(),
        target: "box_b1".to_string(),
        tool: "missing".to_string(),
    });

    let feature = Feature::Cut {
        id: "f1".to_string(),
        target: "c1".to_string(),
        tool: "box_b2".to_string(),
    };

    let result = FeatureCrud::insert(&doc, feature, 3);
    match result {
        Err(engawa_build::FeatureCrudError::BodyNotFound {
            feature_id,
            body_ref,
        }) => {
            assert_eq!(feature_id, "f1");
            assert_eq!(body_ref, "c1");
        }
        other => panic!("expected BodyNotFound for c1, got {:?}", other),
    }
}

/// T03: 正常系 (Extrude sketch broken — e1 が skip → f1 の target ref=e1 が BodyNotFound)
#[test]
fn t03_broken_extrude_sketch_skips_output() {
    let mut doc = Document::new("Test");
    doc.root_component.features.push(Feature::CreateBox {
        id: "box_b1".to_string(),
        width: 10.0,
        height: 10.0,
        depth: 10.0,
    });
    doc.root_component.features.push(Feature::CreateSketch {
        id: "sk_valid".to_string(),
        plane: engawa_format::SketchPlane::Xy,
        offset: 0.0,
        variables: vec![],
        profile: vec![engawa_format::SketchSegment {
            id: "s1".to_string(),
            from: [0.0, 0.0],
            to: [10.0, 0.0],
        }],
        plane_ref: None,
    });
    doc.root_component.features.push(Feature::Extrude {
        id: "e1".to_string(),
        sketch: "missing_sk".to_string(),
        depth: 5.0,
        fuse_target: Some("box_b1".to_string()),
    });

    let feature = Feature::Cut {
        id: "f1".to_string(),
        target: "e1".to_string(),
        tool: "box_b1".to_string(),
    };

    let result = FeatureCrud::insert(&doc, feature, 3);
    match result {
        Err(engawa_build::FeatureCrudError::BodyNotFound {
            feature_id,
            body_ref,
        }) => {
            assert_eq!(feature_id, "f1");
            assert_eq!(body_ref, "e1");
        }
        other => panic!("expected BodyNotFound for e1, got {:?}", other),
    }
}

/// T04: 正常系 (Fuse target broken — g1 が skip → f1 の target ref=g1 が BodyNotFound)
#[test]
fn t04_broken_fuse_target_skips_output() {
    let mut doc = Document::new("Test");
    doc.root_component.features.push(Feature::CreateBox {
        id: "box_b1".to_string(),
        width: 10.0,
        height: 10.0,
        depth: 10.0,
    });
    doc.root_component.features.push(Feature::CreateBox {
        id: "box_b2".to_string(),
        width: 5.0,
        height: 5.0,
        depth: 5.0,
    });
    doc.root_component.features.push(Feature::Fuse {
        id: "g1".to_string(),
        target: "missing_a".to_string(),
        tool: "box_b1".to_string(),
    });

    let feature = Feature::Cut {
        id: "f1".to_string(),
        target: "g1".to_string(),
        tool: "box_b2".to_string(),
    };

    let result = FeatureCrud::insert(&doc, feature, 3);
    match result {
        Err(engawa_build::FeatureCrudError::BodyNotFound {
            feature_id,
            body_ref,
        }) => {
            assert_eq!(feature_id, "f1");
            assert_eq!(body_ref, "g1");
        }
        other => panic!("expected BodyNotFound for g1, got {:?}", other),
    }
}

/// T05: 正常系 (Intersect tool broken — i1 が skip → f1 の target ref=i1 が BodyNotFound)
#[test]
fn t05_broken_intersect_tool_skips_output() {
    let mut doc = Document::new("Test");
    doc.root_component.features.push(Feature::CreateBox {
        id: "box_b1".to_string(),
        width: 10.0,
        height: 10.0,
        depth: 10.0,
    });
    doc.root_component.features.push(Feature::CreateBox {
        id: "box_b2".to_string(),
        width: 5.0,
        height: 5.0,
        depth: 5.0,
    });
    doc.root_component.features.push(Feature::Intersect {
        id: "i1".to_string(),
        target: "box_b1".to_string(),
        tool: "missing".to_string(),
    });

    let feature = Feature::Cut {
        id: "f1".to_string(),
        target: "i1".to_string(),
        tool: "box_b2".to_string(),
    };

    let result = FeatureCrud::insert(&doc, feature, 3);
    match result {
        Err(engawa_build::FeatureCrudError::BodyNotFound {
            feature_id,
            body_ref,
        }) => {
            assert_eq!(feature_id, "f1");
            assert_eq!(body_ref, "i1");
        }
        other => panic!("expected BodyNotFound for i1, got {:?}", other),
    }
}

/// T06: cascade — broken prefix 後の good feature も skip (c1 skip → e1 の fuse_target=c1 が resolve せず e1 も skip)
#[test]
fn t06_cascade_broken_prefix_invalidates_dependent() {
    let mut doc = Document::new("Test");
    doc.root_component.features.push(Feature::CreateSketch {
        id: "sk1".to_string(),
        plane: engawa_format::SketchPlane::Xy,
        offset: 0.0,
        variables: vec![],
        profile: vec![engawa_format::SketchSegment {
            id: "s1".to_string(),
            from: [0.0, 0.0],
            to: [10.0, 0.0],
        }],
        plane_ref: None,
    });
    doc.root_component.features.push(Feature::CreateBox {
        id: "box_b1".to_string(),
        width: 10.0,
        height: 10.0,
        depth: 10.0,
    });
    doc.root_component.features.push(Feature::Cut {
        id: "c1".to_string(),
        target: "box_b1".to_string(),
        tool: "missing".to_string(),
    });
    doc.root_component.features.push(Feature::Extrude {
        id: "e1".to_string(),
        sketch: "sk1".to_string(),
        depth: 5.0,
        fuse_target: Some("c1".to_string()),
    });

    let feature = Feature::Cut {
        id: "f1".to_string(),
        target: "e1".to_string(),
        tool: "box_b1".to_string(),
    };

    let result = FeatureCrud::insert(&doc, feature, 4);
    match result {
        Err(engawa_build::FeatureCrudError::BodyNotFound {
            feature_id,
            body_ref,
        }) => {
            assert_eq!(feature_id, "f1");
            assert_eq!(body_ref, "e1");
        }
        other => panic!("expected BodyNotFound for e1, got {:?}", other),
    }
}

/// T07: broken でない box は consume されず live のまま (b1 は c1 が skip されたため依然 live)
#[test]
fn t07_skipped_broken_keeps_inputs_live() {
    let mut doc = Document::new("Test");
    doc.root_component.features.push(Feature::CreateBox {
        id: "box_b1".to_string(),
        width: 10.0,
        height: 10.0,
        depth: 10.0,
    });
    doc.root_component.features.push(Feature::CreateBox {
        id: "box_b2".to_string(),
        width: 5.0,
        height: 5.0,
        depth: 5.0,
    });
    doc.root_component.features.push(Feature::Cut {
        id: "c1".to_string(),
        target: "box_b1".to_string(),
        tool: "missing".to_string(),
    });

    let feature = Feature::Cut {
        id: "f1".to_string(),
        target: "box_b1".to_string(),
        tool: "box_b2".to_string(),
    };

    let result = FeatureCrud::insert(&doc, feature, 3);
    assert!(
        result.is_ok(),
        "b1 is still live, should succeed: {:?}",
        result
    );
}

/// T08: ExtrudeCut sketch broken — ec1 が skip → f1 の target ref=ec1 が BodyNotFound
#[test]
fn t08_broken_extrudecut_sketch_skips_output() {
    let mut doc = Document::new("Test");
    doc.root_component.features.push(Feature::CreateBox {
        id: "box_b1".to_string(),
        width: 10.0,
        height: 10.0,
        depth: 10.0,
    });
    doc.root_component.features.push(Feature::CreateBox {
        id: "box_b2".to_string(),
        width: 5.0,
        height: 5.0,
        depth: 5.0,
    });
    doc.root_component.features.push(Feature::ExtrudeCut {
        id: "ec1".to_string(),
        sketch: "missing_sk".to_string(),
        target: "box_b1".to_string(),
        depth: 5.0,
    });

    let feature = Feature::Cut {
        id: "f1".to_string(),
        target: "ec1".to_string(),
        tool: "box_b2".to_string(),
    };

    let result = FeatureCrud::insert(&doc, feature, 3);
    match result {
        Err(engawa_build::FeatureCrudError::BodyNotFound {
            feature_id,
            body_ref,
        }) => {
            assert_eq!(feature_id, "f1");
            assert_eq!(body_ref, "ec1");
        }
        other => panic!("expected BodyNotFound for ec1, got {:?}", other),
    }
}

/// T_degen_clean_history: clean な history で既存挙動と変わらないこと
#[test]
fn t_degen_clean_history() {
    let mut doc = Document::new("Test");
    doc.root_component.features.push(Feature::CreateBox {
        id: "box_b1".to_string(),
        width: 10.0,
        height: 10.0,
        depth: 10.0,
    });
    doc.root_component.features.push(Feature::CreateBox {
        id: "box_b2".to_string(),
        width: 5.0,
        height: 5.0,
        depth: 5.0,
    });

    let feature = Feature::Cut {
        id: "f1".to_string(),
        target: "box_b1".to_string(),
        tool: "box_b2".to_string(),
    };

    let result = FeatureCrud::insert(&doc, feature, 2);
    assert!(result.is_ok(), "clean history should succeed: {:?}", result);
}

/// T_degen_first_feature_broken: 先頭が broken — CreateBox 自身に ref がないため成功
#[test]
fn t_degen_first_feature_broken() {
    let mut doc = Document::new("Test");
    doc.root_component.features.push(Feature::Cut {
        id: "c1".to_string(),
        target: "missing_a".to_string(),
        tool: "missing_b".to_string(),
    });

    let feature = Feature::CreateBox {
        id: "box_new".to_string(),
        width: 10.0,
        height: 10.0,
        depth: 10.0,
    };

    let result = FeatureCrud::insert(&doc, feature, 0);
    assert!(
        result.is_ok(),
        "CreateBox has no refs, should succeed: {:?}",
        result
    );
}

/// T_boundary_existing_acceptance_passes: 既存挙動回帰 (clean history で従来挙動が変わらないこと)
#[test]
fn t_boundary_existing_acceptance_passes() {
    // refs_resolve_in_state 追加で既存の clean history 挙動が変わらないことを確認:
    // (1) 空 doc に CreateBox を tail insert → Ok
    // (2) box があるところに同 id の CreateSphere を tail insert → DuplicateFeatureId
    // (3) 単純 history に Cut を tail insert (resolve OK) → Ok
    let mut doc = Document::new("Test");

    let result = FeatureCrud::insert(
        &doc,
        Feature::CreateBox {
            id: "box_a".to_string(),
            width: 1.0,
            height: 1.0,
            depth: 1.0,
        },
        0,
    );
    assert!(result.is_ok(), "(1) empty doc + CreateBox: {:?}", result);

    doc.root_component.features.push(Feature::CreateBox {
        id: "box_a".to_string(),
        width: 1.0,
        height: 1.0,
        depth: 1.0,
    });
    let result = FeatureCrud::insert(
        &doc,
        Feature::CreateSphere {
            id: "box_a".to_string(),
            radius: 1.0,
            center: [0.0, 0.0, 0.0],
        },
        1,
    );
    assert!(
        matches!(
            result,
            Err(engawa_build::FeatureCrudError::DuplicateFeatureId { .. })
        ),
        "(2) duplicate id rejected: {:?}",
        result
    );

    doc.root_component.features.push(Feature::CreateBox {
        id: "box_b".to_string(),
        width: 1.0,
        height: 1.0,
        depth: 1.0,
    });
    let result = FeatureCrud::insert(
        &doc,
        Feature::Cut {
            id: "cut_ab".to_string(),
            target: "box_a".to_string(),
            tool: "box_b".to_string(),
        },
        2,
    );
    assert!(result.is_ok(), "(3) clean Cut at tail: {:?}", result);
}

/// FORWARD01: broken future consumer の前に insert → e1 が skip されて real consumer ではないため Ok
#[test]
fn forward01_broken_future_consumer_not_real_consumer() {
    let mut doc = Document::new("Test");
    doc.root_component.features.push(Feature::CreateBox {
        id: "box_b1".to_string(),
        width: 10.0,
        height: 10.0,
        depth: 10.0,
    });
    doc.root_component.features.push(Feature::CreateBox {
        id: "box_b2".to_string(),
        width: 5.0,
        height: 5.0,
        depth: 5.0,
    });
    doc.root_component.features.push(Feature::Extrude {
        id: "e1".to_string(),
        sketch: "missing_sk".to_string(),
        depth: 5.0,
        fuse_target: Some("box_b1".to_string()),
    });

    // e1 は sketch=missing_sk で skip される → box_b1 を consumer しない → box_b1 は live のまま
    let feature = Feature::Cut {
        id: "f1".to_string(),
        target: "box_b1".to_string(),
        tool: "box_b2".to_string(),
    };

    let result = FeatureCrud::insert(&doc, feature, 2);
    assert!(
        result.is_ok(),
        "e1 is skipped, box_b1 remains live: {:?}",
        result
    );
}

/// FORWARD02: broken future producer の前に insert → c1 が skip されて BodyNotFound (InsertBeforeProducer ではない)
#[test]
fn forward02_broken_future_producer_not_real_producer() {
    let mut doc = Document::new("Test");
    doc.root_component.features.push(Feature::CreateBox {
        id: "box_b1".to_string(),
        width: 10.0,
        height: 10.0,
        depth: 10.0,
    });
    doc.root_component.features.push(Feature::CreateBox {
        id: "box_b2".to_string(),
        width: 5.0,
        height: 5.0,
        depth: 5.0,
    });
    doc.root_component.features.push(Feature::Cut {
        id: "c1".to_string(),
        target: "missing".to_string(),
        tool: "missing".to_string(),
    });

    // c1 は broken (refs 解決不可) で skip される → c1 という body は実在しない
    let feature = Feature::Cut {
        id: "new".to_string(),
        target: "c1".to_string(),
        tool: "box_b1".to_string(),
    };

    let result = FeatureCrud::insert(&doc, feature, 2);
    match result {
        Err(engawa_build::FeatureCrudError::BodyNotFound {
            feature_id,
            body_ref,
        }) => {
            assert_eq!(feature_id, "new");
            assert_eq!(body_ref, "c1");
        }
        other => panic!(
            "expected BodyNotFound for c1 (not InsertBeforeProducer), got {:?}",
            other
        ),
    }
}

/// FORWARD03: broken future re-register が無効化される
#[test]
fn forward03_broken_future_reregister_ignored() {
    let mut doc = Document::new("Test");
    doc.root_component.features.push(Feature::CreateBox {
        id: "box_b1".to_string(),
        width: 10.0,
        height: 10.0,
        depth: 10.0,
    });
    doc.root_component.features.push(Feature::CreateBox {
        id: "box_b2".to_string(),
        width: 5.0,
        height: 5.0,
        depth: 5.0,
    });
    // real consumer downstream
    doc.root_component.features.push(Feature::Cut {
        id: "c_real".to_string(),
        target: "box_b1".to_string(),
        tool: "box_b2".to_string(),
    });
    // broken Cut(box_b1) は re-register にならない (skip される)
    doc.root_component.features.push(Feature::Cut {
        id: "box_b1".to_string(),
        target: "missing".to_string(),
        tool: "missing".to_string(),
    });

    // idx 2 で挿入 (c_real の前)
    // box_b1 は c_real で消費されるため InsertBeforeConsumer になる
    // (broken box_b1 は skip されるため re-register されない)
    let feature = Feature::Cut {
        id: "consume_b1".to_string(),
        target: "box_b1".to_string(),
        tool: "box_b2".to_string(),
    };

    let result = FeatureCrud::insert(&doc, feature, 2);
    match result {
        Err(engawa_build::FeatureCrudError::InsertBeforeConsumer { consumed_ref, .. }) => {
            assert_eq!(consumed_ref, "box_b1");
        }
        other => panic!("expected InsertBeforeConsumer, got {:?}", other),
    }
}

/// FORWARD04: executed_at の決定性 — 同一 history を 2 回 simulate → insert の挙動が同一
#[test]
fn forward04_executed_at_determinism() {
    let mut doc = Document::new("Test");
    doc.root_component.features.push(Feature::CreateBox {
        id: "box_b1".to_string(),
        width: 10.0,
        height: 10.0,
        depth: 10.0,
    });
    doc.root_component.features.push(Feature::Cut {
        id: "c1".to_string(),
        target: "missing".to_string(),
        tool: "missing".to_string(),
    });

    let feature = Feature::Cut {
        id: "f1".to_string(),
        target: "box_b1".to_string(),
        tool: "any".to_string(),
    };

    let err1 = FeatureCrud::insert(&doc, feature.clone(), 2);
    let err2 = FeatureCrud::insert(&doc, feature, 2);

    // 同一エラー variant
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

/// DIFF01: Extrude with broken sketch but Some(fuse_target) — box_b1 は consume されず live のまま
#[test]
fn diff01_extrude_broken_sketch_keeps_fuse_target_live() {
    let mut doc = Document::new("Test");
    doc.root_component.features.push(Feature::CreateBox {
        id: "box_b1".to_string(),
        width: 10.0,
        height: 10.0,
        depth: 10.0,
    });
    doc.root_component.features.push(Feature::CreateBox {
        id: "box_b2".to_string(),
        width: 5.0,
        height: 5.0,
        depth: 5.0,
    });
    doc.root_component.features.push(Feature::Extrude {
        id: "e1".to_string(),
        sketch: "missing_sk".to_string(),
        depth: 5.0,
        fuse_target: Some("box_b1".to_string()),
    });

    // e1 が skip されたため box_b1 は live のまま
    let feature = Feature::Cut {
        id: "f1".to_string(),
        target: "box_b1".to_string(),
        tool: "box_b2".to_string(),
    };

    let result = FeatureCrud::insert(&doc, feature, 3);
    assert!(result.is_ok(), "box_b1 should still be live: {:?}", result);
}

/// DIFF02: Cut with both target and tool broken — c1 が skip される
#[test]
fn diff02_cut_both_broken_skips_output() {
    let mut doc = Document::new("Test");
    doc.root_component.features.push(Feature::Cut {
        id: "c1".to_string(),
        target: "missing_a".to_string(),
        tool: "missing_b".to_string(),
    });

    // c1 が skip されたため c1 は live に入らない
    let feature = Feature::Cut {
        id: "f1".to_string(),
        target: "c1".to_string(),
        tool: "any".to_string(),
    };

    let result = FeatureCrud::insert(&doc, feature, 1);
    match result {
        Err(engawa_build::FeatureCrudError::BodyNotFound {
            feature_id,
            body_ref,
        }) => {
            assert_eq!(feature_id, "f1");
            assert_eq!(body_ref, "c1");
        }
        other => panic!("expected BodyNotFound for c1, got {:?}", other),
    }
}

/// EDGE01a: broken prefix → 後で同 id の正常 register (tool=missing のため BodyNotFound)
#[test]
fn edge01_id_re_registration_then_tool_missing() {
    let mut doc = Document::new("Test");
    doc.root_component.features.push(Feature::Cut {
        id: "c1".to_string(),
        target: "missing_a".to_string(),
        tool: "missing_b".to_string(),
    });
    doc.root_component.features.push(Feature::CreateBox {
        id: "c1".to_string(),
        width: 10.0,
        height: 10.0,
        depth: 10.0,
    });

    // c1 は最初の Cut で skip されるが、CreateBox(c1) で改めて live になる
    let feature = Feature::Cut {
        id: "f1".to_string(),
        target: "c1".to_string(),
        tool: "missing_tool".to_string(),
    };

    let result = FeatureCrud::insert(&doc, feature, 2);
    match result {
        Err(engawa_build::FeatureCrudError::BodyNotFound {
            feature_id,
            body_ref,
        }) => {
            assert_eq!(feature_id, "f1");
            assert_eq!(body_ref, "missing_tool");
        }
        other => panic!("expected BodyNotFound for missing_tool, got {:?}", other),
    }
}

/// EDGE01b: ExtrudeCut with broken sketch — target は consume されず live のまま
#[test]
fn edge01_extrudecut_broken_sketch_keeps_target_live() {
    let mut doc = Document::new("Test");
    doc.root_component.features.push(Feature::CreateBox {
        id: "box_b1".to_string(),
        width: 10.0,
        height: 10.0,
        depth: 10.0,
    });
    doc.root_component.features.push(Feature::CreateBox {
        id: "box_b2".to_string(),
        width: 5.0,
        height: 5.0,
        depth: 5.0,
    });
    doc.root_component.features.push(Feature::ExtrudeCut {
        id: "ec1".to_string(),
        sketch: "missing_sk".to_string(),
        target: "box_b1".to_string(),
        depth: 5.0,
    });

    // ec1 が skip されたため box_b1 は live のまま
    let feature = Feature::Cut {
        id: "f1".to_string(),
        target: "box_b1".to_string(),
        tool: "box_b2".to_string(),
    };

    let result = FeatureCrud::insert(&doc, feature, 3);
    assert!(result.is_ok(), "box_b1 should still be live: {:?}", result);
}

/// EDGE02: broken prefix が複数連鎖 (cascade)
#[test]
fn edge02_cascade_broken_prefix_chain() {
    let mut doc = Document::new("Test");
    doc.root_component.features.push(Feature::Cut {
        id: "c1".to_string(),
        target: "missing_a".to_string(),
        tool: "missing_b".to_string(),
    });
    doc.root_component.features.push(Feature::Cut {
        id: "c2".to_string(),
        target: "c1".to_string(),
        tool: "missing_d".to_string(),
    });
    doc.root_component.features.push(Feature::Cut {
        id: "c3".to_string(),
        target: "c2".to_string(),
        tool: "missing_e".to_string(),
    });

    // c1, c2, c3 すべて skip される
    let feature = Feature::CreateBox {
        id: "box_new".to_string(),
        width: 10.0,
        height: 10.0,
        depth: 10.0,
    };

    let result = FeatureCrud::insert(&doc, feature, 3);
    assert!(
        result.is_ok(),
        "CreateBox has no refs, should succeed: {:?}",
        result
    );
}

/// T01b: 決定性 (YAML byte-equal) — Ok path の 2 回実行で byte-equal
#[test]
fn t01b_determinism_yaml_byteequal() {
    let mut doc = Document::new("Test");
    doc.root_component.features.push(Feature::CreateBox {
        id: "box_b1".to_string(),
        width: 10.0,
        height: 10.0,
        depth: 10.0,
    });
    doc.root_component.features.push(Feature::CreateBox {
        id: "box_b2".to_string(),
        width: 5.0,
        height: 5.0,
        depth: 5.0,
    });
    doc.root_component.features.push(Feature::Cut {
        id: "c1".to_string(),
        target: "box_b1".to_string(),
        tool: "missing".to_string(),
    });

    let feature = Feature::Cut {
        id: "f1".to_string(),
        target: "box_b1".to_string(),
        tool: "box_b2".to_string(),
    };

    let doc1 = FeatureCrud::insert(&doc, feature.clone(), 3).unwrap();
    let doc2 = FeatureCrud::insert(&doc, feature, 3).unwrap();

    let yaml1 = doc1.to_yaml().unwrap();
    let yaml2 = doc2.to_yaml().unwrap();

    assert_eq!(
        yaml1, yaml2,
        "two inserts with same input should produce byte-identical YAML"
    );
}

/// R2 (#266): early consumer + later valid re-register → first-unprotected consumer
/// が返ることを確認 (last-consumer 畳み込み回帰の防止)
///
/// History pattern:
///   [box_2, box_1, sk(plane=Entity(box_1)), CreateBox box_1 (re-register), e2(Extrude sketch=sk)]
///
/// - sk (idx 2) は box_1 の early consumer (implicit ref via plane_ref)
/// - CreateBox box_1 (idx 3) で re-register
/// - e2 (idx 4) は re-register 後の consumer (transitive ref via sketch)
///
/// Cut(target=box_1, tool=box_2) を idx 2 に insert → first-unprotected consumer (sk) が返る。
#[test]
fn t_266_r2_early_consumer_with_later_reregister_returns_first() {
    let mut doc = Document::new("Test");
    // box_2 は Cut.tool 用
    doc.root_component.features.push(Feature::CreateBox {
        id: "box_2".to_string(),
        width: 5.0,
        height: 10.0,
        depth: 15.0,
    });
    // box_1 は対象の body
    doc.root_component.features.push(Feature::CreateBox {
        id: "box_1".to_string(),
        width: 10.0,
        height: 20.0,
        depth: 30.0,
    });
    // sk は box_1 の early consumer (implicit ref via plane_ref)
    doc.root_component.features.push(Feature::CreateSketch {
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
    });
    // box_1 を re-register
    doc.root_component.features.push(Feature::CreateBox {
        id: "box_1".to_string(),
        width: 10.0,
        height: 20.0,
        depth: 30.0,
    });
    // e2 は re-register 後の consumer (transitive ref via sketch)
    doc.root_component.features.push(Feature::Extrude {
        id: "e2".to_string(),
        sketch: "sk".to_string(),
        depth: 5.0,
        fuse_target: None,
    });

    // idx 2 で Cut(target=box_1, tool=box_2) を insert
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
            // first-unprotected semantics で sk (early consumer) が返る。
            // re-register 後の e2 は保護されるが、sk は re-register 前なので未保護。
            assert_eq!(displaced_feature_id, "sk");
        }
        other => panic!("expected InsertBeforeConsumer with sk, got {:?}", other),
    }
}

/// T_degen_no_activation_no_change: Degen — broken-future が無い clean history で既存挙動を維持 (回帰防止)
#[test]
fn t_267_determinism() {
    let mut doc = Document::new("Test");
    doc.root_component.features.push(Feature::CreateBox {
        id: "box_b1".to_string(),
        width: 10.0,
        height: 10.0,
        depth: 10.0,
    });
    doc.root_component.features.push(Feature::CreateBox {
        id: "box_b2".to_string(),
        width: 5.0,
        height: 5.0,
        depth: 5.0,
    });
    doc.root_component.features.push(Feature::Cut {
        id: "c1".to_string(),
        target: "new_box".to_string(),
        tool: "box_b1".to_string(),
    });
    doc.root_component.features.push(Feature::Cut {
        id: "c2".to_string(),
        target: "box_b1".to_string(),
        tool: "box_b2".to_string(),
    });

    let feature = Feature::CreateBox {
        id: "new_box".to_string(),
        width: 3.0,
        height: 3.0,
        depth: 3.0,
    };

    let err1 = FeatureCrud::insert(&doc, feature.clone(), 2);
    let err2 = FeatureCrud::insert(&doc, feature, 2);

    match (&err1, &err2) {
        (
            Err(engawa_build::FeatureCrudError::InsertBeforeConsumer {
                consumed_ref: ref1,
                displaced_feature_id: id1,
                consumer_at: at1,
                ..
            }),
            Err(engawa_build::FeatureCrudError::InsertBeforeConsumer {
                consumed_ref: ref2,
                displaced_feature_id: id2,
                consumer_at: at2,
                ..
            }),
        ) => {
            assert_eq!(ref1, ref2);
            assert_eq!(id1, id2);
            assert_eq!(at1, at2);
        }
        other => panic!(
            "expected matching InsertBeforeConsumer errors, got {:?}",
            other
        ),
    }
}

/// T_267 (#267): Normal — `[box_b1, box_b2, Cut(c1, target=new_box, tool=box_b1), Cut(c2, target=box_b1, tool=box_b2)]` に
/// `CreateBox(new_box)` を idx 2 insert → c1 が活性化 → c2 が壊れる
#[test]
fn t_267_cut_activates_broken_consumer() {
    let mut doc = Document::new("Test");
    doc.root_component.features.push(Feature::CreateBox {
        id: "box_b1".to_string(),
        width: 10.0,
        height: 10.0,
        depth: 10.0,
    });
    doc.root_component.features.push(Feature::CreateBox {
        id: "box_b2".to_string(),
        width: 5.0,
        height: 5.0,
        depth: 5.0,
    });
    doc.root_component.features.push(Feature::Cut {
        id: "c1".to_string(),
        target: "new_box".to_string(),
        tool: "box_b1".to_string(),
    });
    doc.root_component.features.push(Feature::Cut {
        id: "c2".to_string(),
        target: "box_b1".to_string(),
        tool: "box_b2".to_string(),
    });

    let feature = Feature::CreateBox {
        id: "new_box".to_string(),
        width: 3.0,
        height: 3.0,
        depth: 3.0,
    };

    let result = FeatureCrud::insert(&doc, feature, 2);
    match result {
        Err(engawa_build::FeatureCrudError::InsertBeforeConsumer {
            consumed_ref,
            displaced_feature_id,
            ..
        }) => {
            assert_eq!(consumed_ref, "box_b1");
            assert_eq!(displaced_feature_id, "c2");
        }
        other => panic!("expected InsertBeforeConsumer, got {:?}", other),
    }
}

/// T_267 (#267): Normal — Fuse 同パターン
#[test]
fn t_267_fuse_activates_broken_consumer() {
    let mut doc = Document::new("Test");
    doc.root_component.features.push(Feature::CreateBox {
        id: "box_b1".to_string(),
        width: 10.0,
        height: 10.0,
        depth: 10.0,
    });
    doc.root_component.features.push(Feature::CreateBox {
        id: "box_b2".to_string(),
        width: 5.0,
        height: 5.0,
        depth: 5.0,
    });
    doc.root_component.features.push(Feature::Fuse {
        id: "f1".to_string(),
        target: "new_box".to_string(),
        tool: "box_b1".to_string(),
    });
    doc.root_component.features.push(Feature::Cut {
        id: "c2".to_string(),
        target: "box_b1".to_string(),
        tool: "box_b2".to_string(),
    });

    let feature = Feature::CreateBox {
        id: "new_box".to_string(),
        width: 3.0,
        height: 3.0,
        depth: 3.0,
    };

    let result = FeatureCrud::insert(&doc, feature, 2);
    match result {
        Err(engawa_build::FeatureCrudError::InsertBeforeConsumer {
            consumed_ref,
            displaced_feature_id,
            ..
        }) => {
            assert_eq!(consumed_ref, "box_b1");
            assert_eq!(displaced_feature_id, "c2");
        }
        other => panic!("expected InsertBeforeConsumer, got {:?}", other),
    }
}

/// T_267 (#267): Normal — Intersect 同パターン
#[test]
fn t_267_intersect_activates_broken_consumer() {
    let mut doc = Document::new("Test");
    doc.root_component.features.push(Feature::CreateBox {
        id: "box_b1".to_string(),
        width: 10.0,
        height: 10.0,
        depth: 10.0,
    });
    doc.root_component.features.push(Feature::CreateBox {
        id: "box_b2".to_string(),
        width: 5.0,
        height: 5.0,
        depth: 5.0,
    });
    doc.root_component.features.push(Feature::Intersect {
        id: "i1".to_string(),
        target: "new_box".to_string(),
        tool: "box_b1".to_string(),
    });
    doc.root_component.features.push(Feature::Cut {
        id: "c2".to_string(),
        target: "box_b1".to_string(),
        tool: "box_b2".to_string(),
    });

    let feature = Feature::CreateBox {
        id: "new_box".to_string(),
        width: 3.0,
        height: 3.0,
        depth: 3.0,
    };

    let result = FeatureCrud::insert(&doc, feature, 2);
    match result {
        Err(engawa_build::FeatureCrudError::InsertBeforeConsumer {
            consumed_ref,
            displaced_feature_id,
            ..
        }) => {
            assert_eq!(consumed_ref, "box_b1");
            assert_eq!(displaced_feature_id, "c2");
        }
        other => panic!("expected InsertBeforeConsumer, got {:?}", other),
    }
}

/// T_267 (#267): Degen — broken-future が無い clean history で従来挙動を維持 (回帰防止)
#[test]
fn t_267_degen_no_activation_no_change() {
    let mut doc = Document::new("Test");
    doc.root_component.features.push(Feature::CreateBox {
        id: "box_1".to_string(),
        width: 10.0,
        height: 10.0,
        depth: 10.0,
    });
    doc.root_component.features.push(Feature::CreateBox {
        id: "box_2".to_string(),
        width: 5.0,
        height: 5.0,
        depth: 5.0,
    });
    doc.root_component.features.push(Feature::Cut {
        id: "c1".to_string(),
        target: "box_1".to_string(),
        tool: "box_2".to_string(),
    });

    let feature = Feature::CreateSphere {
        id: "sp".to_string(),
        radius: 3.0,
        center: [0.0, 0.0, 0.0],
    };

    let result = FeatureCrud::insert(&doc, feature, 2);
    assert!(result.is_ok(), "clean history should succeed: {:?}", result);
}
