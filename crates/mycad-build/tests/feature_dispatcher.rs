use mycad_build::build_solid_from_features;
use mycad_format::Document;
use mycad_kernel::brep::topology::IdGenerator;
use std::path::Path;

#[test]
fn simple_box_mycad_builds_solid() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("examples")
        .join("simple_box.mycad");

    let doc = Document::from_path(&path).expect("load .mycad");
    let mut g = IdGenerator::new(0);
    let solid =
        build_solid_from_features(&doc.root_component.features, &mut g).expect("build solid");

    assert_eq!(solid.vertices.len(), 8);
    assert_eq!(solid.edges.len(), 12);
    assert_eq!(solid.faces.len(), 6);
    assert_eq!(solid.shells.len(), 1);
}

/// T11: Integration — sphere.mycad → build → topology V=2, E=1, F=1.
#[test]
fn sphere_mycad_builds_solid() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("examples")
        .join("sphere.mycad");

    let doc = Document::from_path(&path).expect("load sphere.mycad");
    let mut g = IdGenerator::new(0);
    let solid =
        build_solid_from_features(&doc.root_component.features, &mut g).expect("build sphere");

    assert_eq!(solid.vertices.len(), 2, "2 poles");
    assert_eq!(solid.edges.len(), 1, "1 seam");
    assert_eq!(solid.faces.len(), 1, "1 face");
    assert_eq!(solid.shells.len(), 1);
}

// T09: Build integration — extruded_rect.mycad → build → V=8, E=12, F=6
#[test]
fn extruded_rect_mycad_builds_solid() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("examples")
        .join("extruded_rect.mycad");

    let doc = Document::from_path(&path).expect("load extruded_rect.mycad");
    let mut g = IdGenerator::new(0);
    let solid =
        build_solid_from_features(&doc.root_component.features, &mut g).expect("build extrude");

    assert_eq!(solid.vertices.len(), 8, "V=8");
    assert_eq!(solid.edges.len(), 12, "E=12");
    assert_eq!(solid.faces.len(), 6, "F=6");
    assert_eq!(solid.shells.len(), 1);
}

// T10: Build error — sketch not found
#[test]
fn sketch_not_found() {
    use mycad_format::feature::{Feature, SketchPlane, SketchSegment};
    let features = vec![Feature::Extrude {
        id: "ext_1".to_string(),
        sketch: "nonexistent".to_string(),
        depth: 5.0,
    }];
    let mut g = IdGenerator::new(0);
    let result = build_solid_from_features(&features, &mut g);
    assert!(result.is_err());
    let err = format!("{}", result.unwrap_err());
    assert!(
        err.contains("sketch not found"),
        "expected SketchNotFound, got: {err}"
    );
}

// T10: Build error — multiple solid features
#[test]
fn multiple_solid_features() {
    use mycad_format::Feature;
    let features = vec![
        Feature::CreateBox {
            id: "b1".to_string(),
            width: 1.0,
            height: 1.0,
            depth: 1.0,
        },
        Feature::CreateBox {
            id: "b2".to_string(),
            width: 2.0,
            height: 2.0,
            depth: 2.0,
        },
    ];
    let mut g = IdGenerator::new(0);
    let result = build_solid_from_features(&features, &mut g);
    assert!(result.is_err());
    let err = format!("{}", result.unwrap_err());
    assert!(
        err.contains("multiple features"),
        "expected MultipleFeatures, got: {err}"
    );
}

// T10: Build error — duplicate segment id
#[test]
fn duplicate_segment_id() {
    use mycad_format::feature::{Feature, SketchPlane, SketchSegment};
    let features = vec![Feature::CreateSketch {
        id: "sketch_1".to_string(),
        plane: SketchPlane::Xy,
        profile: vec![
            SketchSegment {
                id: "seg_a".to_string(),
                from: [0.0, 0.0],
                to: [1.0, 0.0],
            },
            SketchSegment {
                id: "seg_a".to_string(),
                from: [1.0, 0.0],
                to: [1.0, 1.0],
            },
            SketchSegment {
                id: "seg_b".to_string(),
                from: [1.0, 1.0],
                to: [0.0, 0.0],
            },
        ],
    }];
    let mut g = IdGenerator::new(0);
    let result = build_solid_from_features(&features, &mut g);
    assert!(result.is_err());
    let err = format!("{}", result.unwrap_err());
    assert!(
        err.contains("sketch_segment_id"),
        "expected segment id error, got: {err}"
    );
}

// T14: Duplicate feature id rejected
#[test]
fn duplicate_feature_id() {
    use mycad_format::feature::{Feature, SketchPlane, SketchSegment};
    let features = vec![
        Feature::CreateSketch {
            id: "sketch_1".to_string(),
            plane: SketchPlane::Xy,
            profile: vec![
                SketchSegment {
                    id: "seg_a".to_string(),
                    from: [0.0, 0.0],
                    to: [1.0, 0.0],
                },
                SketchSegment {
                    id: "seg_b".to_string(),
                    from: [1.0, 0.0],
                    to: [0.0, 0.0],
                },
            ],
        },
        Feature::CreateSketch {
            id: "sketch_1".to_string(),
            plane: SketchPlane::Xy,
            profile: vec![
                SketchSegment {
                    id: "seg_c".to_string(),
                    from: [0.0, 0.0],
                    to: [2.0, 0.0],
                },
                SketchSegment {
                    id: "seg_d".to_string(),
                    from: [2.0, 0.0],
                    to: [0.0, 0.0],
                },
            ],
        },
    ];
    let mut g = IdGenerator::new(0);
    let result = build_solid_from_features(&features, &mut g);
    assert!(result.is_err());
    let err = format!("{}", result.unwrap_err());
    assert!(
        err.contains("duplicate feature id"),
        "expected DuplicateFeatureId, got: {err}"
    );
}

// T16: Forward reference prohibited
#[test]
fn forward_reference_prohibited() {
    use mycad_format::feature::{Feature, SketchPlane, SketchSegment};
    let features = vec![
        Feature::Extrude {
            id: "ext_1".to_string(),
            sketch: "sketch_1".to_string(),
            depth: 5.0,
        },
        Feature::CreateSketch {
            id: "sketch_1".to_string(),
            plane: SketchPlane::Xy,
            profile: vec![
                SketchSegment {
                    id: "seg_a".to_string(),
                    from: [0.0, 0.0],
                    to: [1.0, 0.0],
                },
                SketchSegment {
                    id: "seg_b".to_string(),
                    from: [1.0, 0.0],
                    to: [0.0, 1.0],
                },
                SketchSegment {
                    id: "seg_c".to_string(),
                    from: [0.0, 1.0],
                    to: [0.0, 0.0],
                },
            ],
        },
    ];
    let mut g = IdGenerator::new(0);
    let result = build_solid_from_features(&features, &mut g);
    assert!(result.is_err());
    let err = format!("{}", result.unwrap_err());
    assert!(
        err.contains("sketch not found"),
        "expected SketchNotFound for forward reference, got: {err}"
    );
}

// T11: Regression — cylinder still works
#[test]
fn cylinder_mycad_builds_solid() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("examples")
        .join("cylinder.mycad");

    let doc = Document::from_path(&path).expect("load cylinder.mycad");
    let mut g = IdGenerator::new(0);
    let solid =
        build_solid_from_features(&doc.root_component.features, &mut g).expect("build cylinder");

    assert_eq!(solid.shells.len(), 1);
}
