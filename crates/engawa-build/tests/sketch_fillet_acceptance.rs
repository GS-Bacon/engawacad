//! Acceptance tests for #296 (Sketch Fillet)
//! テスト計画: features/296-phase10-sketch-fillet-engawa/plan.md 「## テスト計画（ID 付き）」参照

use engawa_build::build_bodies_from_features;
use engawa_build::{FeatureCrud, FeatureCrudError};
use engawa_format::{Document, Feature, RefPlane, SketchElement, SketchPlane};
use engawa_kernel::brep::topology::IdGenerator;
use engawa_kernel::error::KernelError;
use engawa_kernel::geometry::math::LENGTH_TOLERANCE;
use engawa_kernel::geometry::sketch_fillet::apply_sketch_fillet_build;
use std::f64::consts::PI;

// === T01: kernel pure-function determinism + derived Arc id ===
#[test]
fn t01_determinism_and_derived_arc_id() {
    let profile = rect_profile();
    let out1 = apply_sketch_fillet_build(&profile, "l1", "l2", 1.0).unwrap();
    let out2 = apply_sketch_fillet_build(&profile, "l1", "l2", 1.0).unwrap();
    assert_eq!(out1, out2);

    let arc = out1
        .iter()
        .find(|e| matches!(e, SketchElement::Arc { .. }))
        .expect("fillet should insert an Arc");
    let arc_id = match arc {
        SketchElement::Arc { id, .. } => id.as_str(),
        _ => unreachable!(),
    };
    assert_eq!(arc_id, "l1_l2_fillet_arc");
}

// === T01b: elem1_id/elem2_id input order invariance ===
#[test]
fn t01b_input_order_invariance() {
    let profile = rect_profile();
    let forward = apply_sketch_fillet_build(&profile, "l1", "l2", 1.0).unwrap();
    let reverse = apply_sketch_fillet_build(&profile, "l2", "l1", 1.0).unwrap();
    assert_eq!(forward, reverse);
}

// === T01c: Arc ID collision guard ===
#[test]
fn t01c_arc_id_collision_rejected() {
    let mut profile = rect_profile();
    profile.push(SketchElement::Arc {
        id: "l1_l2_fillet_arc".to_string(),
        center: [0.0, 0.0],
        radius: 0.5,
        start_angle: 0.0,
        end_angle: PI / 2.0,
    });
    let err = apply_sketch_fillet_build(&profile, "l1", "l2", 1.0).unwrap_err();
    assert!(matches!(
        err,
        KernelError::InvalidParameter {
            kind: "sketch_fillet_arc_id_collision"
        }
    ));
}

// === T01d: IdGenerator-inclusive build determinism ===
#[test]
fn t01d_build_determinism_with_id_generator() {
    let features = fillet_rect_features("sk", "f1", "ext", 1.0);
    let ref_planes = RefPlane::default_canonical_three();

    let mut gen_a = IdGenerator::new(0);
    let mut gen_b = IdGenerator::new(0);
    let a = build_bodies_from_features(&features, &ref_planes, &mut gen_a).unwrap();
    let b = build_bodies_from_features(&features, &ref_planes, &mut gen_b).unwrap();

    assert_eq!(a.all().len(), b.all().len());
    assert_eq!(a.all().len(), 1);
    let yaml_a = serde_yaml::to_string(&a.all()[0].solid).unwrap();
    let yaml_b = serde_yaml::to_string(&b.all()[0].solid).unwrap();
    assert_eq!(yaml_a, yaml_b);
}

// === T04: rectangle corner fillet → Extrude → manifold ===
#[test]
fn t04_build_rectangle_corner_fillet() {
    let features = fillet_rect_features("sk", "f1", "ext", 1.0);
    let ref_planes = RefPlane::default_canonical_three();
    let mut gen = IdGenerator::new(0);
    let built = build_bodies_from_features(&features, &ref_planes, &mut gen).unwrap();
    assert_eq!(built.all().len(), 1);
    let solid = &built.all()[0].solid;
    assert_eq!(solid.euler_poincare(), 0);
}

/// T02/T03: 90°/60° corner analytic solutions are covered at kernel level
/// (`crates/engawa-kernel/src/geometry/sketch_fillet.rs::tests`).
#[test]
fn t02_normal_90deg_covered_at_kernel_level() {}

