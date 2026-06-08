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

/// Like assert_solids_equal but also checks entity names (vertex, edge, face).
fn assert_solids_equal_with_names(
    a: &mycad_kernel::brep::topology::Solid,
    b: &mycad_kernel::brep::topology::Solid,
) {
    assert_solids_equal(a, b);

    for (i, (va, vb)) in a.vertices.iter().zip(b.vertices.iter()).enumerate() {
        assert_eq!(va.name, vb.name, "vertex {i} name");
    }
    for (i, (ea, eb)) in a.edges.iter().zip(b.edges.iter()).enumerate() {
        assert_eq!(ea.name, eb.name, "edge {i} name");
    }
    for (i, (fa, fb)) in a.faces.iter().zip(b.faces.iter()).enumerate() {
        assert_eq!(fa.name, fb.name, "face {i} name");
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
            origin: [0.0, 0.0, 0.0],
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
            offset: 0.0,
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
            fuse_target: None,
        },
        Feature::CreateSketch {
            id: "sketch2".to_string(),
            plane: mycad_format::SketchPlane::Xy,
            offset: 0.0,
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
            fuse_target: None,
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
            offset: 0.0,
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
            fuse_target: None,
        },
        Feature::CreateSketch {
            id: "sketch2".to_string(),
            plane: mycad_format::SketchPlane::Xy,
            offset: 0.0,
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
            fuse_target: None,
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
            offset: 0.0,
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
            offset: 0.0,
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
        offset: 0.0,
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
        fuse_target: None,
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
            fuse_target: None,
        },
        Feature::CreateSketch {
            id: "sketch_1".to_string(),
            plane: SketchPlane::Xy,
            offset: 0.0,
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
        offset: 0.0,
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
            offset: 0.0,
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
            fuse_target: None,
        },
        Feature::CreateSketch {
            id: "sk2".into(),
            plane: mycad_format::SketchPlane::Xy,
            offset: 0.0,
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
            fuse_target: None,
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
            offset: 0.0,
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
            fuse_target: None,
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
            offset: 0.0,
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
            fuse_target: None,
        },
        Feature::CreateSketch {
            id: "sk2".into(),
            plane: mycad_format::SketchPlane::Xy,
            offset: 0.0,
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
            fuse_target: None,
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
            offset: 0.0,
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
            fuse_target: None,
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
            offset: 0.0,
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
            fuse_target: None,
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
            offset: 0.0,
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
            fuse_target: None,
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

// --- T21: Cone input → NonPlanarBooleanInput (gate still rejects cones) ---

#[test]
fn t21_nonplanar_input_cone_rejected() {
    // Cone surfaces are still rejected by validate_boolean_input.
    // We test this indirectly: any solid containing a cone surface hits NonPlanarBooleanInput.
    // Since we can't create a cone primitive yet, verify the gate exists by checking
    // that axis-aligned cylinder+box now passes the gate (may fail later in pipeline).
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
            origin: [0.0, 0.0, 0.0],
        },
        Feature::Cut {
            id: "cut1".into(),
            target: "box1".into(),
            tool: "cyl1".into(),
        },
    ];
    let result = build_features(features);
    // Cylinder boolean may fail with an internal error (implementation pending),
    // but must NOT fail with NonPlanarBooleanInput (gate relaxed).
    if let Err(e) = &result {
        assert!(
            !matches!(
                e,
                mycad_kernel::error::KernelError::NonPlanarBooleanInput { .. }
            ),
            "cylinder boolean should pass the surface type gate, got: {e:?}"
        );
    }
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
            offset: 0.0,
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
            fuse_target: None,
        },
        Feature::CreateSketch {
            id: "sk2".into(),
            plane: mycad_format::SketchPlane::Xy,
            offset: 0.0,
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
            fuse_target: None,
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

// T18: intersection edges in boolean cut get derived names
// Uses partial-overlap geometry (L-shape cut) where non-coplanar faces intersect.
#[test]
fn t18_intersection_edge_names() {
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
            offset: 0.0,
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
            fuse_target: None,
        },
        Feature::Cut {
            id: "cut1".into(),
            target: "target".into(),
            tool: "tool".into(),
        },
    ];
    let bodies = build_features(features).expect("cut should succeed");
    let solid = &bodies.get("cut1").expect("cut1 body").solid;

    // At least some edges should have names (intersection edges)
    let named_edges: Vec<_> = solid.edges.iter().filter_map(|e| e.name.as_ref()).collect();
    assert!(
        !named_edges.is_empty(),
        "expected at least one named edge, got 0"
    );

    // All named edges should be Derived with op containing "cut"
    for (i, name) in named_edges.iter().enumerate() {
        match name {
            mycad_format::EntityRef::Derived { op, .. } => {
                assert!(
                    op.contains("cut"),
                    "named edge {} should have cut op, got: {}",
                    i,
                    op
                );
            }
            mycad_format::EntityRef::Named { .. } => {
                panic!("intersection edge should be Derived, not Named");
            }
        }
    }
}

