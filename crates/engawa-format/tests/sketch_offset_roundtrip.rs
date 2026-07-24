//! T05: SketchOffset YAML roundtrip integration test.
//!
//! Verifies `Feature::SketchOffset` serialize → deserialize preserves all fields.

use engawa_format::Feature;

#[test]
fn t05_sketch_offset_yaml_roundtrip() {
    let original = Feature::SketchOffset {
        id: "off_1".to_string(),
        sketch: "sketch_a".to_string(),
        selection: vec!["l1".to_string(), "c1".to_string()],
        distance: 1.75,
        suppressed: false,
    };

    let yaml = serde_yaml::to_string(&original).expect("serialize SketchOffset");
    let back: Feature = serde_yaml::from_str(&yaml).expect("deserialize SketchOffset");

    match (&original, &back) {
        (
            Feature::SketchOffset {
                id: id1,
                sketch: s1,
                selection: sel1,
                distance: d1,
                suppressed: sup1,
            },
            Feature::SketchOffset {
                id: id2,
                sketch: s2,
                selection: sel2,
                distance: d2,
                suppressed: sup2,
            },
        ) => {
            assert_eq!(id1, id2);
            assert_eq!(s1, s2);
            assert_eq!(sel1, sel2);
            assert_eq!(d1, d2);
            assert_eq!(sup1, sup2);
        }
        _ => panic!("SketchOffset did not roundtrip as same variant"),
    }
}

#[test]
fn t05_sketch_offset_empty_selection_roundtrip() {
    let original = Feature::SketchOffset {
        id: "off_2".to_string(),
        sketch: "sketch_a".to_string(),
        selection: vec![],
        distance: -0.5,
        suppressed: true,
    };

    let yaml = serde_yaml::to_string(&original).expect("serialize");
    let back: Feature = serde_yaml::from_str(&yaml).expect("deserialize");

    match back {
        Feature::SketchOffset {
            selection,
            distance,
            suppressed,
            ..
        } => {
            assert!(selection.is_empty(), "empty selection preserved");
            assert_eq!(distance, -0.5);
            assert!(suppressed);
        }
        _ => panic!("wrong variant"),
    }
}
