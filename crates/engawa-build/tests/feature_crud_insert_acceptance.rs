//! Acceptance tests for #255 — Feature CRUD Insert (build + cli).

use engawa_build::FeatureCrud;
use engawa_format::{Document, Feature};
use std::path::PathBuf;

fn fixture_path(name: &str) -> PathBuf {
    let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    path.push("tests/fixtures/insert");
    path.push(name);
    path
}

/// T01: Determinism — 同一 (doc, feature, at) を 2 回 insert → to_yaml() byte-identical
#[test]
fn t01_determinism() {
    let doc = Document::new("Test");
    let feature = Feature::CreateBox {
        id: "box_1".to_string(),
        width: 10.0,
        height: 20.0,
        depth: 30.0,
    };

    let result1 = FeatureCrud::insert(&doc, feature.clone(), 0).unwrap();
    let yaml1 = result1.to_yaml().unwrap();

    let result2 = FeatureCrud::insert(&doc, feature, 0).unwrap();
    let yaml2 = result2.to_yaml().unwrap();

    assert_eq!(yaml1, yaml2, "insert must produce byte-identical YAML");
}

/// T02: Normal系 (build) — 既存 1-Feature Document の末尾に Feature 追加
#[test]
fn t02_normal_build_tail_insert() {
    let input_path = fixture_path("input.engawa");
    let doc = Document::from_path(&input_path).expect("failed to load input");

    let feature_yaml =
        std::fs::read_to_string(fixture_path("new_box.yaml")).expect("failed to read feature YAML");
    let feature: Feature = serde_yaml::from_str(&feature_yaml).expect("failed to parse feature");

    let result = FeatureCrud::insert(&doc, feature, 1).expect("insert failed");

    assert_eq!(result.root_component.features.len(), 2);
    assert_eq!(result.root_component.features[0].id(), "box_1");
    assert_eq!(result.root_component.features[1].id(), "box_2");
}

/// T_BOUNDARY_at_zero: 先頭挿入
#[test]
fn t_boundary_at_zero() {
    let input_path = fixture_path("input.engawa");
    let doc = Document::from_path(&input_path).expect("failed to load input");

    let feature: Feature = Feature::CreateSphere {
        id: "sphere_1".to_string(),
        radius: 5.0,
        center: [0.0, 0.0, 0.0],
    };

    let result = FeatureCrud::insert(&doc, feature, 0).expect("insert at head failed");

    assert_eq!(result.root_component.features.len(), 2);
    assert_eq!(result.root_component.features[0].id(), "sphere_1");
    assert_eq!(result.root_component.features[1].id(), "box_1");
}

/// T_BOUNDARY_at_end: 末尾挿入 (at == len)
#[test]
fn t_boundary_at_end() {
    let input_path = fixture_path("input.engawa");
    let doc = Document::from_path(&input_path).expect("failed to load input");

    let len = doc.root_component.features.len();
    let feature: Feature = Feature::CreateSphere {
        id: "sphere_1".to_string(),
        radius: 5.0,
        center: [0.0, 0.0, 0.0],
    };

    let result = FeatureCrud::insert(&doc, feature, len).expect("insert at end failed");

    assert_eq!(result.root_component.features.len(), 2);
    assert_eq!(result.root_component.features[1].id(), "sphere_1");
}

/// T_BOUNDARY_at_end: 空の Document で at=0
#[test]
fn t_boundary_at_end_empty_document() {
    let doc = Document::new("Empty");

    let feature: Feature = Feature::CreateBox {
        id: "box_1".to_string(),
        width: 10.0,
        height: 20.0,
        depth: 30.0,
    };

    let result = FeatureCrud::insert(&doc, feature, 0).expect("insert into empty failed");

    assert_eq!(result.root_component.features.len(), 1);
    assert_eq!(result.root_component.features[0].id(), "box_1");
}

/// T_DEG_at_out_of_range: at > len → OutOfRange エラー
#[test]
fn t_deg_at_out_of_range() {
    let input_path = fixture_path("input.engawa");
    let doc = Document::from_path(&input_path).expect("failed to load input");

    let len = doc.root_component.features.len();
    let feature: Feature = Feature::CreateBox {
        id: "box_2".to_string(),
        width: 5.0,
        height: 15.0,
        depth: 25.0,
    };

    let result = FeatureCrud::insert(&doc, feature, len + 1);

    match result {
        Err(engawa_build::FeatureCrudError::OutOfRange { index, len: l }) => {
            assert_eq!(index, len + 1);
            assert_eq!(l, len);
        }
        other => panic!("expected OutOfRange, got {:?}", other),
    }
}

/// T_DEG_duplicate_id: 重複 id → DuplicateFeatureId エラー
#[test]
fn t_deg_duplicate_id() {
    let input_path = fixture_path("input.engawa");
    let doc = Document::from_path(&input_path).expect("failed to load input");

    let feature: Feature = Feature::CreateBox {
        id: "box_1".to_string(),
        width: 5.0,
        height: 15.0,
        depth: 25.0,
    };

    let result = FeatureCrud::insert(&doc, feature, 1);

    match result {
        Err(engawa_build::FeatureCrudError::DuplicateFeatureId { id }) => {
            assert_eq!(id, "box_1");
        }
        other => panic!("expected DuplicateFeatureId, got {:?}", other),
    }
}

/// T06_insert_then_to_yaml_roundtrip — Insert後のDocumentをto_yaml()→from_yaml()で完全復元
#[test]
fn t06_insert_then_to_yaml_roundtrip() {
    let doc = Document::new("RoundtripTest");
    let feature = Feature::CreateBox {
        id: "box_1".to_string(),
        width: 10.0,
        height: 20.0,
        depth: 30.0,
    };

    let inserted = FeatureCrud::insert(&doc, feature.clone(), 0).unwrap();

    // to_yaml → from_yaml ラウンドトリップ
    let yaml = inserted.to_yaml().expect("failed to serialize");
    let restored = Document::from_yaml(&yaml).expect("failed to deserialize");

    // 全フィールドが一致することを検証
    assert_eq!(restored.schema_version, inserted.schema_version);
    assert_eq!(restored.version, inserted.version);
    assert_eq!(restored.root_component.name, inserted.root_component.name);
    assert_eq!(
        restored.root_component.features.len(),
        inserted.root_component.features.len()
    );

    // Featureの内容も一致
    let restored_feat = &restored.root_component.features[0];
    let inserted_feat = &inserted.root_component.features[0];
    assert_eq!(restored_feat.id(), inserted_feat.id());

    match (restored_feat, inserted_feat) {
        (
            Feature::CreateBox {
                id: i1,
                width: w1,
                height: h1,
                depth: d1,
            },
            Feature::CreateBox {
                id: i2,
                width: w2,
                height: h2,
                depth: d2,
            },
        ) => {
            assert_eq!(i1, i2);
            assert_eq!(w1, w2);
            assert_eq!(h1, h2);
            assert_eq!(d1, d2);
        }
        _ => panic!("feature type mismatch after roundtrip"),
    }
}
