use mycad_build::build_bodies_from_features;
use mycad_format::Document;
use mycad_kernel::brep::topology::IdGenerator;
use std::path::Path;

// --- Helper: field-by-field Solid comparison for determinism tests ---

fn assert_solids_equal(
    a: &mycad_kernel::brep::topology::Solid,
    b: &mycad_kernel::brep::topology::Solid,
) {
    assert_eq!(a.id, b.id, "solid id mismatch");

    assert_eq!(a.vertices.len(), b.vertices.len(), "vertex count");
    for (i, (va, vb)) in a.vertices.iter().zip(b.vertices.iter()).enumerate() {
        assert_eq!(va.id, vb.id, "vertex {i} id");
        assert!((va.point.x - vb.point.x).abs() < 1e-12, "vertex {i} x");
        assert!((va.point.y - vb.point.y).abs() < 1e-12, "vertex {i} y");
        assert!((va.point.z - vb.point.z).abs() < 1e-12, "vertex {i} z");
    }

    assert_eq!(a.edges.len(), b.edges.len(), "edge count");
    for (i, (ea, eb)) in a.edges.iter().zip(b.edges.iter()).enumerate() {
        assert_eq!(ea.id, eb.id, "edge {i} id");
        assert_eq!(ea.vertices, eb.vertices, "edge {i} vertices");
        assert_eq!(ea.t_range, eb.t_range, "edge {i} t_range");
    }

    assert_eq!(a.half_edges.len(), b.half_edges.len(), "half_edge count");
    for (i, (ha, hb)) in a.half_edges.iter().zip(b.half_edges.iter()).enumerate() {
        assert_eq!(ha.id, hb.id, "half_edge {i} id");
        assert_eq!(
            ha.start_vertex, hb.start_vertex,
            "half_edge {i} start_vertex"
        );
        assert_eq!(ha.edge, hb.edge, "half_edge {i} edge");
        assert_eq!(ha.forward, hb.forward, "half_edge {i} forward");
    }

    assert_eq!(a.loops.len(), b.loops.len(), "loop count");
    for (i, (la, lb)) in a.loops.iter().zip(b.loops.iter()).enumerate() {
        assert_eq!(la.id, lb.id, "loop {i} id");
        assert_eq!(la.half_edges, lb.half_edges, "loop {i} half_edges");
    }

    assert_eq!(a.faces.len(), b.faces.len(), "face count");
    for (i, (fa, fb)) in a.faces.iter().zip(b.faces.iter()).enumerate() {
        assert_eq!(fa.id, fb.id, "face {i} id");
        assert_eq!(fa.outer_loop, fb.outer_loop, "face {i} outer_loop");
        assert_eq!(fa.inner_loops, fb.inner_loops, "face {i} inner_loops");
        assert_eq!(fa.same_sense, fb.same_sense, "face {i} same_sense");
    }

    assert_eq!(a.shells.len(), b.shells.len(), "shell count");
    for (i, (sa, sb)) in a.shells.iter().zip(b.shells.iter()).enumerate() {
        assert_eq!(sa.id, sb.id, "shell {i} id");
        assert_eq!(sa.faces, sb.faces, "shell {i} faces");
        assert_eq!(sa.closed, sb.closed, "shell {i} closed");
    }
}

// --- T01: Determinism — 2-body document built twice, all topology identical ---

#[test]
fn t01_determinism_two_bodies() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("examples")
        .join("two_bodies.mycad");

    let doc = Document::from_path(&path).expect("load two_bodies.mycad");

    let mut g1 = IdGenerator::new(0);
    let mut g2 = IdGenerator::new(0);
    let b1 = build_bodies_from_features(&doc.root_component.features, &mut g1).expect("build 1");
    let b2 = build_bodies_from_features(&doc.root_component.features, &mut g2).expect("build 2");

    assert_eq!(b1.len(), b2.len(), "body count");
    for (body_a, body_b) in b1.all().iter().zip(b2.all().iter()) {
        assert_eq!(body_a.feature_id, body_b.feature_id, "feature_id");
        assert_solids_equal(&body_a.solid, &body_b.solid);
    }
}

// --- T02: Normal — box+cylinder build → len==2, get works, order correct ---

#[test]
fn t02_multi_create_box_cylinder() {
    use mycad_format::Feature;
    let features = vec![
        Feature::CreateBox {
            id: "box1".to_string(),
            width: 1.0,
            height: 2.0,
            depth: 3.0,
        },
        Feature::CreateCylinder {
            id: "cyl1".to_string(),
            radius: 5.0,
            height: 10.0,
        },
    ];
    let mut g = IdGenerator::new(0);
    let bodies = build_bodies_from_features(&features, &mut g).expect("build");

    assert_eq!(bodies.len(), 2);
    assert!(bodies.get("box1").is_some());
    assert!(bodies.get("cyl1").is_some());

    let box_body = bodies.get("box1").unwrap();
    assert_eq!(box_body.solid.vertices.len(), 8);
    assert_eq!(box_body.solid.edges.len(), 12);
    assert_eq!(box_body.solid.faces.len(), 6);
    assert_eq!(box_body.solid.shells.len(), 1);

    let cyl_body = bodies.get("cyl1").unwrap();
    assert_eq!(cyl_body.solid.shells.len(), 1);

    // all() order matches feature order
    let all = bodies.all();
    assert_eq!(all[0].feature_id, "box1");
    assert_eq!(all[1].feature_id, "cyl1");
}

