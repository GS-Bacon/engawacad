//! Acceptance tests for Feature CRUD semantic validation (#263).
//!
//! Skeleton placed in STEP 5.5. Bodies are filled in STEP 6 by GLM
//! together with the production code change.

use engawa_build::feature_crud::{FeatureCrud, FeatureCrudError};
use engawa_format::{Document, Feature, SketchPlane, SketchSegment};

#[test]
fn t01_determinism() {
    let mut doc = Document::new("Test");
    doc.root_component.features.push(Feature::CreateSketch {
        id: "sketch_1".to_string(),
        plane: SketchPlane::Xy,
        offset: 0.0,
        variables: vec![],
        profile: vec![
            SketchSegment {
                id: "s1".to_string(),
                from: [0.0, 0.0],
                to: [10.0, 0.0],
            },
            SketchSegment {
                id: "s2".to_string(),
                from: [10.0, 0.0],
                to: [10.0, 10.0],
            },
            SketchSegment {
                id: "s3".to_string(),
                from: [10.0, 10.0],
                to: [0.0, 10.0],
            },
            SketchSegment {
                id: "s4".to_string(),
                from: [0.0, 10.0],
                to: [0.0, 0.0],
            },
        ],
        plane_ref: None,
    });
    doc.root_component.features.push(Feature::CreateBox {
        id: "box_1".to_string(),
        width: 10.0,
        height: 20.0,
        depth: 30.0,
    });
    let feature = Feature::Extrude {
        id: "extrude_1".to_string(),
        sketch: "sketch_1".to_string(),
        depth: 5.0,
        fuse_target: None,
    };
    let yaml1 = FeatureCrud::insert(&doc, feature.clone(), 2)
        .unwrap()
        .to_yaml()
        .unwrap();
    let yaml2 = FeatureCrud::insert(&doc, feature, 2)
        .unwrap()
        .to_yaml()
        .unwrap();
    assert_eq!(yaml1, yaml2);
}

#[test]
fn t02_normal_insert_extrude_after_sketch() {
    let mut doc = Document::new("Test");
    doc.root_component.features.push(Feature::CreateSketch {
        id: "sketch_1".to_string(),
        plane: SketchPlane::Xy,
        offset: 0.0,
        variables: vec![],
        profile: vec![SketchSegment {
            id: "s1".to_string(),
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
    let feature = Feature::Extrude {
        id: "extrude_1".to_string(),
        sketch: "sketch_1".to_string(),
        depth: 5.0,
        fuse_target: None,
    };
    let result = FeatureCrud::insert(&doc, feature, 2).unwrap();
    assert_eq!(result.root_component.features.len(), 3);
}

#[test]
fn t03_sketch_not_found() {
    let doc = Document::new("Test");
    let feature = Feature::Extrude {
        id: "extrude_1".to_string(),
        sketch: "unknown".to_string(),
        depth: 5.0,
        fuse_target: None,
    };
    let result = FeatureCrud::insert(&doc, feature, 0);
    assert!(matches!(
        result,
        Err(FeatureCrudError::SketchNotFound {
            feature_id,
            sketch_ref
        }) if feature_id == "extrude_1" && sketch_ref == "unknown"
    ));
}

#[test]
fn t04_body_not_found() {
    let doc = Document::new("Test");
    let feature = Feature::Cut {
        id: "cut_1".to_string(),
        target: "unknown".to_string(),
        tool: "box_1".to_string(),
    };
    let result = FeatureCrud::insert(&doc, feature, 0);
    assert!(matches!(
        result,
        Err(FeatureCrudError::BodyNotFound {
            feature_id,
            body_ref
        }) if feature_id == "cut_1" && body_ref == "unknown"
    ));
}

#[test]
fn t05_insert_before_producer() {
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
        height: 5.0,
        depth: 5.0,
    });
    let feature = Feature::Cut {
        id: "cut_1".to_string(),
        target: "box_1".to_string(),
        tool: "box_2".to_string(),
    };
    let result = FeatureCrud::insert(&doc, feature, 0);
    assert!(matches!(
        result,
        Err(FeatureCrudError::InsertBeforeProducer {
            ref_id,
            producer_at: 0,
            requested_at: 0,
            ..
        }) if ref_id == "box_1" || ref_id == "box_2"
    ));
}

#[test]
fn t06_insert_before_consumer() {
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
        height: 5.0,
        depth: 5.0,
    });
    doc.root_component.features.push(Feature::Cut {
        id: "cut_1".to_string(),
        target: "box_1".to_string(),
        tool: "box_2".to_string(),
    });
    let feature = Feature::Cut {
        id: "cut_2".to_string(),
        target: "box_1".to_string(),
        tool: "box_2".to_string(),
    };
    let result = FeatureCrud::insert(&doc, feature, 2);
    assert!(matches!(
        result,
        Err(FeatureCrudError::InsertBeforeConsumer {
            consumed_ref,
            displaced_feature_id,
            consumer_at: 2,
            requested_at: 2,
            ..
        }) if (consumed_ref == "box_1" || consumed_ref == "box_2")
            && displaced_feature_id == "cut_1"
    ));
}

