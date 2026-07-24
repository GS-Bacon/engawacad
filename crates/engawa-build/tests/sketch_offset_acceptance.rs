// Acceptance test for #295 (Sketch Offset)
// Core tests (T01-T08) — determinism, normal behavior, selection

use engawa_build::build_bodies_from_features;
use engawa_format::{Feature, SketchElement};
use engawa_kernel::brep::topology::IdGenerator;

/// T01: Determinism — offset applied twice produces identical results.
/// Circle-only (Line + Circle multi-element is rejected by Phase 10 scope defense).
#[test]
fn t01_determinism_offset() {
    let features = vec![
        Feature::CreateSketch {
            id: "sketch_0".to_string(),
            plane: engawa_format::SketchPlane::Xy,
            offset: 0.0,
            variables: vec![],
            profile: vec![SketchElement::Circle {
                id: "c1".to_string(),
                center: [5.0, 5.0],
                radius: 3.0,
            }],
            plane_ref: None,
            suppressed: false,
        },
        Feature::SketchOffset {
            id: "off1".to_string(),
            sketch: "sketch_0".to_string(),
            selection: vec![],
            distance: 1.0,
            suppressed: false,
        },
        Feature::Extrude {
            id: "ext1".to_string(),
            sketch: "sketch_0".to_string(),
            depth: 5.0,
            fuse_target: None,
            suppressed: false,
        },
    ];

    let mut gen1 = IdGenerator::new(0);
    let bodies1 = build_bodies_from_features(&features, &[], &mut gen1).unwrap();

    let mut gen2 = IdGenerator::new(0);
    let bodies2 = build_bodies_from_features(&features, &[], &mut gen2).unwrap();

    // Solid vertex count must match
    let v1 = bodies1.live().next().unwrap().solid.vertices.len();
    let v2 = bodies2.live().next().unwrap().solid.vertices.len();
    assert_eq!(v1, v2, "vertex count mismatch across runs");
}

/// T01a: Selection order invariance — verified at kernel level.
///
/// The build path cannot host a multi-element profile compatible with SketchOffset
/// (mixed Line + Circle is rejected by `validate_sketch_profile_contours`, and
/// connected Line polygons are rejected by C-F01 fix `sketch_offset_of_connected_lines`).
/// Selection order invariance is verified directly against the kernel pure function.
#[test]
fn t01a_selection_order_independence() {
    use engawa_kernel::geometry::sketch_offset::apply_sketch_offset;
    let profile = vec![
        SketchElement::Line {
            id: "l_a".to_string(),
            from: [0.0, 0.0],
            to: [10.0, 0.0],
        },
        SketchElement::Circle {
            id: "c_b".to_string(),
            center: [5.0, 5.0],
            radius: 2.0,
        },
    ];
    let forward =
        apply_sketch_offset(&profile, &["l_a".to_string(), "c_b".to_string()], 1.0).unwrap();
    let reverse =
        apply_sketch_offset(&profile, &["c_b".to_string(), "l_a".to_string()], 1.0).unwrap();
    assert_eq!(forward, reverse, "selection order affects output");
}

/// T02: Line offset covered at kernel level.
/// See `crates/engawa-kernel/src/geometry/sketch_offset.rs::tests::t02_line_offset`.
/// Build-level testing requires non-Line elements to avoid C-F01 rejection.
#[test]
fn t02_line_offset_covered_at_kernel_level() {
    // see sketch_offset.rs::tests::t02_line_offset
}

/// T03: Circle offset (positive distance).
#[test]
fn t03_offset_circle_positive_distance() {
    let features = vec![
        Feature::CreateSketch {
            id: "sketch_0".to_string(),
            plane: engawa_format::SketchPlane::Xy,
            offset: 0.0,
            variables: vec![],
            profile: vec![SketchElement::Circle {
                id: "c1".to_string(),
                center: [5.0, 5.0],
                radius: 3.0,
            }],
            plane_ref: None,
            suppressed: false,
        },
        Feature::SketchOffset {
            id: "off1".to_string(),
            sketch: "sketch_0".to_string(),
            selection: vec![],
            distance: 2.0,
            suppressed: false,
        },
        Feature::Extrude {
            id: "ext1".to_string(),
            sketch: "sketch_0".to_string(),
            depth: 5.0,
            fuse_target: None,
            suppressed: false,
        },
    ];

    let mut gen = IdGenerator::new(0);
    let bodies = build_bodies_from_features(&features, &[], &mut gen).unwrap();
    let solid = &bodies.live().next().unwrap().solid;

    // Offset circle r=3 → r=5, extruded
    // Bounding box radius should be ~5 (from center)
    let cx = 5.0;
    let cy = 5.0;
    let mut max_dist_sq: f64 = 0.0;
    for v in &solid.vertices {
        let dx = v.point.x - cx;
        let dy = v.point.y - cy;
        let dist_sq = dx * dx + dy * dy;
        max_dist_sq = max_dist_sq.max(dist_sq);
    }
    let max_dist = max_dist_sq.sqrt();
    // r=5 → dist ≈ 5
    assert!(max_dist > 4.8, "circle radius not increased by offset");
}