// --- T03: Regression — single body cases with new API ---

#[test]
fn t03_simple_box_regression() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("examples")
        .join("simple_box.mycad");

    let doc = Document::from_path(&path).expect("load .mycad");
    let mut g = IdGenerator::new(0);
    let bodies =
        build_bodies_from_features(&doc.root_component.features, &mut g).expect("build solid");

    assert_eq!(bodies.len(), 1);
    let solid = &bodies.all()[0].solid;
    assert_eq!(solid.vertices.len(), 8);
    assert_eq!(solid.edges.len(), 12);
    assert_eq!(solid.faces.len(), 6);
    assert_eq!(solid.shells.len(), 1);
}

#[test]
fn t03_sphere_regression() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("examples")
        .join("sphere.mycad");

    let doc = Document::from_path(&path).expect("load sphere.mycad");
    let mut g = IdGenerator::new(0);
    let bodies =
        build_bodies_from_features(&doc.root_component.features, &mut g).expect("build sphere");

    assert_eq!(bodies.len(), 1);
    let solid = &bodies.all()[0].solid;
    assert_eq!(solid.vertices.len(), 2, "2 poles");
    assert_eq!(solid.edges.len(), 1, "1 seam");
    assert_eq!(solid.faces.len(), 1, "1 face");
    assert_eq!(solid.shells.len(), 1);
}

#[test]
fn t03_extruded_rect_regression() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("examples")
        .join("extruded_rect.mycad");

    let doc = Document::from_path(&path).expect("load extruded_rect.mycad");
    let mut g = IdGenerator::new(0);
    let bodies =
        build_bodies_from_features(&doc.root_component.features, &mut g).expect("build extrude");

    assert_eq!(bodies.len(), 1);
    let solid = &bodies.all()[0].solid;
    assert_eq!(solid.vertices.len(), 8, "V=8");
    assert_eq!(solid.edges.len(), 12, "E=12");
    assert_eq!(solid.faces.len(), 6, "F=6");
    assert_eq!(solid.shells.len(), 1);
}

#[test]
fn t03_cylinder_regression() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("examples")
        .join("cylinder.mycad");

    let doc = Document::from_path(&path).expect("load cylinder.mycad");
    let mut g = IdGenerator::new(0);
    let bodies =
        build_bodies_from_features(&doc.root_component.features, &mut g).expect("build cylinder");

    assert_eq!(bodies.len(), 1);
    assert_eq!(bodies.all()[0].solid.shells.len(), 1);
}

// --- T04: Reference error — Cut references missing body → BodyNotFound ---

#[test]
fn t04_cut_missing_target() {
    use mycad_format::Feature;
    let features = vec![
        Feature::CreateBox {
            id: "box1".to_string(),
            width: 1.0,
            height: 1.0,
            depth: 1.0,
        },
        Feature::Cut {
            id: "cut1".to_string(),
            target: "missing".to_string(),
            tool: "box1".to_string(),
        },
    ];
    let mut g = IdGenerator::new(0);
    let result = build_bodies_from_features(&features, &mut g);
    assert!(matches!(
        result,
        Err(mycad_kernel::error::KernelError::BodyNotFound { id }) if id == "missing"
    ));
}

#[test]
fn t04_cut_missing_tool() {
    use mycad_format::Feature;
    let features = vec![
        Feature::CreateBox {
            id: "box1".to_string(),
            width: 1.0,
            height: 1.0,
            depth: 1.0,
        },
        Feature::Cut {
            id: "cut1".to_string(),
            target: "box1".to_string(),
            tool: "nonexistent".to_string(),
        },
    ];
    let mut g = IdGenerator::new(0);
    let result = build_bodies_from_features(&features, &mut g);
    assert!(matches!(
        result,
        Err(mycad_kernel::error::KernelError::BodyNotFound { id }) if id == "nonexistent"
    ));
}

// --- T05: Reference resolved but unsupported → UnsupportedFeature ---

#[test]
fn t05_cut_now_supported() {
    use mycad_format::Feature;
    let features = vec![
        Feature::CreateBox {
            id: "box1".to_string(),
            width: 1.0,
            height: 1.0,
            depth: 1.0,
        },
        Feature::CreateBox {
            id: "box2".to_string(),
            width: 2.0,
            height: 2.0,
            depth: 2.0,
        },
        Feature::Cut {
            id: "cut1".to_string(),
            target: "box1".to_string(),
            tool: "box2".to_string(),
        },
    ];
    let mut g = IdGenerator::new(0);
    let result = build_bodies_from_features(&features, &mut g);
    // box1 (1x1x1) is fully inside box2 (2x2x2), so Cut produces empty result
    assert!(
        matches!(
            result,
            Err(mycad_kernel::error::KernelError::EmptyBooleanResult)
        ),
        "expected EmptyBooleanResult, got {:?}",
        result
    );
}

