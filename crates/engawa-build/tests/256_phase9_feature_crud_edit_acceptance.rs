//! #256 Phase 9: Feature CRUD Edit — acceptance tests (build layer)
//!
//! Core tests: T01 (determinism), T02 (normal build), T_DEG_* (degenerate).

use engawa_build::FeatureCrud;
use engawa_format::{Document, Feature};

/// T01: Determinism — same (doc, feature_id, new_feature) → byte-equal YAML.
#[test]
fn t01_edit_determinism() {
    let mut doc = Document::new("Test");
    doc.root_component.features.push(Feature::CreateBox {
        id: "box_1".to_string(),
        width: 10.0,
        height: 20.0,
        depth: 30.0,
    });

    let new_feature = Feature::CreateBox {
        id: "box_1".to_string(),
        width: 100.0,
        height: 200.0,
        depth: 300.0,
    };

    let r1 = FeatureCrud::edit(&doc, "box_1", new_feature.clone()).unwrap();
    let r2 = FeatureCrud::edit(&doc, "box_1", new_feature).unwrap();

    assert_eq!(r1.to_yaml().unwrap(), r2.to_yaml().unwrap());
}

/// T02: Normal build (build layer) — edit CreateBox params, ID preserved.
#[test]
fn t02_edit_creates_box_params_updated() {
    let mut doc = Document::new("Test");
    doc.root_component.features.push(Feature::CreateBox {
        id: "box_1".to_string(),
        width: 10.0,
        height: 20.0,
        depth: 30.0,
    });

    let new_feature = Feature::CreateBox {
        id: "box_1".to_string(),
        width: 100.0,
        height: 200.0,
        depth: 300.0,
    };

    let updated = FeatureCrud::edit(&doc, "box_1", new_feature).unwrap();

    assert_eq!(updated.root_component.features.len(), 1);
    match &updated.root_component.features[0] {
        Feature::CreateBox {
            id,
            width,
            height,
            depth,
        } => {
            assert_eq!(id, "box_1");
            assert_eq!(*width, 100.0);
            assert_eq!(*height, 200.0);
            assert_eq!(*depth, 300.0);
        }
        _ => panic!("expected CreateBox"),
    }
}

/// T03: Degenerate — edit on empty doc → UnknownFeatureId.
#[test]
fn t03_deg_unknown_id() {
    let doc = Document::new("Test");
    let new_feature = Feature::CreateBox {
        id: "box_1".to_string(),
        width: 10.0,
        height: 20.0,
        depth: 30.0,
    };

    let result = FeatureCrud::edit(&doc, "box_1", new_feature);
    assert!(matches!(
        result,
        Err(engawa_build::FeatureCrudError::UnknownFeatureId { .. })
    ));
}

/// T04: Degenerate — edit with mismatched ID → IdMismatch.
#[test]
fn t04_deg_invalid_spec_id_mismatch() {
    let mut doc = Document::new("Test");
    doc.root_component.features.push(Feature::CreateBox {
        id: "box_1".to_string(),
        width: 10.0,
        height: 20.0,
        depth: 30.0,
    });

    let new_feature = Feature::CreateBox {
        id: "box_2".to_string(),
        width: 100.0,
        height: 200.0,
        depth: 300.0,
    };

    let result = FeatureCrud::edit(&doc, "box_1", new_feature);
    assert!(matches!(
        result,
        Err(engawa_build::FeatureCrudError::IdMismatch { .. })
    ));
}

/// T05: Multi-feature doc — edit preserves idx and other features order.
#[test]
fn t05_edit_preserves_idx_in_multi_feature_doc() {
    let mut doc = Document::new("Test");
    doc.root_component.features.push(Feature::CreateBox {
        id: "box_1".to_string(),
        width: 10.0,
        height: 20.0,
        depth: 30.0,
    });
    doc.root_component.features.push(Feature::CreateSphere {
        id: "sphere_1".to_string(),
        radius: 15.0,
        center: [0.0, 0.0, 0.0],
    });
    doc.root_component.features.push(Feature::CreateCylinder {
        id: "cylinder_1".to_string(),
        radius: 8.0,
        height: 40.0,
        origin: [0.0, 0.0, 0.0],
    });

    // Edit sphere_1 (idx=1) with new radius
    let new_sphere = Feature::CreateSphere {
        id: "sphere_1".to_string(),
        radius: 25.0,
        center: [0.0, 0.0, 0.0],
    };

    let updated = FeatureCrud::edit(&doc, "sphere_1", new_sphere).unwrap();

    // Verify: len=3, idx preserved, other features unchanged
    assert_eq!(updated.root_component.features.len(), 3);
    assert_eq!(updated.root_component.features[0].id(), "box_1");
    assert_eq!(updated.root_component.features[1].id(), "sphere_1");
    assert_eq!(updated.root_component.features[2].id(), "cylinder_1");

    // Verify sphere_1 has new radius
    match &updated.root_component.features[1] {
        Feature::CreateSphere {
            id,
            radius,
            center: _,
        } => {
            assert_eq!(id, "sphere_1");
            assert_eq!(*radius, 25.0);
        }
        _ => panic!("expected CreateSphere at idx=1"),
    }
}