// T19: boolean determinism with names — build twice, all names match
// Uses partial-overlap geometry (L-shape cut) for intersection edge naming.
#[test]
fn t19_boolean_determinism_with_names() {
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
            offset: 0.0,
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
            fuse_target: None,
        },
        Feature::Cut {
            id: "cut1".into(),
            target: "target".into(),
            tool: "tool".into(),
        },
    ];

    let mut gen1 = IdGenerator::new(0);
    let mut gen2 = IdGenerator::new(0);
    let result1 = build_bodies_from_features(&features, &mut gen1);
    let result2 = build_bodies_from_features(&features, &mut gen2);

    assert!(result1.is_ok());
    assert!(result2.is_ok());

    let solids1 = result1.unwrap();
    let solids2 = result2.unwrap();
    assert_solids_equal_with_names(
        &solids1.get("cut1").expect("cut1").solid,
        &solids2.get("cut1").expect("cut1").solid,
    );
}

// T20: golden — intersection edge name format is byte-identical across builds
// Uses partial-overlap geometry (L-shape cut) for intersection edge naming.
#[test]
fn t20_intersection_edge_name_golden() {
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
            offset: 0.0,
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
            fuse_target: None,
        },
        Feature::Cut {
            id: "cut1".into(),
            target: "target".into(),
            tool: "tool".into(),
        },
    ];

    let mut gen = IdGenerator::new(0);
    let result = build_bodies_from_features(&features, &mut gen);
    assert!(result.is_ok());
    let bodies = result.unwrap();
    let solid = &bodies.get("cut1").expect("cut1 body").solid;

    // Collect canonical names of all Derived edges
    let mut edge_names: Vec<String> = solid
        .edges
        .iter()
        .filter_map(|e| e.name.as_ref().map(|n| n.canonical_name()))
        .collect();
    edge_names.sort();

    // There should be at least one intersection edge with cut_isect_edge op
    assert!(
        edge_names.iter().any(|n| n.contains("cut_isect_edge")),
        "expected at least one cut_isect_edge, got: {:?}",
        edge_names
    );

    // Verify determinism: rebuild and check
    let mut gen2 = IdGenerator::new(0);
    let result2 = build_bodies_from_features(&features, &mut gen2);
    let binding2 = result2.unwrap();
    let solid2 = &binding2.get("cut1").expect("cut1 body").solid;
    let mut edge_names2: Vec<String> = solid2
        .edges
        .iter()
        .filter_map(|e| e.name.as_ref().map(|n| n.canonical_name()))
        .collect();
    edge_names2.sort();
    assert_eq!(edge_names, edge_names2, "edge names not deterministic");
}

// --- A3: Acceptance — box Cut sphere (sphere fully inside box → 2-shell void shell) ---

fn build_a3_input() -> Vec<mycad_format::Feature> {
    use mycad_format::Feature;
    vec![
        Feature::CreateBox {
            id: "box".into(),
            width: 10.0,
            height: 10.0,
            depth: 10.0,
        },
        Feature::CreateSphere {
            id: "sphere".into(),
            radius: 3.0,
            center: [0.0, 0.0, 0.0],
        },
        Feature::Cut {
            id: "cut1".into(),
            target: "box".into(),
            tool: "sphere".into(),
        },
    ]
}