#[test]
fn t05_fuse_disjoint_boxes() {
    use mycad_format::Feature;
    // Two extruded rectangles far apart → DisjointFuseResult
    let features = vec![
        Feature::CreateSketch {
            id: "sketch1".to_string(),
            plane: mycad_format::SketchPlane::Xy,
            profile: vec![
                mycad_format::SketchSegment {
                    id: "s1".to_string(),
                    from: [0.0, 0.0],
                    to: [1.0, 0.0],
                },
                mycad_format::SketchSegment {
                    id: "s2".to_string(),
                    from: [1.0, 0.0],
                    to: [1.0, 1.0],
                },
                mycad_format::SketchSegment {
                    id: "s3".to_string(),
                    from: [1.0, 1.0],
                    to: [0.0, 1.0],
                },
                mycad_format::SketchSegment {
                    id: "s4".to_string(),
                    from: [0.0, 1.0],
                    to: [0.0, 0.0],
                },
            ],
        },
        Feature::Extrude {
            id: "ext1".to_string(),
            sketch: "sketch1".to_string(),
            depth: 1.0,
        },
        Feature::CreateSketch {
            id: "sketch2".to_string(),
            plane: mycad_format::SketchPlane::Xy,
            profile: vec![
                mycad_format::SketchSegment {
                    id: "s5".to_string(),
                    from: [10.0, 0.0],
                    to: [11.0, 0.0],
                },
                mycad_format::SketchSegment {
                    id: "s6".to_string(),
                    from: [11.0, 0.0],
                    to: [11.0, 1.0],
                },
                mycad_format::SketchSegment {
                    id: "s7".to_string(),
                    from: [11.0, 1.0],
                    to: [10.0, 1.0],
                },
                mycad_format::SketchSegment {
                    id: "s8".to_string(),
                    from: [10.0, 1.0],
                    to: [10.0, 0.0],
                },
            ],
        },
        Feature::Extrude {
            id: "ext2".to_string(),
            sketch: "sketch2".to_string(),
            depth: 1.0,
        },
        Feature::Fuse {
            id: "fuse1".to_string(),
            target: "ext1".to_string(),
            tool: "ext2".to_string(),
        },
    ];
    let mut g = IdGenerator::new(0);
    let result = build_bodies_from_features(&features, &mut g);
    assert!(
        matches!(
            result,
            Err(mycad_kernel::error::KernelError::DisjointFuseResult)
        ),
        "expected DisjointFuseResult, got {:?}",
        result
    );
}

#[test]
fn t05_intersect_disjoint_boxes() {
    use mycad_format::Feature;
    // Two extruded rectangles far apart → Intersect = EmptyBooleanResult
    let features = vec![
        Feature::CreateSketch {
            id: "sketch1".to_string(),
            plane: mycad_format::SketchPlane::Xy,
            profile: vec![
                mycad_format::SketchSegment {
                    id: "s1".to_string(),
                    from: [0.0, 0.0],
                    to: [1.0, 0.0],
                },
                mycad_format::SketchSegment {
                    id: "s2".to_string(),
                    from: [1.0, 0.0],
                    to: [1.0, 1.0],
                },
                mycad_format::SketchSegment {
                    id: "s3".to_string(),
                    from: [1.0, 1.0],
                    to: [0.0, 1.0],
                },
                mycad_format::SketchSegment {
                    id: "s4".to_string(),
                    from: [0.0, 1.0],
                    to: [0.0, 0.0],
                },
            ],
        },
        Feature::Extrude {
            id: "ext1".to_string(),
            sketch: "sketch1".to_string(),
            depth: 1.0,
        },
        Feature::CreateSketch {
            id: "sketch2".to_string(),
            plane: mycad_format::SketchPlane::Xy,
            profile: vec![
                mycad_format::SketchSegment {
                    id: "s5".to_string(),
                    from: [10.0, 0.0],
                    to: [11.0, 0.0],
                },
                mycad_format::SketchSegment {
                    id: "s6".to_string(),
                    from: [11.0, 0.0],
                    to: [11.0, 1.0],
                },
                mycad_format::SketchSegment {
                    id: "s7".to_string(),
                    from: [11.0, 1.0],
                    to: [10.0, 1.0],
                },
                mycad_format::SketchSegment {
                    id: "s8".to_string(),
                    from: [10.0, 1.0],
                    to: [10.0, 0.0],
                },
            ],
        },
        Feature::Extrude {
            id: "ext2".to_string(),
            sketch: "sketch2".to_string(),
            depth: 1.0,
        },
        Feature::Intersect {
            id: "int1".to_string(),
            target: "ext1".to_string(),
            tool: "ext2".to_string(),
        },
    ];
    let mut g = IdGenerator::new(0);
    let result = build_bodies_from_features(&features, &mut g);
    assert!(
        matches!(
            result,
            Err(mycad_kernel::error::KernelError::EmptyBooleanResult)
        ),
        "expected EmptyBooleanResult, got {:?}",
        result
    );
}

