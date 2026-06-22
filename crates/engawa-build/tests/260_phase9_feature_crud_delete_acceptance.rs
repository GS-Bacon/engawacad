//! #260 Phase 9: Feature CRUD Delete — acceptance tests (build layer)
//!
//! Core tests: T01 (determinism), T02 (normal build), T04_DEG_referenced, T05_DEG_unknown_id, T06_BOUNDARY_last_feature.

use engawa_build::FeatureCrud;
use engawa_format::{Document, Feature, SketchElement};

/// T01: Determinism — same (doc, feature_id) → byte-equal YAML.
#[test]
fn t01_delete_determinism() {
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

    let r1 = FeatureCrud::delete(&doc, "sphere_1").unwrap();
    let r2 = FeatureCrud::delete(&doc, "sphere_1").unwrap();

    assert_eq!(r1.to_yaml().unwrap(), r2.to_yaml().unwrap());
}

/// T02: Normal build (build layer) — [box_1, sphere_1, cyl_1] → delete("sphere_1") → [box_1, cyl_1].
#[test]
fn t02_delete_sphere_from_three_features() {
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

    let updated = FeatureCrud::delete(&doc, "sphere_1").unwrap();

    assert_eq!(updated.root_component.features.len(), 2);
    assert_eq!(updated.root_component.features[0].id(), "box_1");
    assert_eq!(updated.root_component.features[1].id(), "cyl_1");
}

/// T04_DEG_referenced: Extrude references CreateSketch, deleting CreateSketch → EditBreaksConsumer.
#[test]
fn t04_deg_referenced_sketch_breaks_consumer() {
    let mut doc = Document::new("Test");
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
    doc.root_component.features.push(Feature::Extrude {
        id: "ext_b".to_string(),
        sketch: "sk_a".to_string(),
        depth: 10.0,
        fuse_target: None,
        suppressed: false,
    });

    // Try to delete sk_a — should break ext_b
    let result = FeatureCrud::delete(&doc, "sk_a");
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

/// T05_DEG_unknown_id: non-existent feature_id → UnknownFeatureId.
#[test]
fn t05_deg_unknown_feature_id() {
    let mut doc = Document::new("Test");
    doc.root_component.features.push(Feature::CreateBox {
        id: "box_1".to_string(),
        width: 10.0,
        height: 20.0,
        depth: 30.0,
        suppressed: false,
    });

    let result = FeatureCrud::delete(&doc, "nonexistent");
    assert!(matches!(
        result,
        Err(engawa_build::FeatureCrudError::UnknownFeatureId { .. })
    ));
}

/// T06_BOUNDARY_last_feature: 1 feature doc → delete → empty feature list.
#[test]
fn t06_boundary_last_feature_results_in_empty() {
    let mut doc = Document::new("Test");
    doc.root_component.features.push(Feature::CreateBox {
        id: "box_1".to_string(),
        width: 10.0,
        height: 20.0,
        depth: 30.0,
        suppressed: false,
    });

    let updated = FeatureCrud::delete(&doc, "box_1").unwrap();

    assert_eq!(updated.root_component.features.len(), 0);
}
