use engawa_format::{Document, Feature};

const M5X20_YAML: &str = include_str!("../../../stdlib/fasteners/jis_b1176/M5x20.engawa");

#[test]
fn t01_parse_ok() {
    let doc: Document = serde_yaml::from_str(M5X20_YAML).expect("YAML parse failed");
    assert_eq!(doc.root_component.features.len(), 2);
}

#[test]
fn t02_shaft_cylinder_params() {
    let doc: Document = serde_yaml::from_str(M5X20_YAML).expect("YAML parse failed");
    let shaft = &doc.root_component.features[0];
    match shaft {
        Feature::CreateCylinder {
            id,
            radius,
            height,
            origin: _,
            suppressed: _,
        } => {
            assert_eq!(id, "shaft");
            assert_eq!(*radius, 2.5);
            assert_eq!(*height, 20.0);
        }
        other => panic!("expected CreateCylinder, got {:?}", other),
    }
}

#[test]
fn t03_boundary_head_origin() {
    let doc: Document = serde_yaml::from_str(M5X20_YAML).expect("YAML parse failed");
    let head = &doc.root_component.features[1];
    match head {
        Feature::CreateCylinder {
            id,
            radius,
            height,
            origin,
            suppressed: _,
        } => {
            assert_eq!(id, "head");
            assert_eq!(*radius, 4.5);
            assert_eq!(*height, 3.0);
            assert_eq!(*origin, [0.0, 20.0, 0.0]);
        }
        other => panic!("expected CreateCylinder, got {:?}", other),
    }
}