// --- T06: Forward reference → BodyNotFound ---

#[test]
fn t06_forward_reference_cut() {
    use mycad_format::Feature;
    let features = vec![
        Feature::CreateBox {
            id: "box1".to_string(),
            width: 1.0,
            height: 1.0,
            depth: 1.0,
        },
        Feature::Cut {
            id: "cut1".to_string(),
            target: "box1".to_string(),
            tool: "later_body".to_string(), // not yet created
        },
    ];
    let mut g = IdGenerator::new(0);
    let result = build_bodies_from_features(&features, &mut g);
    assert!(matches!(
        result,
        Err(mycad_kernel::error::KernelError::BodyNotFound { id }) if id == "later_body"
    ));
}

// --- T07: Duplicate feature id (regression) ---

#[test]
fn t07_duplicate_feature_id() {
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
    let result = build_bodies_from_features(&features, &mut g);
    assert!(result.is_err());
    let err = format!("{}", result.unwrap_err());
    assert!(
        err.contains("duplicate feature id"),
        "expected DuplicateFeatureId, got: {err}"
    );
}

// --- T11: Zero bodies (sketch only) → EmptyFeatureList ---

#[test]
fn t11_zero_bodies() {
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
                id: "seg_b".to_string(),
                from: [1.0, 0.0],
                to: [0.0, 0.0],
            },
        ],
    }];
    let mut g = IdGenerator::new(0);
    let result = build_bodies_from_features(&features, &mut g);
    assert!(matches!(
        result,
        Err(mycad_kernel::error::KernelError::EmptyFeatureList)
    ));
}

// --- Sketch not found (regression) ---

#[test]
fn sketch_not_found() {
    use mycad_format::Feature;
    let features = vec![Feature::Extrude {
        id: "ext_1".to_string(),
        sketch: "nonexistent".to_string(),
        depth: 5.0,
    }];
    let mut g = IdGenerator::new(0);
    let result = build_bodies_from_features(&features, &mut g);
    assert!(result.is_err());
    let err = format!("{}", result.unwrap_err());
    assert!(
        err.contains("sketch not found"),
        "expected SketchNotFound, got: {err}"
    );
}

// --- Forward reference for sketch (regression) ---

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
    let result = build_bodies_from_features(&features, &mut g);
    assert!(result.is_err());
    let err = format!("{}", result.unwrap_err());
    assert!(
        err.contains("sketch not found"),
        "expected SketchNotFound for forward reference, got: {err}"
    );
}

// --- Duplicate segment id (regression) ---

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
    let result = build_bodies_from_features(&features, &mut g);
    assert!(result.is_err());
    let err = format!("{}", result.unwrap_err());
    assert!(
        err.contains("sketch_segment_id"),
        "expected segment id error, got: {err}"
    );
}

// --- T01 extended: 100-run determinism ---

#[test]
fn t01_determinism_100_runs() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("examples")
        .join("two_bodies.mycad");

    let doc = Document::from_path(&path).expect("load two_bodies.mycad");

    let mut g0 = IdGenerator::new(0);
    let first = build_bodies_from_features(&doc.root_component.features, &mut g0).expect("build 0");

    for i in 1..100 {
        let mut g = IdGenerator::new(0);
        let bodies = build_bodies_from_features(&doc.root_component.features, &mut g)
            .unwrap_or_else(|_| panic!("build {i}"));
        assert_eq!(bodies.len(), first.len(), "run {i}: body count");
        for (j, (ba, bb)) in bodies.all().iter().zip(first.all().iter()).enumerate() {
            assert_eq!(ba.feature_id, bb.feature_id, "run {i} body {j}: feature_id");
            assert_eq!(ba.solid.id, bb.solid.id, "run {i} body {j}: solid id");
            assert_eq!(
                ba.solid.vertices.len(),
                bb.solid.vertices.len(),
                "run {i} body {j}: vertices"
            );
        }
    }
}

// --- Helper: build features from inline Feature list ---

fn build_features(
    features: Vec<mycad_format::Feature>,
) -> Result<mycad_build::BuiltBodies, mycad_kernel::error::KernelError> {
    let mut g = IdGenerator::new(0);
    build_bodies_from_features(&features, &mut g)
}

// --- T02: Fuse touching boxes (one shared face) → single shell, Euler OK ---