/// T03 ditto.
#[test]
fn t03_normal_60deg_covered_at_kernel_level() {}

// === T05: Feature::SketchFillet YAML roundtrip covered at format level ===
/// (`crates/engawa-format/src/feature.rs::tests::t_sketch_fillet_roundtrip`).
#[test]
fn t05_roundtrip_covered_at_format_level() {}

// === T06: fillet inserts Arc → V/E counts grow vs baseline rectangle ===
#[test]
fn t06_extrude_uses_filleted_profile() {
    let ref_planes = RefPlane::default_canonical_three();
    let baseline_features = rect_features("sk", "ext");
    let mut gen = IdGenerator::new(0);
    let baseline = build_bodies_from_features(&baseline_features, &ref_planes, &mut gen).unwrap();
    let baseline_solid = &baseline.all()[0].solid;

    let fillet_features = fillet_rect_features("sk", "f1", "ext", 1.0);
    let mut gen = IdGenerator::new(0);
    let filleted = build_bodies_from_features(&fillet_features, &ref_planes, &mut gen).unwrap();
    let filleted_solid = &filleted.all()[0].solid;

    assert!(filleted_solid.edges.len() > baseline_solid.edges.len());
    assert!(filleted_solid.vertices.len() > baseline_solid.vertices.len());
    assert_eq!(baseline_solid.euler_poincare(), 0);
    assert_eq!(filleted_solid.euler_poincare(), 0);
}

// === T07: closed-loop wraparound corner (l4/l1) ===
#[test]
fn t07_closed_loop_wraparound_corner() {
    let mut features = fillet_rect_features("sk", "f1", "ext", 1.0);
    if let Feature::SketchFillet {
        elem1_id, elem2_id, ..
    } = &mut features[1]
    {
        elem1_id.clear();
        elem1_id.push_str("l4");
        elem2_id.clear();
        elem2_id.push_str("l1");
    }
    let ref_planes = RefPlane::default_canonical_three();
    let mut gen = IdGenerator::new(0);
    let built = build_bodies_from_features(&features, &ref_planes, &mut gen).unwrap();
    assert_eq!(built.all().len(), 1);
    assert_eq!(built.all()[0].solid.euler_poincare(), 0);
}

// === T08: CW-wound profile → negative sweep Arc ===
#[test]
fn t08_cw_profile_negative_sweep() {
    let cw = cw_rect_profile();
    let out = apply_sketch_fillet_build(&cw, "l1", "l2", 1.0).unwrap();
    let (start, end) = out
        .iter()
        .find_map(|e| match e {
            SketchElement::Arc {
                start_angle,
                end_angle,
                ..
            } => Some((*start_angle, *end_angle)),
            _ => None,
        })
        .expect("arc present");
    assert!(end < start, "CW profile → negative sweep (end < start)");

    let features = vec![
        Feature::CreateSketch {
            id: "sk".into(),
            plane: SketchPlane::Xy,
            offset: 0.0,
            variables: vec![],
            profile: cw,
            plane_ref: None,
            suppressed: false,
        },
        Feature::Extrude {
            id: "ext".into(),
            sketch: "sk".into(),
            depth: 3.0,
            fuse_target: None,
            suppressed: false,
        },
    ];
    let ref_planes = RefPlane::default_canonical_three();
    let mut gen = IdGenerator::new(0);
    let built = build_bodies_from_features(&features, &ref_planes, &mut gen).unwrap();
    assert_eq!(built.all().len(), 1);
}

// === T09: profile chain continuity (Line.to / Arc endpoints connect) ===
#[test]
fn t09_profile_chain_continuity() {
    let profile = rect_profile();
    for (a, b) in [("l1", "l2"), ("l4", "l1")] {
        let out = apply_sketch_fillet_build(&profile, a, b, 1.0).unwrap();
        let n = out.len();
        assert!(n >= 2);
        for i in 0..n {
            let j = (i + 1) % n;
            let end_i = endpoint_of(&out[i]);
            let start_j = startpoint_of(&out[j]);
            let d = ((end_i[0] - start_j[0]).powi(2) + (end_i[1] - start_j[1]).powi(2)).sqrt();
            assert!(d <= LENGTH_TOLERANCE, "chain break at i={i}: d={d}");
        }
    }
}

