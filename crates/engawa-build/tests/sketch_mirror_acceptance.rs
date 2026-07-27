//! Acceptance tests for #298 (Sketch Mirror)
//! テスト計画: features/298-phase10-sketch-mirror-engawa/plan.md 「## テスト計画（ID 付き）」参照

use engawa_build::build_bodies_from_features;
use engawa_build::{FeatureCrud, FeatureCrudError};
use engawa_format::{Document, Feature, RefPlane, SketchElement, SketchPlane};
use engawa_kernel::brep::topology::IdGenerator;
use engawa_kernel::error::KernelError;
use engawa_kernel::geometry::math::LENGTH_TOLERANCE;
use engawa_kernel::geometry::sketch_mirror::apply_sketch_mirror;
use std::f64::consts::{FRAC_PI_2, FRAC_PI_4, PI};

#[test]
fn t01_determinism() {
    let profile = vec![
        SketchElement::Line {
            id: "l1".to_string(),
            from: [2.0, 0.0],
            to: [2.0, 3.0],
        },
        SketchElement::Circle {
            id: "c1".to_string(),
            center: [3.0, 4.0],
            radius: 2.0,
        },
    ];
    let out1 = apply_sketch_mirror(&profile, [0.0, 0.0], [0.0, 1.0], &[]).unwrap();
    let out2 = apply_sketch_mirror(&profile, [0.0, 0.0], [0.0, 1.0], &[]).unwrap();
    assert_eq!(out1, out2);
    assert_eq!(out1.len(), 4);
}

#[test]
fn t02_normal_line_mirror_y_axis() {
    let profile = vec![SketchElement::Line {
        id: "l1".to_string(),
        from: [2.0, 0.0],
        to: [2.0, 3.0],
    }];
    let out = apply_sketch_mirror(&profile, [0.0, 0.0], [0.0, 1.0], &[]).unwrap();
    assert_eq!(out.len(), 2);
    match &out[1] {
        SketchElement::Line { id, from, to } => {
            assert_eq!(id, "l1_mirror");
            assert!((from[0] - (-2.0)).abs() < LENGTH_TOLERANCE);
            assert!((from[1] - 0.0).abs() < LENGTH_TOLERANCE);
            assert!((to[0] - (-2.0)).abs() < LENGTH_TOLERANCE);
            assert!((to[1] - 3.0).abs() < LENGTH_TOLERANCE);
        }
        _ => panic!("expected mirrored Line"),
    }
}

#[test]
fn t03_normal_circle_mirror() {
    let profile = vec![SketchElement::Circle {
        id: "c1".to_string(),
        center: [3.0, 4.0],
        radius: 2.0,
    }];
    let out = apply_sketch_mirror(&profile, [0.0, 0.0], [1.0, 0.0], &[]).unwrap();
    match &out[1] {
        SketchElement::Circle { id, center, radius } => {
            assert_eq!(id, "c1_mirror");
            assert!((center[0] - 3.0).abs() < LENGTH_TOLERANCE);
            assert!((center[1] - (-4.0)).abs() < LENGTH_TOLERANCE);
            assert!((radius - 2.0).abs() < LENGTH_TOLERANCE);
        }
        _ => panic!("expected mirrored Circle"),
    }
}

#[test]
fn t04_normal_arc_mirror() {
    let profile = vec![SketchElement::Arc {
        id: "a1".to_string(),
        center: [0.0, 0.0],
        radius: 5.0,
        start_angle: 0.0,
        end_angle: FRAC_PI_2,
    }];
    let out = apply_sketch_mirror(&profile, [0.0, 0.0], [1.0, 0.0], &[]).unwrap();
    match &out[1] {
        SketchElement::Arc {
            start_angle,
            end_angle,
            ..
        } => {
            assert!((start_angle - 0.0).abs() < LENGTH_TOLERANCE);
            assert!((end_angle - (-FRAC_PI_2)).abs() < LENGTH_TOLERANCE);
        }
        _ => panic!("expected mirrored Arc"),
    }
}