#[test]
fn t02_fuse_touching_boxes() {
    use mycad_format::Feature;
    // Two 2x2x2 boxes sharing the z=2 face: box_a [0,2]×[0,2]×[0,2], box_b [0,2]×[0,2]×[2,4]
    // Since CreateBox always creates from origin, we use extrudes instead
    let features = vec![
        Feature::CreateSketch {
            id: "sk1".into(),
            plane: mycad_format::SketchPlane::Xy,
            profile: vec![
                mycad_format::SketchSegment {
                    id: "s1".into(),
                    from: [0.0, 0.0],
                    to: [2.0, 0.0],
                },
                mycad_format::SketchSegment {
                    id: "s2".into(),
                    from: [2.0, 0.0],
                    to: [2.0, 2.0],
                },
                mycad_format::SketchSegment {
                    id: "s3".into(),
                    from: [2.0, 2.0],
                    to: [0.0, 2.0],
                },
                mycad_format::SketchSegment {
                    id: "s4".into(),
                    from: [0.0, 2.0],
                    to: [0.0, 0.0],
                },
            ],
        },
        Feature::Extrude {
            id: "ext1".into(),
            sketch: "sk1".into(),
            depth: 2.0,
        },
        Feature::CreateSketch {
            id: "sk2".into(),
            plane: mycad_format::SketchPlane::Xy,
            profile: vec![
                mycad_format::SketchSegment {
                    id: "s5".into(),
                    from: [0.0, 0.0],
                    to: [2.0, 0.0],
                },
                mycad_format::SketchSegment {
                    id: "s6".into(),
                    from: [2.0, 0.0],
                    to: [2.0, 2.0],
                },
                mycad_format::SketchSegment {
                    id: "s7".into(),
                    from: [2.0, 2.0],
                    to: [0.0, 2.0],
                },
                mycad_format::SketchSegment {
                    id: "s8".into(),
                    from: [0.0, 2.0],
                    to: [0.0, 0.0],
                },
            ],
        },
        Feature::Extrude {
            id: "ext2".into(),
            sketch: "sk2".into(),
            depth: 2.0,
        },
        Feature::Fuse {
            id: "fuse1".into(),
            target: "ext1".into(),
            tool: "ext2".into(),
        },
    ];
    let bodies = build_features(features).expect("fuse touching should succeed");
    let solid = &bodies.live().next().unwrap().solid;
    assert_eq!(solid.shells.len(), 1, "fuse touching: single shell");
    solid
        .validate_manifold()
        .expect("fuse touching: manifold OK");
    // Euler: V - E + F = 2*S => V-E+F = 2
    let euler = solid.euler_poincare();
    let v = solid.vertices.len();
    let e = solid.edges.len();
    let f = solid.faces.len();
    let s = solid.shells.len();
    let he = solid.half_edges.len();
    let lp = solid.loops.len();
    eprintln!("DEBUG fuse_touching: V={v} E={e} F={f} S={s} HE={he} Loops={lp} euler={euler}");
    // Check edge HE counts
    let mut edge_he_map: std::collections::HashMap<usize, usize> = std::collections::HashMap::new();
    for he_ref in &solid.half_edges {
        *edge_he_map.entry(he_ref.edge).or_insert(0) += 1;
    }
    for (ei, count) in &edge_he_map {
        if *count != 2 {
            eprintln!("  Edge {} has {} HEs (expected 2)", ei, count);
        }
    }
    assert_eq!(euler, 0, "fuse touching: Euler V-E+F-2S = 0, got {euler}");
}

// --- T03: Fuse overlapping boxes → single shell ---

#[test]
fn t03_fuse_overlapping_boxes() {
    use mycad_format::Feature;
    // Two 2x2x2 boxes that overlap: box at origin, box offset by 1 in x
    let features = vec![
        Feature::CreateBox {
            id: "box_a".into(),
            width: 2.0,
            height: 2.0,
            depth: 2.0,
        },
        Feature::CreateBox {
            id: "box_b".into(),
            width: 2.0,
            height: 2.0,
            depth: 2.0,
        },
        Feature::Fuse {
            id: "fuse1".into(),
            target: "box_a".into(),
            tool: "box_b".into(),
        },
    ];
    let bodies = build_features(features).expect("fuse overlapping should succeed");
    let solid = &bodies.live().next().unwrap().solid;
    eprintln!(
        "DEBUG t03: V={} E={} F={} S={} HE={} euler={}",
        solid.vertices.len(),
        solid.edges.len(),
        solid.faces.len(),
        solid.shells.len(),
        solid.half_edges.len(),
        solid.euler_poincare()
    );
    assert_eq!(solid.shells.len(), 1, "fuse overlapping: single shell");
    solid
        .validate_manifold()
        .expect("fuse overlapping: manifold OK");
    let euler = solid.euler_poincare();
    assert_eq!(euler, 0, "fuse overlapping: Euler OK, got {euler}");
}

// --- T04: Intersect overlapping boxes → smaller box ---