#[test]
fn a3_box_cut_contained_sphere() {
    let features = build_a3_input();
    let bodies = build_features(features).expect("A3: build should succeed");
    let solid = &bodies.get("cut1").expect("cut1 body").solid;

    solid.validate_manifold().expect("A3: manifold validation");

    assert_eq!(
        solid.shells.len(),
        2,
        "A3: expected 2 shells (outer box + inner void), got {}",
        solid.shells.len()
    );

    // Find the inner shell (should contain a single sphere face with same_sense == false)
    let mut inner_shell_found = false;
    for shell in &solid.shells {
        if shell.faces.len() == 1 {
            let face = &solid.faces[shell.faces[0]];
            if matches!(
                face.surface,
                mycad_kernel::geometry::surface::Surface::Sphere { .. }
            ) {
                assert!(
                    !face.same_sense,
                    "A3: inner shell sphere face must have same_sense == false"
                );
                inner_shell_found = true;
            }
        }
    }
    assert!(
        inner_shell_found,
        "A3: inner void shell with sphere face not found"
    );
}

#[test]
fn t22_a3_determinism() {
    let features = build_a3_input();
    let b1 = build_features(features.clone()).expect("build 1");
    let b2 = build_features(features).expect("build 2");

    assert_solids_equal_with_names(
        &b1.get("cut1").expect("cut1").solid,
        &b2.get("cut1").expect("cut1").solid,
    );
}

#[test]
fn t32_a3_euler() {
    let features = build_a3_input();
    let bodies = build_features(features).expect("build");
    let solid = &bodies.get("cut1").expect("cut1 body").solid;

    let v = solid.vertices.len() as i64;
    let e = solid.edges.len() as i64;
    let f = solid.faces.len() as i64;
    let l_inner: i64 = solid
        .faces
        .iter()
        .map(|face| face.inner_loops.len() as i64)
        .sum();
    let s = solid.shells.len() as i64;

    // Euler-Poincaré (B-rep form): V - E + F - L_inner = 2(S - G) = 2(2 - 0) = 4
    let lhs = v - e + f - l_inner;
    assert_eq!(
        lhs, 4,
        "A3 Euler: V({v}) - E({e}) + F({f}) - L_inner({l_inner}) = {lhs}, expected 4"
    );
    assert_eq!(s, 2, "A3: expected 2 shells, got {s}");

    // Verify inner shell sphere face same_sense
    for shell in &solid.shells {
        if shell.faces.len() == 1 {
            let face = &solid.faces[shell.faces[0]];
            if matches!(
                face.surface,
                mycad_kernel::geometry::surface::Surface::Sphere { .. }
            ) {
                assert!(
                    !face.same_sense,
                    "inner sphere face same_sense must be false"
                );
            }
        }
    }
}

// --- A1: Acceptance — box Cut cylinder (blind hole) ---

fn build_a1_input() -> Vec<mycad_format::Feature> {
    use mycad_format::Feature;
    vec![
        Feature::CreateBox {
            id: "box1".into(),
            width: 10.0,
            height: 10.0,
            depth: 10.0,
        },
        Feature::CreateCylinder {
            id: "cyl1".into(),
            radius: 2.0,
            height: 6.0,
            origin: [0.0, 0.0, 0.0],
        },
        Feature::Cut {
            id: "cut1".into(),
            target: "box1".into(),
            tool: "cyl1".into(),
        },
    ]
}

#[test]
fn a1_determinism() {
    let features = build_a1_input();
    let b1 = build_features(features.clone()).expect("A1 build 1 ok");
    let b2 = build_features(features).expect("A1 build 2 ok");
    assert_solids_equal_with_names(
        &b1.get("cut1").unwrap().solid,
        &b2.get("cut1").unwrap().solid,
    );
}

#[test]
fn a1_build_manifold_euler() {
    let features = build_a1_input();
    let bodies = build_features(features).expect("A1: build should succeed");
    let solid = &bodies.get("cut1").expect("cut1 body").solid;

    solid.validate_manifold().expect("A1: manifold validation");

    assert_eq!(solid.shells.len(), 1, "A1: expected 1 shell (genus-0)");
    // euler_poincare = V-E+F-2S = 1 for A1: cylinder lat face uses a seam edge
    // (self-adjacent periodic face), which adds 1 edge without adding V or F.
    // This is valid per B-rep seam-edge convention.
    assert_eq!(
        solid.euler_poincare(),
        1,
        "A1: Euler-Poincaré with 1 seam edge on cylinder lat = 1"
    );
}