#[test]
fn t05_boundary_empty_selection_mirrors_all() {
    let profile = vec![
        SketchElement::Line {
            id: "l1".to_string(),
            from: [2.0, 0.0],
            to: [2.0, 3.0],
        },
        SketchElement::Circle {
            id: "c1".to_string(),
            center: [3.0, 4.0],
            radius: 2.0,
        },
        SketchElement::Arc {
            id: "a1".to_string(),
            center: [0.0, 0.0],
            radius: 5.0,
            start_angle: 0.0,
            end_angle: FRAC_PI_2,
        },
    ];
    let out = apply_sketch_mirror(&profile, [0.0, 0.0], [0.0, 1.0], &[]).unwrap();
    assert_eq!(out.len(), profile.len() * 2);
}

/// T_DEG_mirror_on_axis (Line): line on x-axis → reflection coincides with itself.
#[test]
fn t_deg_mirror_on_axis_line() {
    let profile = vec![SketchElement::Line {
        id: "l1".to_string(),
        from: [0.0, 0.0],
        to: [5.0, 0.0],
    }];
    let err = apply_sketch_mirror(&profile, [0.0, 0.0], [1.0, 0.0], &[]).unwrap_err();
    assert!(matches!(
        err,
        KernelError::DegenerateSketchElement {
            element_id,
            reason: "mirror_axis_coincident",
        } if element_id == "l1"
    ));
}

/// T_DEG_mirror_on_axis_arc: symmetric Arc across x-axis → self-coincident.
#[test]
fn t_deg_mirror_on_axis_arc() {
    let profile = vec![SketchElement::Arc {
        id: "a1".to_string(),
        center: [0.0, 0.0],
        radius: 1.0,
        start_angle: -FRAC_PI_4,
        end_angle: FRAC_PI_4,
    }];
    let err = apply_sketch_mirror(&profile, [0.0, 0.0], [1.0, 0.0], &[]).unwrap_err();
    assert!(matches!(
        err,
        KernelError::DegenerateSketchElement {
            reason: "mirror_axis_coincident",
            ..
        }
    ));
}

/// T_boundary_arc_endpoints_on_axis: half-circle endpoints on axis. Sweep inverts but the
/// arc is NOT self-coincident → must succeed with start=0, end=-π.
#[test]
fn t_boundary_arc_endpoints_on_axis() {
    let profile = vec![SketchElement::Arc {
        id: "a1".to_string(),
        center: [0.0, 0.0],
        radius: 1.0,
        start_angle: 0.0,
        end_angle: PI,
    }];
    let out = apply_sketch_mirror(&profile, [0.0, 0.0], [1.0, 0.0], &[]).unwrap();
    assert_eq!(out.len(), 2);
    match &out[1] {
        SketchElement::Arc {
            start_angle,
            end_angle,
            ..
        } => {
            assert!((start_angle - 0.0).abs() < LENGTH_TOLERANCE);
            assert!((end_angle - (-PI)).abs() < LENGTH_TOLERANCE);
        }
        _ => panic!("expected mirrored Arc"),
    }
}

/// T_DEG_axis_degenerate (same point).
#[test]
fn t_deg_axis_degenerate_same_point() {
    let profile = vec![SketchElement::Line {
        id: "l1".to_string(),
        from: [2.0, 0.0],
        to: [2.0, 3.0],
    }];
    let err = apply_sketch_mirror(&profile, [0.0, 0.0], [0.0, 0.0], &[]).unwrap_err();
    assert!(matches!(
        err,
        KernelError::InvalidParameter {
            kind: "sketch_mirror_axis_degenerate"
        }
    ));
}