/// T04: Arc offset covered at kernel level.
/// Build-level testing requires Circle (A-F01 + C-F01 fix).
/// See `crates/engawa-kernel/src/geometry/sketch_offset.rs::tests::t04_arc_offset`.
#[test]
fn t04_arc_offset_covered_at_kernel_level() {
    // see sketch_offset.rs::tests::t04_arc_offset
}

/// T06: Extrude uses offset profile.
#[test]
fn t06_extrude_uses_offset_profile() {
    // Baseline: no offset
    let features_baseline = vec![
        Feature::CreateSketch {
            id: "sketch_0".to_string(),
            plane: engawa_format::SketchPlane::Xy,
            offset: 0.0,
            variables: vec![],
            profile: vec![SketchElement::Circle {
                id: "c1".to_string(),
                center: [0.0, 0.0],
                radius: 5.0,
            }],
            plane_ref: None,
            suppressed: false,
        },
        Feature::Extrude {
            id: "ext1".to_string(),
            sketch: "sketch_0".to_string(),
            depth: 5.0,
            fuse_target: None,
            suppressed: false,
        },
    ];

    let mut gen_baseline = IdGenerator::new(0);
    let bodies_baseline =
        build_bodies_from_features(&features_baseline, &[], &mut gen_baseline).unwrap();
    let solid_baseline = &bodies_baseline.live().next().unwrap().solid;

    // With offset
    let features_offset = vec![
        Feature::CreateSketch {
            id: "sketch_0".to_string(),
            plane: engawa_format::SketchPlane::Xy,
            offset: 0.0,
            variables: vec![],
            profile: vec![SketchElement::Circle {
                id: "c1".to_string(),
                center: [0.0, 0.0],
                radius: 5.0,
            }],
            plane_ref: None,
            suppressed: false,
        },
        Feature::SketchOffset {
            id: "off1".to_string(),
            sketch: "sketch_0".to_string(),
            selection: vec![],
            distance: 2.0,
            suppressed: false,
        },
        Feature::Extrude {
            id: "ext1".to_string(),
            sketch: "sketch_0".to_string(),
            depth: 5.0,
            fuse_target: None,
            suppressed: false,
        },
    ];

    let mut gen_offset = IdGenerator::new(0);
    let bodies_offset = build_bodies_from_features(&features_offset, &[], &mut gen_offset).unwrap();
    let solid_offset = &bodies_offset.live().next().unwrap().solid;

    // Offset solid should have larger bounding box
    let mut max_r_baseline: f64 = 0.0;
    for v in &solid_baseline.vertices {
        let r = (v.point.x * v.point.x + v.point.y * v.point.y).sqrt();
        max_r_baseline = max_r_baseline.max(r);
    }

    let mut max_r_offset: f64 = 0.0;
    for v in &solid_offset.vertices {
        let r = (v.point.x * v.point.x + v.point.y * v.point.y).sqrt();
        max_r_offset = max_r_offset.max(r);
    }

    assert!(
        max_r_offset > max_r_baseline + 1.5,
        "offset did not affect extruded profile"
    );
}

/// T07: Selection partial application covered at kernel level.
/// See `crates/engawa-kernel/src/geometry/sketch_offset.rs::tests::t07_selection_partial`.
#[test]
fn t07_selection_partial_covered_at_kernel_level() {
    // see sketch_offset.rs::tests::t07_selection_partial
}

