//! Acceptance tests for Feature CRUD Suppress (Issue #258).
//!
//! Tests:
//! - T01: Deterministic suppress (byte-equal)
//! - T02: suppress normal (build-side)
//! - T03: restore normal (build-side)
//! - T06: suppress referenced feature → EditBreaksConsumer (deg)
//! - T07: suppress unknown feature → UnknownFeatureId (deg)
//! - T08: suppressed feature is skipped in build

use engawa_build::FeatureCrud;
use engawa_format::{Document, Feature};

/// T01: Deterministic suppress — same (doc, feature_id, true) produces byte-equal YAML.
#[test]
fn t01_suppress_deterministic() {
    let mut doc = Document::new("t01");
    doc.root_component.features.push(Feature::CreateBox {
        id: "box_1".to_string(),
        width: 10.0,
        height: 20.0,
        depth: 30.0,
        suppressed: false,
    });

    let suppressed1 = FeatureCrud::suppress(&doc, "box_1", true).expect("suppress should succeed");
    let yaml1 = suppressed1.to_yaml().expect("to_yaml should succeed");

    let suppressed2 = FeatureCrud::suppress(&doc, "box_1", true).expect("suppress should succeed");
    let yaml2 = suppressed2.to_yaml().expect("to_yaml should succeed");

    assert_eq!(yaml1, yaml2, "suppress must be deterministic");
}

/// T02: suppress normal — is_suppressed() == true after suppress.
#[test]
fn t02_suppress_normal() {
    let mut doc = Document::new("t02");
    doc.root_component.features.push(Feature::CreateBox {
        id: "box_1".to_string(),
        width: 10.0,
        height: 20.0,
        depth: 30.0,
        suppressed: false,
    });

    let suppressed = FeatureCrud::suppress(&doc, "box_1", true).expect("suppress should succeed");

    assert!(
        suppressed.root_component.features[0].is_suppressed(),
        "feature should be suppressed"
    );
    assert_eq!(suppressed.root_component.features[0].id(), "box_1");
}

/// T03: restore normal — suppress(false) clears flag, byte-equal to original.
#[test]
fn t03_restore_normal() {
    let mut doc = Document::new("t03");
    doc.root_component.features.push(Feature::CreateBox {
        id: "box_1".to_string(),
        width: 10.0,
        height: 20.0,
        depth: 30.0,
        suppressed: false,
    });

    let original_yaml = doc.to_yaml().expect("to_yaml should succeed");

    // First suppress
    let suppressed = FeatureCrud::suppress(&doc, "box_1", true).expect("suppress should succeed");
    assert!(suppressed.root_component.features[0].is_suppressed());

    // Then restore
    let restored =
        FeatureCrud::suppress(&suppressed, "box_1", false).expect("restore should succeed");
    assert!(!restored.root_component.features[0].is_suppressed());

    let restored_yaml = restored.to_yaml().expect("to_yaml should succeed");
    assert_eq!(
        original_yaml, restored_yaml,
        "restored document should be byte-equal to original"
    );
}

/// T06: suppressing a feature that is referenced by downstream consumer returns EditBreaksConsumer (deg).
#[test]
fn t06_deg_referenced() {
    let mut doc = Document::new("t_deg_referenced");
    // CreateSketch → Extrude (sketch: sketch_1)
    // Suppressing CreateSketch should break Extrude
    doc.root_component.features.push(Feature::CreateSketch {
        id: "sketch_1".to_string(),
        plane: engawa_format::SketchPlane::Xy,
        offset: 0.0,
        variables: vec![],
        profile: vec![],
        plane_ref: None,
        suppressed: false,
    });
    doc.root_component.features.push(Feature::Extrude {
        id: "ext_1".to_string(),
        sketch: "sketch_1".to_string(),
        depth: 5.0,
        fuse_target: None,
        suppressed: false,
    });

    // Try to suppress sketch_1 → should fail because ext_1 depends on it
    let result = FeatureCrud::suppress(&doc, "sketch_1", true);
    assert!(
        matches!(
            result,
            Err(engawa_build::FeatureCrudError::EditBreaksConsumer { .. })
        ),
        "suppressing a referenced sketch should return EditBreaksConsumer, got: {:?}",
        result
    );
}

/// T07: suppressing a non-existent feature returns UnknownFeatureId (deg).
#[test]
fn t07_deg_unknown_id() {
    let doc = Document::new("t07_deg_unknown_id");

    let result = FeatureCrud::suppress(&doc, "nonexistent", true);
    assert!(
        matches!(
            result,
            Err(engawa_build::FeatureCrudError::UnknownFeatureId { .. })
        ),
        "suppressing unknown feature should return UnknownFeatureId, got: {:?}",
        result
    );
}

/// T08: suppressed feature is skipped in build_bodies_from_features.
#[test]
fn t08_suppress_skipped_in_build() {
    use engawa_kernel::brep::topology::IdGenerator;

    // Create document with box_1 (suppressed=true) and sphere_1 (not suppressed)
    let mut doc = Document::new("t08");
    doc.root_component.features.push(Feature::CreateBox {
        id: "box_1".to_string(),
        width: 10.0,
        height: 20.0,
        depth: 30.0,
        suppressed: true, // suppressed
    });
    doc.root_component.features.push(Feature::CreateSphere {
        id: "sphere_1".to_string(),
        radius: 5.0,
        center: [0.0, 0.0, 0.0],
        suppressed: false, // not suppressed
    });

    // build_bodies_from_features should skip suppressed feature
    let mut gen = IdGenerator::new(0);
    let built =
        engawa_build::build_bodies_from_features(&doc.root_component.features, &[], &mut gen);

    assert!(built.is_ok(), "build should succeed: {:?}", built);
    let built = built.unwrap();

    // Only sphere_1 should be built (box_1 suppressed)
    assert_eq!(built.len(), 1, "should have 1 built body");
    assert!(!built.is_empty(), "should not be empty");

    let all_bodies = built.all();
    assert_eq!(all_bodies.len(), 1, "should have 1 body in all");
    assert_eq!(
        all_bodies[0].feature_id, "sphere_1",
        "sphere_1 should be built"
    );

    // box_1 should not be present
    assert!(
        built.get("box_1").is_none(),
        "box_1 should not be built (suppressed)"
    );
    assert!(built.get("sphere_1").is_some(), "sphere_1 should be built");
}