/// T_DEG_axis_degenerate (NaN in axis).
#[test]
fn t_deg_axis_degenerate_nan() {
    let profile = vec![SketchElement::Line {
        id: "l1".to_string(),
        from: [2.0, 0.0],
        to: [2.0, 3.0],
    }];
    let err = apply_sketch_mirror(&profile, [f64::NAN, 0.0], [0.0, 1.0], &[]).unwrap_err();
    assert!(matches!(
        err,
        KernelError::InvalidParameter {
            kind: "sketch_mirror_axis_degenerate"
        }
    ));
}

/// T_DEG_axis_degenerate (Inf in axis).
#[test]
fn t_deg_axis_degenerate_inf() {
    let profile = vec![SketchElement::Line {
        id: "l1".to_string(),
        from: [2.0, 0.0],
        to: [2.0, 3.0],
    }];
    let err = apply_sketch_mirror(&profile, [0.0, 0.0], [f64::INFINITY, 0.0], &[]).unwrap_err();
    assert!(matches!(
        err,
        KernelError::InvalidParameter {
            kind: "sketch_mirror_axis_degenerate"
        }
    ));
}

/// T_DEG_unknown_element: selection id not present in profile.
#[test]
fn t_deg_unknown_element() {
    let profile = vec![SketchElement::Line {
        id: "l1".to_string(),
        from: [2.0, 0.0],
        to: [2.0, 3.0],
    }];
    let err = apply_sketch_mirror(&profile, [0.0, 0.0], [0.0, 1.0], &["missing".to_string()])
        .unwrap_err();
    assert!(matches!(
        err,
        KernelError::InvalidParameter {
            kind: "sketch_mirror_unknown_element_id"
        }
    ));
}

/// T_DEG_unsupported_type (Ellipse).
#[test]
fn t_deg_unsupported_type_ellipse() {
    let profile = vec![SketchElement::Ellipse {
        id: "e1".to_string(),
        center: [0.0, 0.0],
        major: 2.0,
        minor: 1.0,
        rotation: 0.0,
    }];
    let err =
        apply_sketch_mirror(&profile, [0.0, 0.0], [0.0, 1.0], &["e1".to_string()]).unwrap_err();
    assert!(matches!(
        err,
        KernelError::UnsupportedFeature {
            kind: "sketch_mirror_of_ellipse_or_conic"
        }
    ));
}

/// T_DEG_unsupported_type (Conic).
#[test]
fn t_deg_unsupported_type_conic() {
    let profile = vec![SketchElement::Conic {
        id: "cn1".to_string(),
        coeffs: [1.0, 0.0, 1.0, 0.0, 0.0],
    }];
    let err =
        apply_sketch_mirror(&profile, [0.0, 0.0], [0.0, 1.0], &["cn1".to_string()]).unwrap_err();
    assert!(matches!(
        err,
        KernelError::UnsupportedFeature {
            kind: "sketch_mirror_of_ellipse_or_conic"
        }
    ));
}

/// T_DEG_duplicate_id: derived "{elem_id}_mirror" already present in profile.
#[test]
fn t_deg_duplicate_id() {
    let profile = vec![
        SketchElement::Line {
            id: "l1".to_string(),
            from: [2.0, 0.0],
            to: [2.0, 3.0],
        },
        SketchElement::Line {
            id: "l1_mirror".to_string(),
            from: [-2.0, 0.0],
            to: [-2.0, 3.0],
        },
    ];
    let err =
        apply_sketch_mirror(&profile, [0.0, 0.0], [0.0, 1.0], &["l1".to_string()]).unwrap_err();
    assert!(matches!(
        err,
        KernelError::DegenerateSketchElement {
            element_id,
            reason: "mirror_duplicate_id",
        } if element_id == "l1_mirror"
    ));
}