/// T08: Empty selection means all elements offset.
/// Single circle (closed primitive), all elements offset.
#[test]
fn t08_selection_empty_means_all() {
    let features = vec![
        Feature::CreateSketch {
            id: "sketch_0".to_string(),
            plane: engawa_format::SketchPlane::Xy,
            offset: 0.0,
            variables: vec![],
            profile: vec![SketchElement::Circle {
                id: "c1".to_string(),
                center: [5.0, 5.0],
                radius: 2.0,
            }],
            plane_ref: None,
            suppressed: false,
        },
        Feature::SketchOffset {
            id: "off1".to_string(),
            sketch: "sketch_0".to_string(),
            selection: vec![], // Empty = all
            distance: 1.0,
            suppressed: false,
        },
        Feature::Extrude {
            id: "ext1".to_string(),
            sketch: "sketch_0".to_string(),
            depth: 5.0,
            fuse_target: None,
            suppressed: false,
        },
    ];

    let mut gen = IdGenerator::new(0);
    let bodies = build_bodies_from_features(&features, &[], &mut gen).unwrap();
    let solid = &bodies.live().next().unwrap().solid;
    // Verify radius increased (2 → 3)
    let mut max_r: f64 = 0.0;
    for v in &solid.vertices {
        let dx = v.point.x - 5.0;
        let dy = v.point.y - 5.0;
        let r = (dx * dx + dy * dy).sqrt();
        max_r = max_r.max(r);
    }
    // r=3 after offset
    assert!(max_r > 2.8, "circle radius not increased by offset");
}

// Degeneracy tests (T_DEG_*) — edge cases

/// T_DEG_zero_distance_noop: Near-zero distance → no-op.
/// Single circle profile.
#[test]
fn t_deg_zero_distance_noop() {
    let features = vec![
        Feature::CreateSketch {
            id: "sketch_0".to_string(),
            plane: engawa_format::SketchPlane::Xy,
            offset: 0.0,
            variables: vec![],
            profile: vec![SketchElement::Circle {
                id: "c1".to_string(),
                center: [0.0, 0.0],
                radius: 3.0,
            }],
            plane_ref: None,
            suppressed: false,
        },
        Feature::SketchOffset {
            id: "off1".to_string(),
            sketch: "sketch_0".to_string(),
            selection: vec![],
            distance: 1e-10, // < EPS_LENGTH
            suppressed: false,
        },
        Feature::Extrude {
            id: "ext1".to_string(),
            sketch: "sketch_0".to_string(),
            depth: 5.0,
            fuse_target: None,
            suppressed: false,
        },
    ];

    let mut gen = IdGenerator::new(0);
    let bodies = build_bodies_from_features(&features, &[], &mut gen).unwrap();
    let solid = &bodies.live().next().unwrap().solid;
    // Verify radius unchanged (no-op)
    let mut max_r: f64 = 0.0;
    for v in &solid.vertices {
        let r = (v.point.x * v.point.x + v.point.y * v.point.y).sqrt();
        max_r = max_r.max(r);
    }
    // r=3 unchanged
    assert!(max_r < 3.2 && max_r > 2.8, "no-op distance changed profile");
}

/// T_DEG_zero_distance_boundary: EPS_LENGTH exactly → no-op.
/// Single circle profile.
#[test]
fn t_deg_zero_distance_boundary_epsilon_length() {
    use engawa_kernel::LENGTH_TOLERANCE;

    let features = vec![
        Feature::CreateSketch {
            id: "sketch_0".to_string(),
            plane: engawa_format::SketchPlane::Xy,
            offset: 0.0,
            variables: vec![],
            profile: vec![SketchElement::Circle {
                id: "c1".to_string(),
                center: [0.0, 0.0],
                radius: 3.0,
            }],
            plane_ref: None,
            suppressed: false,
        },
        Feature::SketchOffset {
            id: "off1".to_string(),
            sketch: "sketch_0".to_string(),
            selection: vec![],
            distance: LENGTH_TOLERANCE,
            suppressed: false,
        },
        Feature::Extrude {
            id: "ext1".to_string(),
            sketch: "sketch_0".to_string(),
            depth: 5.0,
            fuse_target: None,
            suppressed: false,
        },
    ];

    let mut gen = IdGenerator::new(0);
    let bodies = build_bodies_from_features(&features, &[], &mut gen).unwrap();
    let solid = &bodies.live().next().unwrap().solid;
    // Verify radius unchanged (boundary no-op per ADR-018 `<=` rule)
    let mut max_r: f64 = 0.0;
    for v in &solid.vertices {
        let r = (v.point.x * v.point.x + v.point.y * v.point.y).sqrt();
        max_r = max_r.max(r);
    }
    // r=3 unchanged
    assert!(max_r < 3.2 && max_r > 2.8, "boundary no-op changed profile");
}

