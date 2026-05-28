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
fn t05_cut_unsupported() {
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
    assert!(matches!(
        result,
        Err(mycad_kernel::error::KernelError::UnsupportedFeature { kind: "cut" })
    ));
}

#[test]
fn t05_fuse_unsupported() {
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
        Feature::Fuse {
            id: "fuse1".to_string(),
            target: "box1".to_string(),
            tool: "box2".to_string(),
        },
    ];
    let mut g = IdGenerator::new(0);
    let result = build_bodies_from_features(&features, &mut g);
    assert!(matches!(
        result,
        Err(mycad_kernel::error::KernelError::UnsupportedFeature { kind: "fuse" })
    ));
}

#[test]
fn t05_intersect_unsupported() {
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
        Feature::Intersect {
            id: "int1".to_string(),
            target: "box1".to_string(),
            tool: "box2".to_string(),
        },
    ];
    let mut g = IdGenerator::new(0);
    let result = build_bodies_from_features(&features, &mut g);
    assert!(matches!(
        result,
        Err(mycad_kernel::error::KernelError::UnsupportedFeature { kind: "intersect" })
    ));
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