/// T_DEG_sketch_ref_not_found: build pipeline rejects unknown sketch reference.
#[test]
fn t_deg_sketch_ref_not_found() {
    // CreateSketch profile must be closed (single Circle is closed; a single Line would be
    // rejected by validate_profile_closed before SketchMirror runs).
    let features = vec![
        Feature::CreateSketch {
            id: "sk1".to_string(),
            plane: SketchPlane::Xy,
            offset: 0.0,
            variables: Vec::new(),
            profile: vec![SketchElement::Circle {
                id: "c1".to_string(),
                center: [3.0, 0.0],
                radius: 1.0,
            }],
            plane_ref: None,
            suppressed: false,
        },
        Feature::SketchMirror {
            id: "m1".to_string(),
            sketch: "missing_sketch".to_string(),
            axis_p1: [0.0, 0.0],
            axis_p2: [0.0, 1.0],
            selection: Vec::new(),
            suppressed: false,
        },
    ];
    let ref_planes = RefPlane::default_canonical_three();
    let mut gen = IdGenerator::new(0);
    let err = build_bodies_from_features(&features, &ref_planes, &mut gen).unwrap_err();
    assert!(matches!(
        err,
        KernelError::SketchNotFound { sketch } if sketch == "missing_sketch"
    ));
}

/// T07_crud_gate_delete_sketch_breaks_mirror:
/// SketchMirror が参照している CreateSketch を delete しようとすると EditBreaksConsumer で拒否される。
/// これは `feature_crud::refs_resolve_in_state` が SketchMirror arm を持つことで
/// `simulate_history` → `check_edit_preserves_consumers` 経由で保護される。
#[test]
fn t07_crud_gate_delete_sketch_breaks_mirror() {
    let mut doc = Document::new("t");
    doc.root_component.features = vec![
        Feature::CreateSketch {
            id: "sk1".to_string(),
            plane: SketchPlane::Xy,
            offset: 0.0,
            variables: Vec::new(),
            profile: vec![SketchElement::Circle {
                id: "c1".to_string(),
                center: [3.0, 0.0],
                radius: 1.0,
            }],
            plane_ref: None,
            suppressed: false,
        },
        Feature::SketchMirror {
            id: "m1".to_string(),
            sketch: "sk1".to_string(),
            axis_p1: [0.0, 0.0],
            axis_p2: [0.0, 1.0],
            selection: Vec::new(),
            suppressed: false,
        },
    ];
    let err = FeatureCrud::delete(&doc, "sk1").unwrap_err();
    assert!(matches!(
        err,
        FeatureCrudError::EditBreaksConsumer {
            broken_consumer_id,
            ..
        } if broken_consumer_id == "m1"
    ));
}

/// T08_crud_gate_suppress_sketch_breaks_mirror:
/// SketchMirror が参照している CreateSketch を suppress しようとしても EditBreaksConsumer で拒否される。
#[test]
fn t08_crud_gate_suppress_sketch_breaks_mirror() {
    let mut doc = Document::new("t");
    doc.root_component.features = vec![
        Feature::CreateSketch {
            id: "sk1".to_string(),
            plane: SketchPlane::Xy,
            offset: 0.0,
            variables: Vec::new(),
            profile: vec![SketchElement::Circle {
                id: "c1".to_string(),
                center: [3.0, 0.0],
                radius: 1.0,
            }],
            plane_ref: None,
            suppressed: false,
        },
        Feature::SketchMirror {
            id: "m1".to_string(),
            sketch: "sk1".to_string(),
            axis_p1: [0.0, 0.0],
            axis_p2: [0.0, 1.0],
            selection: Vec::new(),
            suppressed: false,
        },
    ];
    let err = FeatureCrud::suppress(&doc, "sk1", true).unwrap_err();
    assert!(matches!(
        err,
        FeatureCrudError::EditBreaksConsumer {
            broken_consumer_id,
            ..
        } if broken_consumer_id == "m1"
    ));
}