/// T_DEG_offset_collapse_circle: Negative distance that collapses radius.
#[test]
fn t_deg_offset_collapse_circle_negative_radius() {
    let features = vec![
        Feature::CreateSketch {
            id: "sketch_0".to_string(),
            plane: engawa_format::SketchPlane::Xy,
            offset: 0.0,
            variables: vec![],
            profile: vec![SketchElement::Circle {
                id: "c1".to_string(),
                center: [0.0, 0.0],
                radius: 3.0,
            }],
            plane_ref: None,
            suppressed: false,
        },
        Feature::SketchOffset {
            id: "off1".to_string(),
            sketch: "sketch_0".to_string(),
            selection: vec![],
            distance: -3.0 - 1e-8, // Collapses radius
            suppressed: false,
        },
        Feature::Extrude {
            id: "ext1".to_string(),
            sketch: "sketch_0".to_string(),
            depth: 5.0,
            fuse_target: None,
            suppressed: false,
        },
    ];

    let mut gen = IdGenerator::new(0);
    let result = build_bodies_from_features(&features, &[], &mut gen);
    assert!(result.is_err(), "collapsed circle should error");
}

/// T_DEG_offset_collapse_line: Zero-length line fails.
/// Covered at kernel level (build-level rejects Line profiles).
/// See `crates/engawa-kernel/src/geometry/sketch_offset.rs::tests::t_deg_offset_collapse_line`.
#[test]
fn t_deg_offset_collapse_line_degenerate_length() {
    // see sketch_offset.rs::tests::t_deg_offset_collapse_line
}

/// T_DEG_ellipse_reject: Ellipse in selection is rejected.
#[test]
fn t_deg_ellipse_reject_unsupported_feature() {
    let features = vec![
        Feature::CreateSketch {
            id: "sketch_0".to_string(),
            plane: engawa_format::SketchPlane::Xy,
            offset: 0.0,
            variables: vec![],
            profile: vec![SketchElement::Ellipse {
                id: "e1".to_string(),
                center: [0.0, 0.0],
                major: 2.0,
                minor: 1.0,
                rotation: 0.0,
            }],
            plane_ref: None,
            suppressed: false,
        },
        Feature::SketchOffset {
            id: "off1".to_string(),
            sketch: "sketch_0".to_string(),
            selection: vec![],
            distance: 1.0,
            suppressed: false,
        },
        Feature::Extrude {
            id: "ext1".to_string(),
            sketch: "sketch_0".to_string(),
            depth: 5.0,
            fuse_target: None,
            suppressed: false,
        },
    ];

    let mut gen = IdGenerator::new(0);
    let result = build_bodies_from_features(&features, &[], &mut gen);
    assert!(result.is_err(), "ellipse offset should be rejected");
}

/// T_DEG_nan_distance: NaN distance is rejected.
/// Circle profile (build-level valid).
#[test]
fn t_deg_nan_distance_invalid_parameter() {
    let features = vec![
        Feature::CreateSketch {
            id: "sketch_0".to_string(),
            plane: engawa_format::SketchPlane::Xy,
            offset: 0.0,
            variables: vec![],
            profile: vec![SketchElement::Circle {
                id: "c1".to_string(),
                center: [0.0, 0.0],
                radius: 3.0,
            }],
            plane_ref: None,
            suppressed: false,
        },
        Feature::SketchOffset {
            id: "off1".to_string(),
            sketch: "sketch_0".to_string(),
            selection: vec![],
            distance: f64::NAN,
            suppressed: false,
        },
        Feature::Extrude {
            id: "ext1".to_string(),
            sketch: "sketch_0".to_string(),
            depth: 5.0,
            fuse_target: None,
            suppressed: false,
        },
    ];

    let mut gen = IdGenerator::new(0);
    let result = build_bodies_from_features(&features, &[], &mut gen);
    assert!(result.is_err(), "NaN distance should be rejected");
}

/// T_DEG_sketch_ref_not_found: Non-existent sketch id fails.
#[test]
fn t_deg_sketch_ref_not_found() {
    let features = vec![Feature::SketchOffset {
        id: "off1".to_string(),
        sketch: "nonexistent".to_string(),
        selection: vec![],
        distance: 1.0,
        suppressed: false,
    }];

    let mut gen = IdGenerator::new(0);
    let result = build_bodies_from_features(&features, &[], &mut gen);
    assert!(result.is_err(), "non-existent sketch should error");
}

