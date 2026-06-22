//! #259 Phase 9: Feature CRUD Reorder — acceptance tests (build layer)
//!
//! Core tests: T01 (determinism), T02 (normal build), T04_BOUNDARY_self, T05_DEG_circular, T06_DEG_unknown_id.

use engawa_build::FeatureCrud;
use engawa_format::{Document, Feature, SketchElement};

/// T01: Determinism — same (doc, feature_id, before_id) → byte-equal YAML.
#[test]
fn t01_reorder_determinism() {
    let mut doc = Document::new("Test");
    doc.root_component.features.push(Feature::CreateBox {
        id: "box_1".to_string(),
        width: 10.0,
        height: 20.0,
        depth: 30.0,
        suppressed: false,
    });
    doc.root_component.features.push(Feature::CreateSphere {
        id: "sphere_1".to_string(),
        radius: 5.0,
        center: [0.0, 0.0, 0.0],
        suppressed: false,
    });
    doc.root_component.features.push(Feature::CreateCylinder {
        id: "cyl_1".to_string(),
        radius: 8.0,
        height: 40.0,
        origin: [0.0, 0.0, 0.0],
        suppressed: false,
    });

    let r1 = FeatureCrud::reorder(&doc, "cyl_1", "sphere_1").unwrap();
    let r2 = FeatureCrud::reorder(&doc, "cyl_1", "sphere_1").unwrap();

    assert_eq!(r1.to_yaml().unwrap(), r2.to_yaml().unwrap());
}

/// T02: Normal build (build layer) — reorder cyl_1 before sphere_1, order becomes [box_1, cyl_1, sphere_1].
#[test]
fn t02_reorder_cyl_before_sphere() {
    let mut doc = Document::new("Test");
    doc.root_component.features.push(Feature::CreateBox {
        id: "box_1".to_string(),
        width: 10.0,
        height: 20.0,
        depth: 30.0,
        suppressed: false,
    });
    doc.root_component.features.push(Feature::CreateSphere {
        id: "sphere_1".to_string(),
        radius: 5.0,
        center: [0.0, 0.0, 0.0],
        suppressed: false,
    });
    doc.root_component.features.push(Feature::CreateCylinder {
        id: "cyl_1".to_string(),
        radius: 8.0,
        height: 40.0,
        origin: [0.0, 0.0, 0.0],
        suppressed: false,
    });

    let updated = FeatureCrud::reorder(&doc, "cyl_1", "sphere_1").unwrap();

    assert_eq!(updated.root_component.features.len(), 3);
    assert_eq!(updated.root_component.features[0].id(), "box_1");
    assert_eq!(updated.root_component.features[1].id(), "cyl_1");
    assert_eq!(updated.root_component.features[2].id(), "sphere_1");
}

/// T04_BOUNDARY_self: reorder("box_1", "box_1") → no-op (byte-equal to original).
#[test]
fn t04_boundary_self_noop() {
    let mut doc = Document::new("Test");
    doc.root_component.features.push(Feature::CreateBox {
        id: "box_1".to_string(),
        width: 10.0,
        height: 20.0,
        depth: 30.0,
        suppressed: false,
    });

    let updated = FeatureCrud::reorder(&doc, "box_1", "box_1").unwrap();

    assert_eq!(doc.to_yaml().unwrap(), updated.to_yaml().unwrap());
}

/// T05_DEG_circular: B depends on A, moving A after B → EditBreaksConsumer.
#[test]
fn t05_deg_circular_producer_after_consumer() {
    let mut doc = Document::new("Test");
    // A: CreateSketch
    doc.root_component.features.push(Feature::CreateSketch {
        id: "sk_a".to_string(),
        plane: engawa_format::SketchPlane::Xy,
        offset: 0.0,
        variables: vec![],
        profile: vec![
            SketchElement::Line {
                id: "seg1".to_string(),
                from: [0.0, 0.0],
                to: [10.0, 0.0],
            },
            SketchElement::Line {
                id: "seg2".to_string(),
                from: [10.0, 0.0],
                to: [10.0, 10.0],
            },
            SketchElement::Line {
                id: "seg3".to_string(),
                from: [10.0, 10.0],
                to: [0.0, 10.0],
            },
            SketchElement::Line {
                id: "seg4".to_string(),
                from: [0.0, 10.0],
                to: [0.0, 0.0],
            },
        ],
        plane_ref: None,
        suppressed: false,
    });
    // B: Extrude that depends on sk_a
    doc.root_component.features.push(Feature::Extrude {
        id: "ext_b".to_string(),
        sketch: "sk_a".to_string(),
        depth: 10.0,
        fuse_target: None,
        suppressed: false,
    });

    // Try to move ext_b before sk_a — should break ext_b
    let result = FeatureCrud::reorder(&doc, "ext_b", "sk_a");
    assert!(matches!(
        result,
        Err(engawa_build::FeatureCrudError::EditBreaksConsumer { .. })
    ));
    if let Err(engawa_build::FeatureCrudError::EditBreaksConsumer {
        broken_consumer_id, ..
    }) = result
    {
        assert_eq!(broken_consumer_id, "ext_b");
    }
}

/// T06_DEG_unknown_id: non-existent feature_id → UnknownFeatureId.
#[test]
fn t06_deg_unknown_feature_id() {
    let mut doc = Document::new("Test");
    doc.root_component.features.push(Feature::CreateBox {
        id: "box_1".to_string(),
        width: 10.0,
        height: 20.0,
        depth: 30.0,
        suppressed: false,
    });

    let result = FeatureCrud::reorder(&doc, "nonexistent", "box_1");
    assert!(matches!(
        result,
        Err(engawa_build::FeatureCrudError::UnknownFeatureId { .. })
    ));
}

/// T06_DEG_unknown_id (before_id variant): non-existent before_id → UnknownFeatureId.
#[test]
fn t06_deg_unknown_before_id() {
    let mut doc = Document::new("Test");
    doc.root_component.features.push(Feature::CreateBox {
        id: "box_1".to_string(),
        width: 10.0,
        height: 20.0,
        depth: 30.0,
        suppressed: false,
    });

    let result = FeatureCrud::reorder(&doc, "box_1", "nonexistent");
    assert!(matches!(
        result,
        Err(engawa_build::FeatureCrudError::UnknownFeatureId { .. })
    ));
}