/// T09_crud_gate_insert_mirror_unknown_sketch:
/// 存在しない sketch を参照する SketchMirror の insert は SketchNotFound で拒否される。
#[test]
fn t09_crud_gate_insert_mirror_unknown_sketch() {
    let mut doc = Document::new("t");
    doc.root_component.features = vec![Feature::CreateSketch {
        id: "sk1".to_string(),
        plane: SketchPlane::Xy,
        offset: 0.0,
        variables: Vec::new(),
        profile: vec![SketchElement::Circle {
            id: "c1".to_string(),
            center: [3.0, 0.0],
            radius: 1.0,
        }],
        plane_ref: None,
        suppressed: false,
    }];
    let bad_mirror = Feature::SketchMirror {
        id: "m1".to_string(),
        sketch: "missing_sketch".to_string(),
        axis_p1: [0.0, 0.0],
        axis_p2: [0.0, 1.0],
        selection: Vec::new(),
        suppressed: false,
    };
    let err = FeatureCrud::insert(&doc, bad_mirror, 1).unwrap_err();
    assert!(matches!(
        err,
        FeatureCrudError::SketchNotFound { sketch_ref, .. } if sketch_ref == "missing_sketch"
    ));
}

/// T10_crud_gate_insert_mirror_unknown_element:
/// selection に profile 内へ存在しない id を指定した SketchMirror の insert は
/// SketchElementNotResolved で拒否される (element-level gate, Item 2)。
#[test]
fn t10_crud_gate_insert_mirror_unknown_element() {
    let mut doc = Document::new("t");
    doc.root_component.features = vec![Feature::CreateSketch {
        id: "sk1".to_string(),
        plane: SketchPlane::Xy,
        offset: 0.0,
        variables: Vec::new(),
        profile: vec![SketchElement::Circle {
            id: "c1".to_string(),
            center: [3.0, 0.0],
            radius: 1.0,
        }],
        plane_ref: None,
        suppressed: false,
    }];
    let bad_mirror = Feature::SketchMirror {
        id: "m1".to_string(),
        sketch: "sk1".to_string(),
        axis_p1: [0.0, 0.0],
        axis_p2: [0.0, 1.0],
        selection: vec!["nope".to_string()],
        suppressed: false,
    };
    let err = FeatureCrud::insert(&doc, bad_mirror, 1).unwrap_err();
    match err {
        FeatureCrudError::SketchElementNotResolved {
            reason,
            elem1_id,
            sketch_ref,
            feature_id,
            ..
        } if reason == "sketch_mirror_elem_not_found"
            && elem1_id == "nope"
            && sketch_ref == "sk1"
            && feature_id == "m1" => {}
        other => panic!("expected SketchElementNotResolved, got {other:?}"),
    }
}

/// T11_crud_gate_edit_breaks_mirror_selection (Codex #299 STEP 7.5 A01 regression):
/// SketchMirror の `selection` が参照している要素を `CreateSketch` の edit で rename すると
/// `EditBreaksConsumer` で拒否される。修正前は `refs_resolve_in_state` の SketchMirror arm が
/// sketch id の存在だけを見ていたため edit が成功し、後続の `build_bodies_from_features` で
/// 初めて `sketch_mirror_unknown_element_id` になっていた (false-accept 非対称)。
/// SketchFillet の T10 (`sketch_fillet_acceptance.rs`) と同じ契約。
#[test]
fn t11_crud_gate_edit_breaks_mirror_selection() {
    let sk = |elem_id: &str| Feature::CreateSketch {
        id: "sk1".to_string(),
        plane: SketchPlane::Xy,
        offset: 0.0,
        variables: Vec::new(),
        profile: vec![SketchElement::Circle {
            id: elem_id.to_string(),
            center: [3.0, 0.0],
            radius: 1.0,
        }],
        plane_ref: None,
        suppressed: false,
    };
    let mut doc = Document::new("t");
    doc.root_component.features = vec![
        sk("c1"),
        Feature::SketchMirror {
            id: "m1".to_string(),
            sketch: "sk1".to_string(),
            axis_p1: [0.0, 0.0],
            axis_p2: [0.0, 1.0],
            selection: vec!["c1".to_string()],
            suppressed: false,
        },
    ];
    let err = FeatureCrud::edit(&doc, "sk1", sk("c_renamed")).unwrap_err();
    assert!(
        matches!(
            &err,
            FeatureCrudError::EditBreaksConsumer { broken_consumer_id, .. }
                if broken_consumer_id == "m1"
        ),
        "expected EditBreaksConsumer(m1), got {err:?}"
    );
}