// === T10: CRUD gate rejects CreateSketch rename that breaks SketchFillet ===
#[test]
fn t10_crud_gate_rejects_rename_breaking_fillet() {
    let mut doc = Document::new("t");
    doc.root_component.features = fillet_rect_features("sk", "f1", "ext", 1.0);

    let renamed_profile = vec![
        line("l1b", [0.0, 0.0], [10.0, 0.0]),
        line("l2", [10.0, 0.0], [10.0, 5.0]),
        line("l3", [10.0, 5.0], [0.0, 5.0]),
        line("l4", [0.0, 5.0], [0.0, 0.0]),
    ];
    let new_sketch = Feature::CreateSketch {
        id: "sk".to_string(),
        plane: SketchPlane::Xy,
        offset: 0.0,
        variables: vec![],
        profile: renamed_profile,
        plane_ref: None,
        suppressed: false,
    };

    let err = FeatureCrud::edit(&doc, "sk", new_sketch).unwrap_err();
    assert!(matches!(
        err,
        FeatureCrudError::EditBreaksConsumer {
            broken_consumer_id,
            ..
        } if broken_consumer_id == "f1"
    ));
}

// === T11: CRUD gate rejects insert with missing element ref ===
#[test]
fn t11_crud_gate_rejects_insert_with_missing_element() {
    let mut doc = Document::new("t");
    doc.root_component.features = vec![rect_create_sketch("sk")];

    let bad_fillet = Feature::SketchFillet {
        id: "f1".to_string(),
        sketch: "sk".to_string(),
        elem1_id: "l1".to_string(),
        elem2_id: "nope".to_string(),
        radius: 1.0,
        suppressed: false,
    };
    let err = FeatureCrud::insert(&doc, bad_fillet, 1).unwrap_err();
    match err {
        FeatureCrudError::SketchElementNotResolved {
            reason: "sketch_fillet_elem_not_found",
            elem1_id,
            elem2_id,
            sketch_ref,
            feature_id,
        } if elem1_id == "l1" && elem2_id == "nope" && sketch_ref == "sk" && feature_id == "f1" => {
        }
        other => panic!("expected SketchElementNotResolved(elem_not_found), got {other:?}"),
    }
}

// === T12: CRUD gate rejects reorder that breaks adjacency ===
#[test]
fn t12_crud_gate_rejects_reorder_breaking_adjacency() {
    let mut doc = Document::new("t");
    doc.root_component.features = fillet_rect_features("sk", "f1", "ext", 1.0);

    // Reorder: [l1, l3, l2, l4] — l1 and l2 are no longer adjacent.
    let reordered = vec![
        line("l1", [0.0, 0.0], [10.0, 0.0]),
        line("l3", [10.0, 5.0], [0.0, 5.0]),
        line("l2", [10.0, 0.0], [10.0, 5.0]),
        line("l4", [0.0, 5.0], [0.0, 0.0]),
    ];
    let new_sketch = Feature::CreateSketch {
        id: "sk".to_string(),
        plane: SketchPlane::Xy,
        offset: 0.0,
        variables: vec![],
        profile: reordered,
        plane_ref: None,
        suppressed: false,
    };

    let err = FeatureCrud::edit(&doc, "sk", new_sketch).unwrap_err();
    assert!(matches!(
        err,
        FeatureCrudError::EditBreaksConsumer {
            broken_consumer_id,
            ..
        } if broken_consumer_id == "f1"
    ));
}

// === T_DEG_fillet_too_large: radius exceeds tangent length budget ===
#[test]
fn t_deg_fillet_too_large() {
    let profile = vec![
        line("l1", [0.0, 0.0], [1.0, 0.0]),
        line("l2", [1.0, 0.0], [1.0, 1.0]),
    ];
    let err = apply_sketch_fillet_build(&profile, "l1", "l2", 10.0).unwrap_err();
    assert!(matches!(
        err,
        KernelError::FilletRadiusTooLarge {
            elem1_id,
            elem2_id,
            radius: 10.0,
        } if elem1_id == "l1" && elem2_id == "l2"
    ));
}