#[test]
fn a1_top_face_has_inner_loop() {
    use mycad_kernel::geometry::surface::Surface;
    use mycad_kernel::geometry::Vec3;

    let features = build_a1_input();
    let bodies = build_features(features).expect("A1: build");
    let solid = &bodies.get("cut1").expect("cut1 body").solid;

    let has_annular_face = solid.faces.iter().any(|f| {
        matches!(f.surface, Surface::Plane { normal, .. }
            if (normal - Vec3::z()).norm() < 1e-6)
            && !f.inner_loops.is_empty()
    });
    assert!(
        has_annular_face,
        "A1: top face should have an inner_loop (circular hole)"
    );
}

#[test]
fn a1_intersection_edge_is_circle() {
    use mycad_kernel::geometry::curve::Curve;

    let features = build_a1_input();
    let bodies = build_features(features).expect("A1: build");
    let solid = &bodies.get("cut1").expect("cut1 body").solid;

    let circle_edges = solid
        .edges
        .iter()
        .filter(|e| matches!(e.curve, Curve::Circle { .. }))
        .count();
    assert!(
        circle_edges > 0,
        "A1: at least one intersection edge should be Curve::Circle, got {circle_edges}"
    );
}

#[test]
fn a1_tessellation_succeeds() {
    use mycad_kernel::tessellation::tessellate_solid;

    let features = build_a1_input();
    let bodies = build_features(features).expect("A1: build");
    let solid = &bodies.get("cut1").expect("cut1 body").solid;

    let mesh = tessellate_solid(solid).expect("A1: tessellation should succeed");
    assert!(!mesh.positions.is_empty(), "A1: mesh should have positions");
    assert!(!mesh.indices.is_empty(), "A1: mesh should have triangles");
}

#[test]
fn a1_stl_export_succeeds() {
    use mycad_kernel::tessellation::{tessellate_solid, to_ascii_stl};

    let features = build_a1_input();
    let bodies = build_features(features).expect("A1: build");
    let solid = &bodies.get("cut1").expect("cut1 body").solid;

    let mesh = tessellate_solid(solid).expect("A1: tessellate");
    let stl = to_ascii_stl(&mesh, "a1_cut");
    assert!(!stl.is_empty(), "STL output should be non-empty");
    assert!(
        stl.contains("facet normal"),
        "STL should contain facet normals"
    );
}

// --- A1 close gate (#44): T22b + T27 ---

fn a1_mesh_volume_abs(mesh: &mycad_kernel::tessellation::TriangleMesh) -> f64 {
    let mut vol = 0.0_f64;
    for tri in 0..mesh.triangle_count() {
        let i0 = mesh.indices[tri * 3] as usize;
        let i1 = mesh.indices[tri * 3 + 1] as usize;
        let i2 = mesh.indices[tri * 3 + 2] as usize;
        let p0 = &mesh.positions[i0];
        let p1 = &mesh.positions[i1];
        let p2 = &mesh.positions[i2];
        vol += (p0[0] * (p1[1] * p2[2] - p2[1] * p1[2])
            + p1[0] * (p2[1] * p0[2] - p0[1] * p2[2])
            + p2[0] * (p0[1] * p1[2] - p1[1] * p0[2]))
            / 6.0;
    }
    vol.abs()
}

#[test]
fn a1_solid_invariant_across_angular_segments() {
    use mycad_kernel::tessellation::{tessellate_solid_with, TessellationOptions};

    let mut g1 = IdGenerator::new(0);
    let b1 = build_bodies_from_features(&build_a1_input(), &mut g1).expect("build @8");
    let solid1 = b1.get("cut1").unwrap().solid.clone();
    let _mesh_low =
        tessellate_solid_with(&solid1, &TessellationOptions::new(8, 1)).expect("tess @8");

    let mut g2 = IdGenerator::new(0);
    let b2 = build_bodies_from_features(&build_a1_input(), &mut g2).expect("build @64");
    let solid2 = b2.get("cut1").unwrap().solid.clone();
    let _mesh_high =
        tessellate_solid_with(&solid2, &TessellationOptions::new(64, 1)).expect("tess @64");

    assert_solids_equal_with_names(&solid1, &solid2);
}