/// T_known_limitation_mirror_derived_elem_false_reject:
/// 先行 SketchFillet が挿入した派生 Arc (`l1_l2_fillet_arc`) を SketchMirror の
/// selection に指定した場合、build は current profile 基準で成功するが CRUD gate は
/// 元の CreateSketch profile 基準で照合するため `sketch_mirror_elem_not_found` で拒否される
/// (既知の false-reject 制約、`feature_crud.rs` の SketchMirror element-level gate コメント
/// および plan.md Non-Goals 参照。Fillet/Chamfer と同型で Issue #331 で横断対応予定)。
#[test]
fn t_known_limitation_mirror_derived_elem_false_reject() {
    // Original profile: closed rectangle (l1..l4).
    let rect = vec![
        SketchElement::Line {
            id: "l1".to_string(),
            from: [0.0, 0.0],
            to: [10.0, 0.0],
        },
        SketchElement::Line {
            id: "l2".to_string(),
            from: [10.0, 0.0],
            to: [10.0, 5.0],
        },
        SketchElement::Line {
            id: "l3".to_string(),
            from: [10.0, 5.0],
            to: [0.0, 5.0],
        },
        SketchElement::Line {
            id: "l4".to_string(),
            from: [0.0, 5.0],
            to: [0.0, 0.0],
        },
    ];
    let create_sk = Feature::CreateSketch {
        id: "sk1".to_string(),
        plane: SketchPlane::Xy,
        offset: 0.0,
        variables: Vec::new(),
        profile: rect,
        plane_ref: None,
        suppressed: false,
    };
    let fillet = Feature::SketchFillet {
        id: "f1".to_string(),
        sketch: "sk1".to_string(),
        elem1_id: "l1".to_string(),
        elem2_id: "l2".to_string(),
        radius: 1.0,
        suppressed: false,
    };
    let mirror = Feature::SketchMirror {
        id: "m1".to_string(),
        sketch: "sk1".to_string(),
        axis_p1: [0.0, 0.0],
        axis_p2: [0.0, 1.0],
        selection: vec!["l1_l2_fillet_arc".to_string()],
        suppressed: false,
    };

    // (a) build path: SketchFillet inserts `l1_l2_fillet_arc` into the current profile,
    // then SketchMirror resolves that id against the current profile and mirrors it.
    let features_build = vec![create_sk.clone(), fillet.clone(), mirror.clone()];
    let ref_planes = RefPlane::default_canonical_three();
    let mut gen = IdGenerator::new(0);
    let build_result = build_bodies_from_features(&features_build, &ref_planes, &mut gen);
    assert!(
        build_result.is_ok(),
        "build should accept derived fillet_arc ref, got {build_result:?}"
    );

    // (b) CRUD gate: SketchMirror element-level gate inspects the *original* CreateSketch
    // profile, which contains only l1..l4 — `l1_l2_fillet_arc` is not present, so the gate
    // rejects with `sketch_mirror_elem_not_found` even though build would succeed.
    let mut doc = Document::new("t");
    doc.root_component.features = vec![create_sk, fillet];
    let err = FeatureCrud::insert(&doc, mirror, 2).unwrap_err();
    match err {
        FeatureCrudError::SketchElementNotResolved {
            reason,
            elem1_id,
            sketch_ref,
            feature_id,
            ..
        } if reason == "sketch_mirror_elem_not_found"
            && elem1_id == "l1_l2_fillet_arc"
            && sketch_ref == "sk1"
            && feature_id == "m1" => {}
        other => panic!("expected SketchElementNotResolved, got {other:?}"),
    }
}