#[test]
fn t04_intersect_overlapping_boxes() {
    use mycad_format::Feature;
    let features = vec![
        Feature::CreateBox {
            id: "box_a".into(),
            width: 2.0,
            height: 2.0,
            depth: 2.0,
        },
        Feature::CreateBox {
            id: "box_b".into(),
            width: 2.0,
            height: 2.0,
            depth: 2.0,
        },
        Feature::Intersect {
            id: "int1".into(),
            target: "box_a".into(),
            tool: "box_b".into(),
        },
    ];
    let bodies = build_features(features).expect("intersect overlapping should succeed");
    let solid = &bodies.live().next().unwrap().solid;
    assert_eq!(solid.shells.len(), 1, "intersect: single shell");
    solid.validate_manifold().expect("intersect: manifold OK");
    let euler = solid.euler_poincare();
    assert_eq!(euler, 0, "intersect: Euler OK, got {euler}");
}

// --- T06: Cut partial (L-shaped solid) ---

#[test]
fn t06_cut_partial_l_shape() {
    use mycad_format::Feature;
    // Target: 2x2x2 box centered at origin (-1..1 in each axis).
    // Tool: extrusion at x=[0.5,2], y=[-2,2], z=[0,2] — no face is coplanar with target.
    // Cut removes the x=[0.5,1], z=[0,1] corner, leaving an L-shaped solid.
    let features = vec![
        Feature::CreateBox {
            id: "target".into(),
            width: 2.0,
            height: 2.0,
            depth: 2.0,
        },
        Feature::CreateSketch {
            id: "sk_tool".into(),
            plane: mycad_format::SketchPlane::Xy,
            profile: vec![
                mycad_format::SketchSegment {
                    id: "ts1".into(),
                    from: [0.5, -2.0],
                    to: [2.0, -2.0],
                },
                mycad_format::SketchSegment {
                    id: "ts2".into(),
                    from: [2.0, -2.0],
                    to: [2.0, 2.0],
                },
                mycad_format::SketchSegment {
                    id: "ts3".into(),
                    from: [2.0, 2.0],
                    to: [0.5, 2.0],
                },
                mycad_format::SketchSegment {
                    id: "ts4".into(),
                    from: [0.5, 2.0],
                    to: [0.5, -2.0],
                },
            ],
        },
        Feature::Extrude {
            id: "tool".into(),
            sketch: "sk_tool".into(),
            depth: 2.0,
        },
        Feature::Cut {
            id: "cut1".into(),
            target: "target".into(),
            tool: "tool".into(),
        },
    ];
    let bodies = build_features(features).expect("cut partial should succeed");
    let solid = &bodies.live().next().unwrap().solid;
    assert_eq!(solid.shells.len(), 1, "cut partial: single shell");
    solid.validate_manifold().expect("cut partial: manifold OK");
    let euler = solid.euler_poincare();
    assert_eq!(euler, 0, "cut partial: Euler OK, got {euler}");
}

// --- T08: Cut void shell (outer box with inner box fully inside) ---

#[test]
fn t08_cut_void_shell() {
    use mycad_format::Feature;
    // Outer: 4x4x4, Inner: 2x2x2 (fully inside at center)
    let features = vec![
        Feature::CreateBox {
            id: "outer".into(),
            width: 4.0,
            height: 4.0,
            depth: 4.0,
        },
        Feature::CreateBox {
            id: "inner".into(),
            width: 2.0,
            height: 2.0,
            depth: 2.0,
        },
        Feature::Cut {
            id: "cut1".into(),
            target: "outer".into(),
            tool: "inner".into(),
        },
    ];
    let bodies = build_features(features).expect("cut void should succeed");
    let solid = &bodies.live().next().unwrap().solid;
    assert!(
        solid.shells.len() >= 2,
        "cut void: at least 2 shells (outer + void), got {}",
        solid.shells.len()
    );
    solid.validate_manifold().expect("cut void: manifold OK");
    // Euler: V - E + F = 2*S (S >= 2)
    let euler = solid.euler_poincare();
    assert_eq!(euler, 0, "cut void: Euler OK, got {euler}");
}

// --- T09: Intersect contact only → DegenerateBooleanContact ---

