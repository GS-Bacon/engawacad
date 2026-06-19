//! FeatureCrud::rollback acceptance tests.
//!
//! Core test set for #257 Phase 9 Feature CRUD Rollback.
//! Covers determinism, normal cases, degenerate cases, and boundary conditions.

use engawa_build::FeatureCrud;
use engawa_format::{Document, Feature};

#[test]
fn t01_determinism() {
    // 同一 (doc, feature_id) を 2 回 rollback → 結果 Document の to_yaml() byte-equal
    let mut doc = Document::new("test");
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
        radius: 3.0,
        height: 15.0,
        origin: [0.0, 0.0, 0.0],
        suppressed: false,
    });

    let r1 = FeatureCrud::rollback(&doc, "sphere_1").unwrap();
    let r2 = FeatureCrud::rollback(&doc, "sphere_1").unwrap();

    assert_eq!(r1.to_yaml().unwrap(), r2.to_yaml().unwrap());
}

#[test]
fn t02_normal_rollback_middle() {
    // 3 feature doc [box_1, sphere_1, cyl_1] → rollback("sphere_1") → 結果 features == [box_1]
    let mut doc = Document::new("test");
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
        radius: 3.0,
        height: 15.0,
        origin: [0.0, 0.0, 0.0],
        suppressed: false,
    });

    let result = FeatureCrud::rollback(&doc, "sphere_1").unwrap();

    assert_eq!(result.root_component.features.len(), 1);
    assert_eq!(result.root_component.features[0].id(), "box_1");
}

#[test]
fn t04_deg_unknown_id() {
    // 空 doc に対して rollback("box_1") → UnknownFeatureId エラー
    let doc = Document::new("test");
    let result = FeatureCrud::rollback(&doc, "box_1");
    assert!(matches!(
        result,
        Err(engawa_build::FeatureCrudError::UnknownFeatureId { .. })
    ));
}

#[test]
fn t05_boundary_first_feature() {
    // 3 feature doc → rollback("box_1") (先頭) → 結果 features 列が空 (len() == 0)
    let mut doc = Document::new("test");
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
        radius: 3.0,
        height: 15.0,
        origin: [0.0, 0.0, 0.0],
        suppressed: false,
    });

    let result = FeatureCrud::rollback(&doc, "box_1").unwrap();

    assert_eq!(result.root_component.features.len(), 0);
}

#[test]
fn t06_rollback_tail() {
    // 3 feature doc [box_1, sphere_1, cyl_1] → rollback("cyl_1") (末尾)
    // → 結果 features == [box_1, sphere_1] (cyl_1 のみ削除)
    let mut doc = Document::new("test");
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
        radius: 3.0,
        height: 15.0,
        origin: [0.0, 0.0, 0.0],
        suppressed: false,
    });

    let result = FeatureCrud::rollback(&doc, "cyl_1").unwrap();

    assert_eq!(result.root_component.features.len(), 2);
    assert_eq!(result.root_component.features[0].id(), "box_1");
    assert_eq!(result.root_component.features[1].id(), "sphere_1");
}