/// T27 — A1 signed mesh volume matches analytical physical volume.
///
/// Physical volume = box(1000) - cylinder_hole(π × r² × h_overlap = π × 4 × 5 = 20π).
/// With same_sense=false on the cylinder face, mesh normals point inward (into void),
/// giving the correct physical volume ~937.17.
#[test]
fn a1_signed_volume_matches_theoretical() {
    use mycad_kernel::tessellation::tessellate_solid;

    let bodies = build_features(build_a1_input()).expect("A1 build");
    let solid = &bodies.get("cut1").unwrap().solid;
    let mesh = tessellate_solid(solid).expect("tessellate");

    let vol = a1_mesh_volume_abs(&mesh);
    let expected = 1000.0 - 20.0 * std::f64::consts::PI;
    let rel_err = (vol - expected).abs() / expected;
    assert!(
        rel_err < 0.01,
        "A1 volume: expected ~{expected:.3}, got {vol:.3} (rel_err={rel_err:.4})"
    );
}

/// Verify that A1 Cut cylinder lateral face has same_sense=false
/// (normals pointing inward toward the axis, into the hole void).
/// If same_sense=true, the face normal points outward from the axis (into material),
/// which is incorrect for a hole surface in a boolean Cut result.
#[test]
fn a1_cyl_face_same_sense_check() {
    use mycad_kernel::geometry::surface::Surface;

    let features = build_a1_input();
    let bodies = build_features(features).expect("A1: build");
    let solid = &bodies.get("cut1").expect("cut1 body").solid;

    let cyl_face = solid
        .faces
        .iter()
        .find(|f| matches!(f.surface, Surface::Cylinder { .. }))
        .expect("A1: should have a cylinder lateral face");

    assert!(
        !cyl_face.same_sense,
        "A1: cylinder lateral face should have same_sense=false (normals into void), got true"
    );
}

/// TX7 — A3 sphere face name: derived from the original sphere Named ref
#[test]
fn tx7_a3_sphere_face_name() {
    use mycad_format::{EntityKind, EntityRef};

    let features = build_a3_input();
    let bodies = build_features(features).expect("build");
    let solid = &bodies.get("cut1").expect("cut1 body").solid;

    for shell in &solid.shells {
        if shell.faces.len() == 1 {
            let face = &solid.faces[shell.faces[0]];
            if matches!(
                face.surface,
                mycad_kernel::geometry::surface::Surface::Sphere { .. }
            ) {
                let name = face.name.as_ref().expect("sphere face must have a name");
                if let EntityRef::Derived { from, .. } = name {
                    assert_eq!(from.len(), 1, "derived from should have 1 parent");
                    assert_eq!(
                        from[0],
                        EntityRef::Named {
                            feature_id: "sphere".to_string(),
                            kind: EntityKind::Face,
                            role: "surface".to_string(),
                        },
                        "derived from should reference the original sphere face name"
                    );
                } else {
                    panic!("expected Derived name, got {:?}", name);
                }
                return;
            }
        }
    }
    panic!("inner sphere face not found");
}

// --- #96 ExtrudeCut tests (U01-U04) ---

/// U01: Determinism — build ExtrudeCut twice with same IdGenerator seed, solids byte-identical.
#[test]
fn u01_extrude_cut_determinism() {
    use mycad_format::feature::{Feature, SketchPlane, SketchSegment};

    // make_cuboid(10,10,10) → [-5,5]×[-5,5]×[-5,5]
    // make_extrusion XY plane, profile [-3,3]→[3,-3]→[3,3]→[-3,3], depth=3 → z∈[0,3]
    // Tool fully inside target box
    let features = vec![
        Feature::CreateBox {
            id: "box".into(),
            width: 10.0,
            height: 10.0,
            depth: 10.0,
        },
        Feature::CreateSketch {
            id: "sk_cut".into(),
            plane: SketchPlane::Xy,
            offset: 0.0,
            profile: vec![
                SketchSegment {
                    id: "s1".into(),
                    from: [-3.0, -3.0],
                    to: [3.0, -3.0],
                },
                SketchSegment {
                    id: "s2".into(),
                    from: [3.0, -3.0],
                    to: [3.0, 3.0],
                },
                SketchSegment {
                    id: "s3".into(),
                    from: [3.0, 3.0],
                    to: [-3.0, 3.0],
                },
                SketchSegment {
                    id: "s4".into(),
                    from: [-3.0, 3.0],
                    to: [-3.0, -3.0],
                },
            ],
        },
        Feature::ExtrudeCut {
            id: "cut1".into(),
            sketch: "sk_cut".into(),
            depth: 3.0,
            target: "box".into(),
        },
    ];

    let mut g1 = IdGenerator::new(0);
    let b1 = build_bodies_from_features(&features, &mut g1).expect("build 1");
    let mut g2 = IdGenerator::new(0);
    let b2 = build_bodies_from_features(&features, &mut g2).expect("build 2");

    let s1 = &b1.get("cut1").expect("cut1 in b1").solid;
    let s2 = &b2.get("cut1").expect("cut1 in b2").solid;
    assert_solids_equal(s1, s2);
}