#[test]
fn t09_intersect_contact_only() {
    use mycad_format::Feature;
    // Two extruded rectangles sharing an edge: box at [0,1]×[0,1], box at [1,2]×[0,1]
    // CreateBox always creates from origin, so use extrudes
    let features = vec![
        Feature::CreateSketch {
            id: "sk1".into(),
            plane: mycad_format::SketchPlane::Xy,
            profile: vec![
                mycad_format::SketchSegment {
                    id: "s1".into(),
                    from: [0.0, 0.0],
                    to: [1.0, 0.0],
                },
                mycad_format::SketchSegment {
                    id: "s2".into(),
                    from: [1.0, 0.0],
                    to: [1.0, 1.0],
                },
                mycad_format::SketchSegment {
                    id: "s3".into(),
                    from: [1.0, 1.0],
                    to: [0.0, 1.0],
                },
                mycad_format::SketchSegment {
                    id: "s4".into(),
                    from: [0.0, 1.0],
                    to: [0.0, 0.0],
                },
            ],
        },
        Feature::Extrude {
            id: "ext1".into(),
            sketch: "sk1".into(),
            depth: 1.0,
        },
        Feature::CreateSketch {
            id: "sk2".into(),
            plane: mycad_format::SketchPlane::Xy,
            profile: vec![
                mycad_format::SketchSegment {
                    id: "s5".into(),
                    from: [1.0, 0.0],
                    to: [2.0, 0.0],
                },
                mycad_format::SketchSegment {
                    id: "s6".into(),
                    from: [2.0, 0.0],
                    to: [2.0, 1.0],
                },
                mycad_format::SketchSegment {
                    id: "s7".into(),
                    from: [2.0, 1.0],
                    to: [1.0, 1.0],
                },
                mycad_format::SketchSegment {
                    id: "s8".into(),
                    from: [1.0, 1.0],
                    to: [1.0, 0.0],
                },
            ],
        },
        Feature::Extrude {
            id: "ext2".into(),
            sketch: "sk2".into(),
            depth: 1.0,
        },
        Feature::Intersect {
            id: "int1".into(),
            target: "ext1".into(),
            tool: "ext2".into(),
        },
    ];
    let result = build_features(features);
    // Contact-only intersection should produce empty or degenerate result
    assert!(
        result.is_err(),
        "intersect contact only should error, got {:?}",
        result
    );
    let err = result.unwrap_err();
    assert!(
        matches!(
            err,
            mycad_kernel::error::KernelError::EmptyBooleanResult
                | mycad_kernel::error::KernelError::DegenerateBooleanContact
        ),
        "expected EmptyBooleanResult or DegenerateBooleanContact, got {:?}",
        err
    );
}

// --- T12: Boolean determinism ---

#[test]
fn t12_boolean_determinism() {
    use mycad_format::Feature;
    let features = vec![
        Feature::CreateBox {
            id: "target".into(),
            width: 2.0,
            height: 2.0,
            depth: 2.0,
        },
        Feature::CreateSketch {
            id: "sk_tool".into(),
            plane: mycad_format::SketchPlane::Xy,
            profile: vec![
                mycad_format::SketchSegment {
                    id: "ts1".into(),
                    from: [0.5, -2.0],
                    to: [2.0, -2.0],
                },
                mycad_format::SketchSegment {
                    id: "ts2".into(),
                    from: [2.0, -2.0],
                    to: [2.0, 2.0],
                },
                mycad_format::SketchSegment {
                    id: "ts3".into(),
                    from: [2.0, 2.0],
                    to: [0.5, 2.0],
                },
                mycad_format::SketchSegment {
                    id: "ts4".into(),
                    from: [0.5, 2.0],
                    to: [0.5, -2.0],
                },
            ],
        },
        Feature::Extrude {
            id: "tool".into(),
            sketch: "sk_tool".into(),
            depth: 2.0,
        },
        Feature::Cut {
            id: "cut1".into(),
            target: "target".into(),
            tool: "tool".into(),
        },
    ];
    let b1 = build_features(features.clone()).expect("build 1");
    let b2 = build_features(features).expect("build 2");
    let s1 = &b1.live().next().unwrap().solid;
    let s2 = &b2.live().next().unwrap().solid;
    assert_solids_equal(s1, s2);
}

// --- T17: Export STL from boolean result ---

#[test]
fn t17_boolean_stl_export() {
    use mycad_format::Feature;
    use mycad_kernel::tessellation::{tessellate_solid, to_ascii_stl};

    let features = vec![
        Feature::CreateBox {
            id: "target".into(),
            width: 2.0,
            height: 2.0,
            depth: 2.0,
        },
        Feature::CreateSketch {
            id: "sk_tool".into(),
            plane: mycad_format::SketchPlane::Xy,
            profile: vec![
                mycad_format::SketchSegment {
                    id: "ts1".into(),
                    from: [0.5, -2.0],
                    to: [2.0, -2.0],
                },
                mycad_format::SketchSegment {
                    id: "ts2".into(),
                    from: [2.0, -2.0],
                    to: [2.0, 2.0],
                },
                mycad_format::SketchSegment {
                    id: "ts3".into(),
                    from: [2.0, 2.0],
                    to: [0.5, 2.0],
                },
                mycad_format::SketchSegment {
                    id: "ts4".into(),
                    from: [0.5, 2.0],
                    to: [0.5, -2.0],
                },
            ],
        },
        Feature::Extrude {
            id: "tool".into(),
            sketch: "sk_tool".into(),
            depth: 2.0,
        },
        Feature::Cut {
            id: "cut1".into(),
            target: "target".into(),
            tool: "tool".into(),
        },
    ];
    let bodies = build_features(features).expect("build");
    let solid = &bodies.live().next().unwrap().solid;
    let mesh = tessellate_solid(solid).expect("tessellate");
    assert!(
        mesh.triangle_count() > 0,
        "boolean result should have triangles"
    );
    let stl = to_ascii_stl(&mesh, "boolean_cut");
    assert!(stl.contains("facet"), "STL should contain facets");
}