// === T_DEG_corner_angle_flat: ~180° corner (collinear) ===
#[test]
fn t_deg_corner_angle_flat() {
    let profile = vec![
        line("l1", [0.0, 0.0], [5.0, 0.0]),
        line("l2", [5.0, 0.0], [5.0 + 1e-12, 0.0]),
    ];
    let err = apply_sketch_fillet_build(&profile, "l1", "l2", 0.5).unwrap_err();
    assert!(matches!(
        err,
        KernelError::DegenerateSketchElement { reason, .. }
            if reason == "fillet_corner_angle_degenerate"
                || reason == "fillet_zero_length_input_line"
    ));
}

// === T_DEG_corner_angle_zero: ~0° corner (fold-back) ===
#[test]
fn t_deg_corner_angle_zero() {
    let profile = vec![
        line("l1", [-5.0, 0.0], [0.0, 0.0]),
        line("l2", [0.0, 0.0], [-5.0, 0.0]),
    ];
    let err = apply_sketch_fillet_build(&profile, "l1", "l2", 0.5).unwrap_err();
    assert!(matches!(
        err,
        KernelError::DegenerateSketchElement {
            reason: "fillet_corner_angle_degenerate",
            ..
        }
    ));
}

// === T_DEG_no_shared_corner: a.to != b.from ===
#[test]
fn t_deg_no_shared_corner() {
    let profile = vec![
        line("l1", [0.0, 0.0], [5.0, 0.0]),
        line("l2", [6.0, 0.0], [6.0, 5.0]),
    ];
    let err = apply_sketch_fillet_build(&profile, "l1", "l2", 0.5).unwrap_err();
    assert!(matches!(
        err,
        KernelError::InvalidParameter {
            kind: "sketch_fillet_no_shared_corner"
        }
    ));
}

// === T_DEG_non_line_element: elem pointing at Circle ===
#[test]
fn t_deg_non_line_element_rejected() {
    let mut profile = rect_profile();
    profile[1] = SketchElement::Circle {
        id: "c1".to_string(),
        center: [0.0, 0.0],
        radius: 1.0,
    };
    let err = apply_sketch_fillet_build(&profile, "l1", "c1", 0.5).unwrap_err();
    assert!(matches!(
        err,
        KernelError::UnsupportedFeature {
            kind: "sketch_fillet_only_line_line"
        }
    ));
}

// === T_DEG_same_element: elem1_id == elem2_id ===
#[test]
fn t_deg_same_element_rejected() {
    let profile = rect_profile();
    let err = apply_sketch_fillet_build(&profile, "l1", "l1", 0.5).unwrap_err();
    assert!(matches!(
        err,
        KernelError::InvalidParameter {
            kind: "sketch_fillet_same_element"
        }
    ));
}

// === T_DEG_not_adjacent: gap of one element between pair ===
#[test]
fn t_deg_not_adjacent_rejected() {
    let profile = rect_profile();
    let err = apply_sketch_fillet_build(&profile, "l1", "l3", 0.5).unwrap_err();
    assert!(matches!(
        err,
        KernelError::InvalidParameter {
            kind: "sketch_fillet_elems_not_adjacent"
        }
    ));
}

// === T_DEG_elem_not_found: unknown element ID ===
#[test]
fn t_deg_elem_not_found() {
    let profile = rect_profile();
    let err = apply_sketch_fillet_build(&profile, "l1", "nope", 0.5).unwrap_err();
    assert!(matches!(
        err,
        KernelError::InvalidParameter {
            kind: "sketch_fillet_elem_not_found"
        }
    ));
}

// === T_DEG_negative_radius ===
#[test]
fn t_deg_negative_radius() {
    let profile = rect_profile();
    let err = apply_sketch_fillet_build(&profile, "l1", "l2", -1.0).unwrap_err();
    assert!(matches!(
        err,
        KernelError::InvalidParameter { kind: "radius" }
    ));
}

// === T_DEG_nan_radius ===
#[test]
fn t_deg_nan_radius() {
    let profile = rect_profile();
    let err = apply_sketch_fillet_build(&profile, "l1", "l2", f64::NAN).unwrap_err();
    assert!(matches!(
        err,
        KernelError::InvalidParameter { kind: "radius" }
    ));
}