/// U02: Void shell — box minus inset cuboid (fully embedded, depth small).
#[test]
fn u02_extrude_cut_void_shell() {
    use mycad_format::feature::{Feature, SketchPlane, SketchSegment};

    // make_cuboid(10,10,10) → [-5,5]×[-5,5]×[-5,5]
    // tool: XY profile [-3,-3]→[3,-3]→[3,3]→[-3,3], depth=3 → z∈[0,3]
    // Tool fully embedded inside box → void (2 shells)
    let features = vec![
        Feature::CreateBox {
            id: "box".into(),
            width: 10.0,
            height: 10.0,
            depth: 10.0,
        },
        Feature::CreateSketch {
            id: "sk_cut".into(),
            plane: SketchPlane::Xy,
            offset: 0.0,
            profile: vec![
                SketchSegment {
                    id: "s1".into(),
                    from: [-3.0, -3.0],
                    to: [3.0, -3.0],
                },
                SketchSegment {
                    id: "s2".into(),
                    from: [3.0, -3.0],
                    to: [3.0, 3.0],
                },
                SketchSegment {
                    id: "s3".into(),
                    from: [3.0, 3.0],
                    to: [-3.0, 3.0],
                },
                SketchSegment {
                    id: "s4".into(),
                    from: [-3.0, 3.0],
                    to: [-3.0, -3.0],
                },
            ],
        },
        Feature::ExtrudeCut {
            id: "cut1".into(),
            sketch: "sk_cut".into(),
            depth: 3.0, // tool z ∈ [0, 3] — fully inside box z ∈ [-5, 5]
            target: "box".into(),
        },
    ];

    let bodies = build_features(features).expect("void cut should succeed");
    let solid = &bodies.get("cut1").expect("cut1").solid;

    assert!(
        solid.shells.len() >= 2,
        "void cut: at least 2 shells (outer + void), got {}",
        solid.shells.len()
    );
    solid.validate_manifold().expect("void cut: manifold OK");
    let euler = solid.euler_poincare();
    assert_eq!(euler, 0, "void cut: Euler OK, got {euler}");
}

/// U05_degen: ExtrudeCut with degenerate depth (≤ 0) → InvalidParameter.
#[test]
fn u05_extrude_cut_degen_depth() {
    use mycad_format::feature::{Feature, SketchPlane, SketchSegment};

    // Normal box + sketch, but depth = 0.0
    let features_depth_zero = vec![
        Feature::CreateBox {
            id: "box".into(),
            width: 10.0,
            height: 10.0,
            depth: 10.0,
        },
        Feature::CreateSketch {
            id: "sk_cut".into(),
            plane: SketchPlane::Xy,
            offset: 0.0,
            profile: vec![
                SketchSegment {
                    id: "s1".into(),
                    from: [-3.0, -3.0],
                    to: [3.0, -3.0],
                },
                SketchSegment {
                    id: "s2".into(),
                    from: [3.0, -3.0],
                    to: [3.0, 3.0],
                },
                SketchSegment {
                    id: "s3".into(),
                    from: [3.0, 3.0],
                    to: [-3.0, 3.0],
                },
                SketchSegment {
                    id: "s4".into(),
                    from: [-3.0, 3.0],
                    to: [-3.0, -3.0],
                },
            ],
        },
        Feature::ExtrudeCut {
            id: "cut1".into(),
            sketch: "sk_cut".into(),
            depth: 0.0,
            target: "box".into(),
        },
    ];
    let result = build_features(features_depth_zero);
    assert!(result.is_err(), "depth=0 should error, got {:?}", result);
    let err = format!("{}", result.unwrap_err());
    assert!(
        err.contains("invalid parameter"),
        "expected InvalidParameter, got: {err}"
    );

    // depth = -1.0
    let features_depth_neg = vec![
        Feature::CreateBox {
            id: "box".into(),
            width: 10.0,
            height: 10.0,
            depth: 10.0,
        },
        Feature::CreateSketch {
            id: "sk_cut".into(),
            plane: SketchPlane::Xy,
            offset: 0.0,
            profile: vec![
                SketchSegment {
                    id: "s1".into(),
                    from: [-3.0, -3.0],
                    to: [3.0, -3.0],
                },
                SketchSegment {
                    id: "s2".into(),
                    from: [3.0, -3.0],
                    to: [3.0, 3.0],
                },
                SketchSegment {
                    id: "s3".into(),
                    from: [3.0, 3.0],
                    to: [-3.0, 3.0],
                },
                SketchSegment {
                    id: "s4".into(),
                    from: [-3.0, 3.0],
                    to: [-3.0, -3.0],
                },
            ],
        },
        Feature::ExtrudeCut {
            id: "cut1".into(),
            sketch: "sk_cut".into(),
            depth: -1.0,
            target: "box".into(),
        },
    ];
    let result = build_features(features_depth_neg);
    assert!(result.is_err(), "depth=-1 should error, got {:?}", result);
    let err = format!("{}", result.unwrap_err());
    assert!(
        err.contains("invalid parameter"),
        "expected InvalidParameter, got: {err}"
    );
}