#[test]
fn t07_degen_self_reference() {
    let doc = Document::new("Test");
    let feature = Feature::Cut {
        id: "cut_self".to_string(),
        target: "cut_self".to_string(),
        tool: "anything".to_string(),
    };
    let result = FeatureCrud::insert(&doc, feature, 0);
    assert!(matches!(
        result,
        Err(FeatureCrudError::SelfReference {
            feature_id,
            ref_kind: "body"
        }) if feature_id == "cut_self"
    ));
}

#[test]
fn t08_boundary_at_zero_no_ref() {
    let doc = Document::new("Test");
    let feature = Feature::CreateBox {
        id: "box_1".to_string(),
        width: 10.0,
        height: 20.0,
        depth: 30.0,
    };
    let result = FeatureCrud::insert(&doc, feature, 0);
    assert!(result.is_ok());
}

#[test]
fn t10_extrude_cut_target_not_found() {
    let doc = Document::new("Test");
    let feature = Feature::ExtrudeCut {
        id: "ec1".to_string(),
        sketch: "unknown".to_string(),
        target: "unknown".to_string(),
        depth: 5.0,
    };
    let result = FeatureCrud::insert(&doc, feature, 0);
    assert!(matches!(
        result,
        Err(FeatureCrudError::SketchNotFound {
            feature_id,
            sketch_ref
        }) if feature_id == "ec1" && sketch_ref == "unknown"
    ));
}

#[test]
fn t11_extrude_cut_body_not_found() {
    let mut doc = Document::new("Test");
    doc.root_component.features.push(Feature::CreateSketch {
        id: "s1".to_string(),
        plane: SketchPlane::Xy,
        offset: 0.0,
        variables: vec![],
        profile: vec![SketchSegment {
            id: "s1".to_string(),
            from: [0.0, 0.0],
            to: [10.0, 0.0],
        }],
        plane_ref: None,
    });
    let feature = Feature::ExtrudeCut {
        id: "ec1".to_string(),
        sketch: "s1".to_string(),
        target: "unknown".to_string(),
        depth: 5.0,
    };
    let result = FeatureCrud::insert(&doc, feature, 1);
    assert!(matches!(
        result,
        Err(FeatureCrudError::BodyNotFound {
            feature_id,
            body_ref
        }) if feature_id == "ec1" && body_ref == "unknown"
    ));
}