// === T_DEG_sketch_ref_not_found: build path, sketch id missing ===
#[test]
fn t_deg_sketch_ref_not_found() {
    let features = vec![Feature::SketchFillet {
        id: "f1".to_string(),
        sketch: "missing_sketch".to_string(),
        elem1_id: "l1".to_string(),
        elem2_id: "l2".to_string(),
        radius: 1.0,
        suppressed: false,
    }];
    let ref_planes = RefPlane::default_canonical_three();
    let mut gen = IdGenerator::new(0);
    let err = build_bodies_from_features(&features, &ref_planes, &mut gen).unwrap_err();
    assert!(matches!(
        err,
        KernelError::SketchNotFound {
            sketch
        } if sketch == "missing_sketch"
    ));
}

// ---------- helpers ----------

fn line(id: &str, from: [f64; 2], to: [f64; 2]) -> SketchElement {
    SketchElement::Line {
        id: id.to_string(),
        from,
        to,
    }
}

/// CCW 4-Line rectangle: (0,0)→(10,0)→(10,5)→(0,5)→(0,0).
fn rect_profile() -> Vec<SketchElement> {
    vec![
        line("l1", [0.0, 0.0], [10.0, 0.0]),
        line("l2", [10.0, 0.0], [10.0, 5.0]),
        line("l3", [10.0, 5.0], [0.0, 5.0]),
        line("l4", [0.0, 5.0], [0.0, 0.0]),
    ]
}

/// CW 4-Line rectangle: (0,0)→(0,5)→(10,5)→(10,0)→(0,0).
fn cw_rect_profile() -> Vec<SketchElement> {
    vec![
        line("l1", [0.0, 0.0], [0.0, 5.0]),
        line("l2", [0.0, 5.0], [10.0, 5.0]),
        line("l3", [10.0, 5.0], [10.0, 0.0]),
        line("l4", [10.0, 0.0], [0.0, 0.0]),
    ]
}

fn rect_create_sketch(id: &str) -> Feature {
    Feature::CreateSketch {
        id: id.to_string(),
        plane: SketchPlane::Xy,
        offset: 0.0,
        variables: vec![],
        profile: rect_profile(),
        plane_ref: None,
        suppressed: false,
    }
}

/// `[CreateSketch(sk, rectangle), SketchFillet(f1, sk, l1, l2, radius), Extrude(ext, sk)]`
fn fillet_rect_features(sk: &str, f1: &str, ext: &str, radius: f64) -> Vec<Feature> {
    vec![
        rect_create_sketch(sk),
        Feature::SketchFillet {
            id: f1.to_string(),
            sketch: sk.to_string(),
            elem1_id: "l1".to_string(),
            elem2_id: "l2".to_string(),
            radius,
            suppressed: false,
        },
        Feature::Extrude {
            id: ext.to_string(),
            sketch: sk.to_string(),
            depth: 3.0,
            fuse_target: None,
            suppressed: false,
        },
    ]
}

/// `[CreateSketch(sk, rectangle), Extrude(ext, sk)]` — no fillet, for baseline comparison.
fn rect_features(sk: &str, ext: &str) -> Vec<Feature> {
    vec![
        rect_create_sketch(sk),
        Feature::Extrude {
            id: ext.to_string(),
            sketch: sk.to_string(),
            depth: 3.0,
            fuse_target: None,
            suppressed: false,
        },
    ]
}

fn endpoint_of(elem: &SketchElement) -> [f64; 2] {
    match elem {
        SketchElement::Line { to, .. } => *to,
        SketchElement::Arc {
            center,
            radius,
            end_angle,
            ..
        } => [
            center[0] + radius * end_angle.cos(),
            center[1] + radius * end_angle.sin(),
        ],
        _ => panic!("non-Line/Arc element"),
    }
}

fn startpoint_of(elem: &SketchElement) -> [f64; 2] {
    match elem {
        SketchElement::Line { from, .. } => *from,
        SketchElement::Arc {
            center,
            radius,
            start_angle,
            ..
        } => [
            center[0] + radius * start_angle.cos(),
            center[1] + radius * start_angle.sin(),
        ],
        _ => panic!("non-Line/Arc element"),
    }
}