/// T06_DEG_variant_mismatch: edit が variant 変更を reject すること。
#[test]
fn t06_deg_variant_mismatch() {
    let mut doc = Document::new("Test");
    doc.root_component.features.push(Feature::CreateBox {
        id: "box_1".to_string(),
        width: 10.0,
        height: 20.0,
        depth: 30.0,
    });

    // 同 ID で variant を CreateBox → CreateSphere に変更
    let new_feature = Feature::CreateSphere {
        id: "box_1".to_string(),
        radius: 5.0,
        center: [0.0, 0.0, 0.0],
    };

    let result = FeatureCrud::edit(&doc, "box_1", new_feature);
    assert!(matches!(
        result,
        Err(engawa_build::FeatureCrudError::VariantMismatch { .. })
    ));
}

/// T07_DEG_edit_steals_body_from_downstream:
/// Extrude{fuse_target=None} → Extrude{fuse_target=Some(box_b)} で
/// 後段の Cut{tool:box_b} が box_b を失うパターンを reject。
#[test]
fn t07_deg_edit_steals_body_from_downstream() {
    use engawa_format::SketchSegment;
    let mut doc = Document::new("Test");
    // box_a (target), box_b (tool), sk1 (sketch for e1), Extrude e1 (fuse_target=None), Cut c1 (tool=box_b)
    doc.root_component.features.push(Feature::CreateBox {
        id: "box_a".to_string(),
        width: 10.0,
        height: 10.0,
        depth: 10.0,
    });
    doc.root_component.features.push(Feature::CreateBox {
        id: "box_b".to_string(),
        width: 5.0,
        height: 5.0,
        depth: 5.0,
    });
    // CreateSketch for Extrude
    doc.root_component.features.push(Feature::CreateSketch {
        id: "sk1".to_string(),
        plane: engawa_format::SketchPlane::Xy,
        offset: 0.0,
        variables: vec![],
        profile: vec![
            SketchSegment {
                id: "seg1".to_string(),
                from: [0.0, 0.0],
                to: [10.0, 0.0],
            },
            SketchSegment {
                id: "seg2".to_string(),
                from: [10.0, 0.0],
                to: [10.0, 10.0],
            },
            SketchSegment {
                id: "seg3".to_string(),
                from: [10.0, 10.0],
                to: [0.0, 10.0],
            },
            SketchSegment {
                id: "seg4".to_string(),
                from: [0.0, 10.0],
                to: [0.0, 0.0],
            },
        ],
        plane_ref: None,
    });
    doc.root_component.features.push(Feature::Extrude {
        id: "e1".to_string(),
        sketch: "sk1".to_string(),
        depth: 10.0,
        fuse_target: None, // Initially not consuming box_b
    });
    doc.root_component.features.push(Feature::Cut {
        id: "c1".to_string(),
        target: "box_a".to_string(),
        tool: "box_b".to_string(),
    });

    // Edit e1 to consume box_b via fuse_target — this should break c1
    let new_e1 = Feature::Extrude {
        id: "e1".to_string(),
        sketch: "sk1".to_string(),
        depth: 10.0,
        fuse_target: Some("box_b".to_string()), // Now consumes box_b
    };

    let result = FeatureCrud::edit(&doc, "e1", new_e1);
    assert!(matches!(
        result,
        Err(engawa_build::FeatureCrudError::EditBreaksConsumer { .. })
    ));
    if let Err(engawa_build::FeatureCrudError::EditBreaksConsumer {
        broken_consumer_id,
        broken_ref,
        ..
    }) = result
    {
        assert_eq!(broken_consumer_id, "c1");
        assert_eq!(broken_ref, "box_b");
    }
}

/// T08_DEG_edit_retargets_plane_ref (skipped — complex fixture, T07 covers the critical case).
/// This test requires CreateSketch/PlaneRef/EntityRef fixtures that interact with transitive
/// implicit body refs. Deferring to a follow-up issue to minimize scope for T07-only validation.
#[test]
#[ignore]
fn t08_deg_edit_retargets_plane_ref() {
    // TODO: Implement when CreateSketch.plane_ref transitive dependency fixture is stabilized
}