#[test]
fn t12_extrude_fuse_target_not_found() {
    let mut doc = Document::new("Test");
    doc.root_component.features.push(Feature::CreateSketch {
        id: "s1".to_string(),
        plane: SketchPlane::Xy,
        offset: 0.0,
        variables: vec![],
        profile: vec![SketchSegment {
            id: "s1".to_string(),
            from: [0.0, 0.0],
            to: [10.0, 0.0],
        }],
        plane_ref: None,
    });
    let feature = Feature::Extrude {
        id: "e1".to_string(),
        sketch: "s1".to_string(),
        depth: 5.0,
        fuse_target: Some("unknown".to_string()),
    };
    let result = FeatureCrud::insert(&doc, feature, 1);
    assert!(matches!(
        result,
        Err(FeatureCrudError::BodyNotFound {
            feature_id,
            body_ref
        }) if feature_id == "e1" && body_ref == "unknown"
    ));
}

#[test]
fn t13_fuse_both_refs_ok() {
    let mut doc = Document::new("Test");
    doc.root_component.features.push(Feature::CreateBox {
        id: "b1".to_string(),
        width: 10.0,
        height: 20.0,
        depth: 30.0,
    });
    doc.root_component.features.push(Feature::CreateBox {
        id: "b2".to_string(),
        width: 5.0,
        height: 5.0,
        depth: 5.0,
    });
    let feature = Feature::Fuse {
        id: "f1".to_string(),
        target: "b1".to_string(),
        tool: "b2".to_string(),
    };
    let result = FeatureCrud::insert(&doc, feature, 2).unwrap();
    assert_eq!(result.root_component.features.len(), 3);
}

#[test]
fn t14_intersect_ref_consumed() {
    let mut doc = Document::new("Test");
    doc.root_component.features.push(Feature::CreateBox {
        id: "b1".to_string(),
        width: 10.0,
        height: 20.0,
        depth: 30.0,
    });
    doc.root_component.features.push(Feature::CreateBox {
        id: "b2".to_string(),
        width: 5.0,
        height: 5.0,
        depth: 5.0,
    });
    doc.root_component.features.push(Feature::Cut {
        id: "c1".to_string(),
        target: "b1".to_string(),
        tool: "b2".to_string(),
    });
    let feature = Feature::Intersect {
        id: "i1".to_string(),
        target: "b1".to_string(),
        tool: "b2".to_string(),
    };
    let result = FeatureCrud::insert(&doc, feature, 3);
    assert!(matches!(
        result,
        Err(FeatureCrudError::BodyNotFound {
            feature_id,
            body_ref
        }) if feature_id == "i1" && (body_ref == "b1" || body_ref == "b2")
    ));
}

#[test]
fn t15_self_reference_sketch() {
    let doc = Document::new("Test");
    let feature = Feature::Extrude {
        id: "x".to_string(),
        sketch: "x".to_string(),
        depth: 5.0,
        fuse_target: None,
    };
    let result = FeatureCrud::insert(&doc, feature, 0);
    assert!(matches!(
        result,
        Err(FeatureCrudError::SelfReference {
            feature_id,
            ref_kind: "sketch"
        }) if feature_id == "x"
    ));
}

#[test]
fn t16_insert_at_tail_full_history() {
    let mut doc = Document::new("Test");
    doc.root_component.features.push(Feature::CreateSketch {
        id: "s1".to_string(),
        plane: SketchPlane::Xy,
        offset: 0.0,
        variables: vec![],
        profile: vec![SketchSegment {
            id: "s1".to_string(),
            from: [0.0, 0.0],
            to: [10.0, 0.0],
        }],
        plane_ref: None,
    });
    doc.root_component.features.push(Feature::CreateBox {
        id: "b1".to_string(),
        width: 10.0,
        height: 20.0,
        depth: 30.0,
    });
    doc.root_component.features.push(Feature::Extrude {
        id: "e1".to_string(),
        sketch: "s1".to_string(),
        depth: 5.0,
        fuse_target: None,
    });
    let feature = Feature::Fuse {
        id: "f1".to_string(),
        target: "b1".to_string(),
        tool: "e1".to_string(),
    };
    let result = FeatureCrud::insert(&doc, feature, 3).unwrap();
    assert_eq!(result.root_component.features.len(), 4);
}

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