/// A-F01 + C-F01 fix: Build-level SketchOffset only accepts Circle single-element.
/// Line/Arc profiles are rejected at build level (kernel-level pure functions still work).
#[test]
fn t_deg_line_arc_rejected_at_build() {
    // Closed Line loop (square) — passes validate_profile_closed, rejected by SketchOffset
    let features = vec![
        Feature::CreateSketch {
            id: "sketch_0".to_string(),
            plane: engawa_format::SketchPlane::Xy,
            offset: 0.0,
            variables: vec![],
            profile: vec![
                SketchElement::Line {
                    id: "l1".to_string(),
                    from: [0.0, 0.0],
                    to: [10.0, 0.0],
                },
                SketchElement::Line {
                    id: "l2".to_string(),
                    from: [10.0, 0.0],
                    to: [10.0, 10.0],
                },
                SketchElement::Line {
                    id: "l3".to_string(),
                    from: [10.0, 10.0],
                    to: [0.0, 10.0],
                },
                SketchElement::Line {
                    id: "l4".to_string(),
                    from: [0.0, 10.0],
                    to: [0.0, 0.0],
                },
            ],
            plane_ref: None,
            suppressed: false,
        },
        Feature::SketchOffset {
            id: "off1".to_string(),
            sketch: "sketch_0".to_string(),
            selection: vec![],
            distance: 1.0,
            suppressed: false,
        },
    ];

    let mut gen = IdGenerator::new(0);
    let err = build_bodies_from_features(&features, &[], &mut gen).unwrap_err();
    assert!(matches!(
        err,
        engawa_kernel::error::KernelError::UnsupportedFeature {
            kind: "sketch_offset_only_circle"
        }
    ));
}

/// A-F01 + C-F01 fix: Build-level SketchOffset only accepts Circle single-element.
/// Multi-element Line polygon is rejected at build level.
#[test]
fn t_deg_connected_lines_rejected() {
    let features = vec![
        Feature::CreateSketch {
            id: "sketch_0".to_string(),
            plane: engawa_format::SketchPlane::Xy,
            offset: 0.0,
            variables: vec![],
            profile: vec![
                SketchElement::Line {
                    id: "l1".to_string(),
                    from: [0.0, 0.0],
                    to: [10.0, 0.0],
                },
                SketchElement::Line {
                    id: "l2".to_string(),
                    from: [10.0, 0.0],
                    to: [10.0, 10.0],
                },
                SketchElement::Line {
                    id: "l3".to_string(),
                    from: [10.0, 10.0],
                    to: [0.0, 10.0],
                },
                SketchElement::Line {
                    id: "l4".to_string(),
                    from: [0.0, 10.0],
                    to: [0.0, 0.0],
                },
            ],
            plane_ref: None,
            suppressed: false,
        },
        Feature::SketchOffset {
            id: "off1".to_string(),
            sketch: "sketch_0".to_string(),
            selection: vec![],
            distance: 1.0,
            suppressed: false,
        },
    ];

    let mut gen = IdGenerator::new(0);
    let err = build_bodies_from_features(&features, &[], &mut gen).unwrap_err();
    assert!(matches!(
        err,
        engawa_kernel::error::KernelError::UnsupportedFeature {
            kind: "sketch_offset_only_circle"
        }
    ));
}

/// A-F01 + C-F01 fix: Build-level SketchOffset only accepts Circle single-element.
/// Single Arc profile is rejected at build level.
#[test]
fn t_deg_single_arc_rejected_at_build() {
    let features = vec![
        Feature::CreateSketch {
            id: "sketch_0".to_string(),
            plane: engawa_format::SketchPlane::Xy,
            offset: 0.0,
            variables: vec![],
            profile: vec![SketchElement::Arc {
                id: "a1".to_string(),
                center: [0.0, 0.0],
                radius: 5.0,
                start_angle: 0.0,
                end_angle: std::f64::consts::PI / 2.0,
            }],
            plane_ref: None,
            suppressed: false,
        },
        Feature::SketchOffset {
            id: "off1".to_string(),
            sketch: "sketch_0".to_string(),
            selection: vec![],
            distance: 1.0,
            suppressed: false,
        },
    ];

    let mut gen = IdGenerator::new(0);
    let err = build_bodies_from_features(&features, &[], &mut gen).unwrap_err();
    assert!(matches!(
        err,
        engawa_kernel::error::KernelError::UnsupportedFeature {
            kind: "sketch_offset_only_circle"
        }
    ));
}