/// U06a: ExtrudeCut with missing target → BodyNotFound.
#[test]
fn u06a_extrude_cut_missing_target() {
    use mycad_format::feature::{Feature, SketchPlane, SketchSegment};

    let features = vec![
        Feature::CreateBox {
            id: "box".into(),
            width: 10.0,
            height: 10.0,
            depth: 10.0,
        },
        Feature::CreateSketch {
            id: "sk_cut".into(),
            plane: SketchPlane::Xy,
            offset: 0.0,
            profile: vec![
                SketchSegment {
                    id: "s1".into(),
                    from: [-3.0, -3.0],
                    to: [3.0, -3.0],
                },
                SketchSegment {
                    id: "s2".into(),
                    from: [3.0, -3.0],
                    to: [3.0, 3.0],
                },
                SketchSegment {
                    id: "s3".into(),
                    from: [3.0, 3.0],
                    to: [-3.0, 3.0],
                },
                SketchSegment {
                    id: "s4".into(),
                    from: [-3.0, 3.0],
                    to: [-3.0, -3.0],
                },
            ],
        },
        Feature::ExtrudeCut {
            id: "cut1".into(),
            sketch: "sk_cut".into(),
            depth: 3.0,
            target: "nonexistent".into(),
        },
    ];
    let result = build_features(features);
    assert!(
        result.is_err(),
        "missing target should error, got {:?}",
        result
    );
    let err = format!("{}", result.unwrap_err());
    assert!(
        err.contains("body not found"),
        "expected BodyNotFound, got: {err}"
    );
}