/// T09: all features suppressed → build_assembly returns empty bodies (C-F01 regression).
#[test]
fn t09_all_suppressed_component_builds_empty() {
    use engawa_kernel::brep::topology::IdGenerator;

    // Create document with all features suppressed
    let mut doc = Document::new("t09");
    doc.root_component.features.push(Feature::CreateBox {
        id: "box_1".to_string(),
        width: 10.0,
        height: 20.0,
        depth: 30.0,
        suppressed: true,
    });
    doc.root_component.features.push(Feature::CreateSphere {
        id: "sphere_1".to_string(),
        radius: 5.0,
        center: [0.0, 0.0, 0.0],
        suppressed: true,
    });

    // build_bodies_from_features should succeed with empty bodies
    let mut gen = IdGenerator::new(0);
    let built =
        engawa_build::build_bodies_from_features(&doc.root_component.features, &[], &mut gen);

    assert!(
        built.is_ok(),
        "build should succeed with all suppressed: {:?}",
        built
    );
    let built = built.unwrap();

    // Should be empty (no bodies built)
    assert!(built.is_empty(), "should have 0 built bodies");
    assert_eq!(built.len(), 0, "len should be 0");
    assert_eq!(built.all().len(), 0, "all() should be empty");
}

/// T10: restore invalid feature (broken refs) → rejected (C-F02 regression).
#[test]
fn t10_restore_invalid_feature_rejected() {
    use engawa_format::{SketchElement, SketchPlane};

    // Create document with CreateSketch and a suppressed Extrude with broken sketch ref.
    // This simulates hand-edited YAML where a suppressed Extrude has an invalid ref.
    let mut doc = Document::new("t10");
    doc.root_component.features.push(Feature::CreateSketch {
        id: "sketch_1".to_string(),
        plane: SketchPlane::Xy,
        offset: 0.0,
        variables: vec![],
        profile: vec![SketchElement::Line {
            id: "seg1".to_string(),
            from: [0.0, 0.0],
            to: [10.0, 0.0],
        }],
        plane_ref: None,
        suppressed: false,
    });
    // Extrude with broken ref (sketch does not exist) and suppressed=true
    doc.root_component.features.push(Feature::Extrude {
        id: "ext_1".to_string(),
        sketch: "nonexistent_sketch".to_string(), // broken ref
        depth: 5.0,
        fuse_target: None,
        suppressed: true, // suppressed (inert)
    });

    // Attempt to restore ext_1 → should fail with SketchNotFound (broken ref detected)
    let result = FeatureCrud::suppress(&doc, "ext_1", false);
    assert!(
        matches!(
            result,
            Err(engawa_build::FeatureCrudError::SketchNotFound { .. })
        ),
        "restoring feature with broken sketch ref should fail with SketchNotFound, got: {:?}",
        result
    );
}

/// T11: body producer suppressed with sketch active → builds empty (M-F01 regression).
#[test]
fn t11_body_producer_suppressed_with_sketch_builds_empty() {
    use engawa_format::{SketchElement, SketchPlane};
    use engawa_kernel::brep::topology::IdGenerator;

    // Create document with CreateSketch (active) and Extrude (suppressed)
    let mut doc = Document::new("t11");
    // Closed triangle profile (3 segments forming a loop)
    let seg1 = SketchElement::Line {
        id: "seg1".to_string(),
        from: [0.0, 0.0],
        to: [10.0, 0.0],
    };
    let seg2 = SketchElement::Line {
        id: "seg2".to_string(),
        from: [10.0, 0.0],
        to: [5.0, 10.0],
    };
    let seg3 = SketchElement::Line {
        id: "seg3".to_string(),
        from: [5.0, 10.0],
        to: [0.0, 0.0], // closes loop
    };
    doc.root_component.features.push(Feature::CreateSketch {
        id: "sketch_1".to_string(),
        plane: SketchPlane::Xy,
        offset: 0.0,
        variables: vec![],
        profile: vec![seg1, seg2, seg3],
        plane_ref: None,
        suppressed: false, // active
    });
    doc.root_component.features.push(Feature::Extrude {
        id: "ext_1".to_string(),
        sketch: "sketch_1".to_string(),
        depth: 5.0,
        fuse_target: None,
        suppressed: true, // suppressed (body producer inert)
    });

    // build_bodies_from_features should succeed with empty bodies
    let mut gen = IdGenerator::new(0);
    let built =
        engawa_build::build_bodies_from_features(&doc.root_component.features, &[], &mut gen);

    assert!(
        built.is_ok(),
        "build should succeed with active sketch but suppressed extrude: {:?}",
        built
    );
    let built = built.unwrap();

    // Should be empty (no bodies built because only body producer is suppressed)
    assert!(built.is_empty(), "should have 0 built bodies");
    assert_eq!(built.len(), 0, "len should be 0");
}
