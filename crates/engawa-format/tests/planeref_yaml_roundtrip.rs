//! Issue #215: PlaneRef YAML roundtrip tests

use engawa_format::Document;

/// T05: YAML roundtrip legacy — examples/sketch_via_refplane.engawa を roundtrip
#[test]
fn t05_yaml_roundtrip_sketch_via_refplane_example() {
    let yaml = include_str!("../../../examples/sketch_via_refplane.engawa");
    let doc1: Document = serde_yaml::from_str(yaml).expect("first parse failed");
    let yaml2 = serde_yaml::to_string(&doc1).expect("first serialize failed");
    let doc2: Document = serde_yaml::from_str(&yaml2).expect("second parse failed");

    // 再 serialize の YAML は安定 (2 回目以降は同一)
    let yaml3 = serde_yaml::to_string(&doc2).expect("second serialize failed");
    assert_eq!(yaml2, yaml3, "roundtrip after first cycle = stable");

    // CreateSketch.plane_ref が PlaneRef::RefPlane variant であること
    let root_features = &doc1.root_component.features;
    let plane_ref = root_features.iter().find_map(|f| match f {
        engawa_format::Feature::CreateSketch { plane_ref, .. } => plane_ref.clone(),
        _ => None,
    });
    let plane_ref = plane_ref.expect("plane_ref がある");
    match plane_ref {
        engawa_format::PlaneRef::RefPlane(s) => assert_eq!(s, "Front"),
        engawa_format::PlaneRef::Entity(_) => {
            panic!("legacy 形式は PlaneRef::RefPlane variant")
        }
    }
}

/// T06: YAML roundtrip Entity — examples/sketch_via_face_entity_ref.engawa を roundtrip
#[test]
fn t06_yaml_roundtrip_sketch_via_face_entity_ref_example() {
    let yaml = include_str!("../../../examples/sketch_via_face_entity_ref.engawa");
    let doc1: Document = serde_yaml::from_str(yaml).expect("first parse failed");
    let yaml2 = serde_yaml::to_string(&doc1).expect("first serialize failed");
    let doc2: Document = serde_yaml::from_str(&yaml2).expect("second parse failed");

    // 再 serialize の YAML には "plane_ref:" の下に "ref:" "feature_id:" "kind:" "role:" が含まれる
    assert!(
        yaml2.contains("ref:"),
        "EntityRef map 形式には 'ref:' が含まれる"
    );
    assert!(
        yaml2.contains("feature_id:"),
        "EntityRef map 形式には 'feature_id:' が含まれる"
    );
    assert!(
        yaml2.contains("kind:"),
        "EntityRef map 形式には 'kind:' が含まれる"
    );
    assert!(
        yaml2.contains("role:"),
        "EntityRef map 形式には 'role:' が含まれる"
    );

    // legacy string 形式ではないこと (feature_id: "cuboid" 単独ではなく map 形式)
    assert!(
        !yaml2.contains("plane_ref: cuboid\n"),
        "単純 string 形式ではない"
    );

    // roundtrip 安定性
    let yaml3 = serde_yaml::to_string(&doc2).expect("second serialize failed");
    assert_eq!(yaml2, yaml3, "roundtrip after first cycle = stable");

    // CreateSketch.plane_ref が PlaneRef::Entity variant であること
    let root_features = &doc1.root_component.features;
    let plane_ref = root_features.iter().find_map(|f| match f {
        engawa_format::Feature::CreateSketch { plane_ref, .. } => plane_ref.clone(),
        _ => None,
    });
    let plane_ref = plane_ref.expect("plane_ref がある");
    match plane_ref {
        engawa_format::PlaneRef::Entity(_) => {
            // OK: EntityRef variant
        }
        engawa_format::PlaneRef::RefPlane(_) => {
            panic!("EntityRef 形式は PlaneRef::Entity variant")
        }
    }
}