// --- T20: Build integration — live() returns only result body ---

#[test]
fn t20_build_live_bodies() {
    use mycad_format::Feature;
    let features = vec![
        Feature::CreateBox {
            id: "target".into(),
            width: 2.0,
            height: 2.0,
            depth: 2.0,
        },
        Feature::CreateSketch {
            id: "sk_tool".into(),
            plane: mycad_format::SketchPlane::Xy,
            profile: vec![
                mycad_format::SketchSegment {
                    id: "ts1".into(),
                    from: [0.5, -2.0],
                    to: [2.0, -2.0],
                },
                mycad_format::SketchSegment {
                    id: "ts2".into(),
                    from: [2.0, -2.0],
                    to: [2.0, 2.0],
                },
                mycad_format::SketchSegment {
                    id: "ts3".into(),
                    from: [2.0, 2.0],
                    to: [0.5, 2.0],
                },
                mycad_format::SketchSegment {
                    id: "ts4".into(),
                    from: [0.5, 2.0],
                    to: [0.5, -2.0],
                },
            ],
        },
        Feature::Extrude {
            id: "tool".into(),
            sketch: "sk_tool".into(),
            depth: 2.0,
        },
        Feature::Cut {
            id: "cut1".into(),
            target: "target".into(),
            tool: "tool".into(),
        },
    ];
    let bodies = build_features(features).expect("build");
    assert_eq!(
        bodies.live().count(),
        1,
        "live() should return exactly 1 body (the result)"
    );
    assert_eq!(bodies.len(), 3, "all() should still contain 3 bodies");
    let live_body = bodies.live().next().unwrap();
    assert_eq!(live_body.feature_id, "cut1");
}

// --- T21: Non-planar input → NonPlanarBooleanInput ---

#[test]
fn t21_nonplanar_input_cylinder() {
    use mycad_format::Feature;
    let features = vec![
        Feature::CreateBox {
            id: "box1".into(),
            width: 2.0,
            height: 2.0,
            depth: 2.0,
        },
        Feature::CreateCylinder {
            id: "cyl1".into(),
            radius: 1.0,
            height: 2.0,
        },
        Feature::Cut {
            id: "cut1".into(),
            target: "box1".into(),
            tool: "cyl1".into(),
        },
    ];
    let result = build_features(features);
    assert!(result.is_err(), "cylinder boolean should fail");
    assert!(
        matches!(
            result.unwrap_err(),
            mycad_kernel::error::KernelError::NonPlanarBooleanInput { .. }
        ),
        "expected NonPlanarBooleanInput"
    );
}

// --- T24: Disjoint Fuse → DisjointFuseResult ---

#[test]
fn t24_disjoint_fuse_boxes() {
    use mycad_format::Feature;
    // Use extrudes to place boxes far apart
    let features = vec![
        Feature::CreateSketch {
            id: "sk1".into(),
            plane: mycad_format::SketchPlane::Xy,
            profile: vec![
                mycad_format::SketchSegment {
                    id: "s1".into(),
                    from: [0.0, 0.0],
                    to: [1.0, 0.0],
                },
                mycad_format::SketchSegment {
                    id: "s2".into(),
                    from: [1.0, 0.0],
                    to: [1.0, 1.0],
                },
                mycad_format::SketchSegment {
                    id: "s3".into(),
                    from: [1.0, 1.0],
                    to: [0.0, 1.0],
                },
                mycad_format::SketchSegment {
                    id: "s4".into(),
                    from: [0.0, 1.0],
                    to: [0.0, 0.0],
                },
            ],
        },
        Feature::Extrude {
            id: "ext1".into(),
            sketch: "sk1".into(),
            depth: 1.0,
        },
        Feature::CreateSketch {
            id: "sk2".into(),
            plane: mycad_format::SketchPlane::Xy,
            profile: vec![
                mycad_format::SketchSegment {
                    id: "s5".into(),
                    from: [10.0, 0.0],
                    to: [11.0, 0.0],
                },
                mycad_format::SketchSegment {
                    id: "s6".into(),
                    from: [11.0, 0.0],
                    to: [11.0, 1.0],
                },
                mycad_format::SketchSegment {
                    id: "s7".into(),
                    from: [11.0, 1.0],
                    to: [10.0, 1.0],
                },
                mycad_format::SketchSegment {
                    id: "s8".into(),
                    from: [10.0, 1.0],
                    to: [10.0, 0.0],
                },
            ],
        },
        Feature::Extrude {
            id: "ext2".into(),
            sketch: "sk2".into(),
            depth: 1.0,
        },
        Feature::Fuse {
            id: "fuse1".into(),
            target: "ext1".into(),
            tool: "ext2".into(),
        },
    ];
    let result = build_features(features);
    assert!(
        matches!(
            result,
            Err(mycad_kernel::error::KernelError::DisjointFuseResult)
        ),
        "expected DisjointFuseResult, got {:?}",
        result
    );
}