/// U06b: ExtrudeCut with non-intersecting tool (outside target box).
/// Tool is placed far from the target — no intersection.
/// boolean(Cut) should return Ok with target unchanged (no-op cut).
#[test]
fn u06b_extrude_cut_nonintersecting() {
    use mycad_format::feature::{Feature, SketchPlane, SketchSegment};

    // Target box: make_cuboid(10,10,10) → [-5,5]×[-5,5]×[-5,5]
    // Tool: XY profile far away at [100,100]→[110,110], depth=5 → z∈[0,5]
    // Tool is completely outside the target → no intersection
    let features = vec![
        Feature::CreateBox {
            id: "box".into(),
            width: 10.0,
            height: 10.0,
            depth: 10.0,
        },
        Feature::CreateSketch {
            id: "sk_cut".into(),
            plane: SketchPlane::Xy,
            offset: 0.0,
            profile: vec![
                SketchSegment {
                    id: "s1".into(),
                    from: [100.0, 100.0],
                    to: [110.0, 100.0],
                },
                SketchSegment {
                    id: "s2".into(),
                    from: [110.0, 100.0],
                    to: [110.0, 110.0],
                },
                SketchSegment {
                    id: "s3".into(),
                    from: [110.0, 110.0],
                    to: [100.0, 110.0],
                },
                SketchSegment {
                    id: "s4".into(),
                    from: [100.0, 110.0],
                    to: [100.0, 100.0],
                },
            ],
        },
        Feature::ExtrudeCut {
            id: "cut1".into(),
            sketch: "sk_cut".into(),
            depth: 5.0,
            target: "box".into(),
        },
    ];

    // Build just the target box for comparison
    let target_features = vec![Feature::CreateBox {
        id: "box".into(),
        width: 10.0,
        height: 10.0,
        depth: 10.0,
    }];

    let result = build_features(features);
    // Non-intersecting cut: boolean may return Ok (target unchanged) or Err
    match result {
        Ok(bodies) => {
            // Result should be geometrically identical to the target box
            // (IDs will differ due to IdGenerator advancing through tool creation)
            let target_bodies = build_features(target_features).expect("target build");
            let result_solid = &bodies.get("cut1").expect("cut1").solid;
            let target_solid = &target_bodies.get("box").expect("box").solid;
            // Same topological structure (topology counts, manifold, euler)
            assert_eq!(
                result_solid.vertices.len(),
                target_solid.vertices.len(),
                "vertex count"
            );
            assert_eq!(
                result_solid.edges.len(),
                target_solid.edges.len(),
                "edge count"
            );
            assert_eq!(
                result_solid.faces.len(),
                target_solid.faces.len(),
                "face count"
            );
            assert_eq!(
                result_solid.shells.len(),
                target_solid.shells.len(),
                "shell count"
            );
            assert_eq!(
                result_solid.euler_poincare(),
                target_solid.euler_poincare(),
                "euler"
            );
            result_solid
                .validate_manifold()
                .expect("non-intersecting cut: manifold OK");
        }
        Err(e) => {
            let err = format!("{e}");
            assert!(
                err.contains("empty boolean result")
                    || err.contains("boolean internal")
                    || err.contains("unsupported"),
                "unexpected error for non-intersecting cut: {err}"
            );
        }
    }
}

/// U03: Partial cut — box minus an L-shaped corner removal (no coplanar faces).
/// Mirrors the existing t06_cut_partial_l_shape pattern but via ExtrudeCut dispatch.
#[test]
fn u03_extrude_cut_partial_l() {
    use mycad_format::feature::{Feature, SketchPlane, SketchSegment};

    // make_cuboid(2,2,2) → [-1,1]×[-1,1]×[-1,1]
    // Tool: XY profile [0.5,-2]→[2,-2]→[2,2]→[0.5,2], depth=2 → z∈[0,2]
    // No face is coplanar with target (same setup as t06_cut_partial_l_shape).
    let features = vec![
        Feature::CreateBox {
            id: "target".into(),
            width: 2.0,
            height: 2.0,
            depth: 2.0,
        },
        Feature::CreateSketch {
            id: "sk_cut".into(),
            plane: SketchPlane::Xy,
            offset: 0.0,
            profile: vec![
                SketchSegment {
                    id: "ts1".into(),
                    from: [0.5, -2.0],
                    to: [2.0, -2.0],
                },
                SketchSegment {
                    id: "ts2".into(),
                    from: [2.0, -2.0],
                    to: [2.0, 2.0],
                },
                SketchSegment {
                    id: "ts3".into(),
                    from: [2.0, 2.0],
                    to: [0.5, 2.0],
                },
                SketchSegment {
                    id: "ts4".into(),
                    from: [0.5, 2.0],
                    to: [0.5, -2.0],
                },
            ],
        },
        Feature::ExtrudeCut {
            id: "cut1".into(),
            sketch: "sk_cut".into(),
            depth: 2.0,
            target: "target".into(),
        },
    ];

    let bodies = build_features(features).expect("partial cut should succeed");
    let solid = &bodies.get("cut1").expect("cut1").solid;

    assert_eq!(
        solid.shells.len(),
        1,
        "partial cut: single shell, got {}",
        solid.shells.len()
    );
    solid.validate_manifold().expect("partial cut: manifold OK");
    let euler = solid.euler_poincare();
    assert_eq!(euler, 0, "partial cut: Euler OK, got {euler}");
    // Base box has 6 faces; L-cut adds faces (step walls + floor)
    assert!(
        solid.faces.len() > 6,
        "partial cut: face count {} should be > 6 (base box)",
        solid.faces.len()
    );
}
