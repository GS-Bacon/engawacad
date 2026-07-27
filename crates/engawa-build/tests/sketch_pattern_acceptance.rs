//! Acceptance tests for #299 (Sketch Pattern: PatternLinear / PatternCircular)
//! テスト計画: features/299-sketch-pattern/plan.md 「## テスト計画（ID 付き）」参照
//!
//! コアテスト (T01-T05 + T_CRUD_*) のみ STEP 6 で実装。エッジケース (T_DEG_*) は
//! `#[ignore]` のままで後続の glm-test-implementer フェーズに委譲する。

use engawa_build::build_bodies_from_features;
use engawa_build::{FeatureCrud, FeatureCrudError};
use engawa_format::{Document, Feature, RefPlane, SketchElement, SketchPlane};
use engawa_kernel::brep::topology::IdGenerator;
use engawa_kernel::error::KernelError;
use engawa_kernel::geometry::math::LENGTH_TOLERANCE;
use engawa_kernel::geometry::sketch_pattern::{
    apply_sketch_pattern_circular, apply_sketch_pattern_linear, MAX_PATTERN_COUNT,
};
use std::f64::consts::{FRAC_PI_2, PI, TAU};

/// CRUD テスト用の最小 CreateSketch (Circle 1 個のみ)。
fn sketch_with_circle(sketch_id: &str, elem_id: &str) -> Feature {
    Feature::CreateSketch {
        id: sketch_id.to_string(),
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
    }
}

#[test]
fn t01_determinism() {
    let profile = vec![
        SketchElement::Line {
            id: "l1".to_string(),
            from: [0.0, 0.0],
            to: [2.0, 0.0],
        },
        SketchElement::Circle {
            id: "c1".to_string(),
            center: [3.0, 0.0],
            radius: 1.0,
        },
        SketchElement::Arc {
            id: "a1".to_string(),
            center: [0.0, 0.0],
            radius: 5.0,
            start_angle: 0.0,
            end_angle: FRAC_PI_2,
        },
    ];
    let lin1 = apply_sketch_pattern_linear(&profile, &[], 3, [1.0, 0.0], 3.0).unwrap();
    let lin2 = apply_sketch_pattern_linear(&profile, &[], 3, [1.0, 0.0], 3.0).unwrap();
    assert_eq!(lin1, lin2);
    let cir1 = apply_sketch_pattern_circular(&profile, &[], [0.0, 0.0], 4, TAU).unwrap();
    let cir2 = apply_sketch_pattern_circular(&profile, &[], [0.0, 0.0], 4, TAU).unwrap();
    assert_eq!(cir1, cir2);
    // Document-level determinism: same features → same build outcome.
    let features = vec![
        Feature::CreateSketch {
            id: "sk1".to_string(),
            plane: SketchPlane::Xy,
            offset: 0.0,
            variables: Vec::new(),
            profile: profile.clone(),
            plane_ref: None,
            suppressed: false,
        },
        Feature::SketchPatternLinear {
            id: "pl1".to_string(),
            sketch: "sk1".to_string(),
            selection: Vec::new(),
            count: 3,
            direction: [1.0, 0.0],
            distance: 3.0,
            suppressed: false,
        },
        Feature::SketchPatternCircular {
            id: "pc1".to_string(),
            sketch: "sk1".to_string(),
            selection: Vec::new(),
            center: [0.0, 0.0],
            count: 4,
            total_angle: TAU,
            suppressed: false,
        },
    ];
    let ref_planes = RefPlane::default_canonical_three();
    let mut gen_a = IdGenerator::new(0);
    let mut gen_b = IdGenerator::new(0);
    let a = build_bodies_from_features(&features, &ref_planes, &mut gen_a).unwrap();
    let b = build_bodies_from_features(&features, &ref_planes, &mut gen_b).unwrap();
    assert_eq!(a.len(), b.len());
}

#[test]
fn t02_linear_line_normal() {
    let profile = vec![SketchElement::Line {
        id: "l1".to_string(),
        from: [0.0, 0.0],
        to: [2.0, 0.0],
    }];
    let out = apply_sketch_pattern_linear(&profile, &[], 3, [1.0, 0.0], 3.0).unwrap();
    assert_eq!(out.len(), 3);
    match &out[1] {
        SketchElement::Line { id, from, to } => {
            assert_eq!(id, "l1_pattern_linear_1");
            assert!((from[0] - 3.0).abs() < LENGTH_TOLERANCE);
            assert!((from[1] - 0.0).abs() < LENGTH_TOLERANCE);
            assert!((to[0] - 5.0).abs() < LENGTH_TOLERANCE);
            assert!((to[1] - 0.0).abs() < LENGTH_TOLERANCE);
        }
        _ => panic!("expected copy-1 Line"),
    }
    match &out[2] {
        SketchElement::Line { id, from, to } => {
            assert_eq!(id, "l1_pattern_linear_2");
            assert!((from[0] - 6.0).abs() < LENGTH_TOLERANCE);
            assert!((to[0] - 8.0).abs() < LENGTH_TOLERANCE);
        }
        _ => panic!("expected copy-2 Line"),
    }
}

#[test]
fn t03_circular_circle_normal() {
    let profile = vec![SketchElement::Circle {
        id: "c1".to_string(),
        center: [3.0, 0.0],
        radius: 1.0,
    }];
    let out = apply_sketch_pattern_circular(&profile, &[], [0.0, 0.0], 4, TAU).unwrap();
    assert_eq!(out.len(), 4);
    match &out[1] {
        SketchElement::Circle { id, center, radius } => {
            assert_eq!(id, "c1_pattern_circular_1");
            assert!((center[0] - 0.0).abs() < LENGTH_TOLERANCE);
            assert!((center[1] - 3.0).abs() < LENGTH_TOLERANCE);
            assert!((radius - 1.0).abs() < LENGTH_TOLERANCE);
        }
        _ => panic!("expected copy-1 Circle"),
    }
    match &out[2] {
        SketchElement::Circle { id, center, .. } => {
            assert_eq!(id, "c1_pattern_circular_2");
            assert!((center[0] - (-3.0)).abs() < LENGTH_TOLERANCE);
            assert!((center[1] - 0.0).abs() < LENGTH_TOLERANCE);
        }
        _ => panic!("expected copy-2 Circle"),
    }
    match &out[3] {
        SketchElement::Circle { id, center, .. } => {
            assert_eq!(id, "c1_pattern_circular_3");
            assert!((center[0] - 0.0).abs() < LENGTH_TOLERANCE);
            assert!((center[1] - (-3.0)).abs() < LENGTH_TOLERANCE);
        }
        _ => panic!("expected copy-3 Circle"),
    }
}

#[test]
fn t04_circular_arc_numeric_model() {
    let profile = vec![SketchElement::Arc {
        id: "a1".to_string(),
        center: [2.0, 0.0],
        radius: 1.0,
        start_angle: 0.0,
        end_angle: FRAC_PI_2,
    }];
    // step = total_angle / count = π/2; copies = k=1 only.
    let out = apply_sketch_pattern_circular(&profile, &[], [0.0, 0.0], 2, PI).unwrap();
    assert_eq!(out.len(), 2);
    match &out[1] {
        SketchElement::Arc {
            id,
            center,
            radius,
            start_angle,
            end_angle,
        } => {
            assert_eq!(id, "a1_pattern_circular_1");
            assert!((center[0] - 0.0).abs() < LENGTH_TOLERANCE);
            assert!((center[1] - 2.0).abs() < LENGTH_TOLERANCE);
            assert!((radius - 1.0).abs() < LENGTH_TOLERANCE);
            assert!((start_angle - FRAC_PI_2).abs() < LENGTH_TOLERANCE);
            assert!((end_angle - PI).abs() < LENGTH_TOLERANCE);
        }
        _ => panic!("expected copy-1 Arc"),
    }
}

#[test]
fn t05_boundary_empty_selection_all() {
    let profile = vec![
        SketchElement::Line {
            id: "l1".to_string(),
            from: [0.0, 0.0],
            to: [2.0, 0.0],
        },
        SketchElement::Circle {
            id: "c1".to_string(),
            center: [3.0, 0.0],
            radius: 1.0,
        },
        SketchElement::Arc {
            id: "a1".to_string(),
            center: [0.0, 0.0],
            radius: 5.0,
            start_angle: 0.0,
            end_angle: FRAC_PI_2,
        },
    ];
    let lin = apply_sketch_pattern_linear(&profile, &[], 3, [1.0, 0.0], 3.0).unwrap();
    assert_eq!(lin.len(), profile.len() * 3);
    let cir = apply_sketch_pattern_circular(&profile, &[], [0.0, 0.0], 3, TAU).unwrap();
    assert_eq!(cir.len(), profile.len() * 3);
}

/// T_DEG_pattern_n0_linear: count=0 → fail-fast `sketch_pattern_linear_count_zero`.
#[test]
fn t_deg_pattern_n0_linear() {
    let profile = vec![SketchElement::Line {
        id: "l1".to_string(),
        from: [0.0, 0.0],
        to: [2.0, 0.0],
    }];
    let err = apply_sketch_pattern_linear(&profile, &[], 0, [1.0, 0.0], 1.0).unwrap_err();
    assert!(matches!(
        err,
        KernelError::InvalidParameter {
            kind: "sketch_pattern_linear_count_zero"
        }
    ));
}

/// T_DEG_pattern_n0_circular: count=0 → fail-fast `sketch_pattern_circular_count_zero`.
#[test]
fn t_deg_pattern_n0_circular() {
    let profile = vec![SketchElement::Circle {
        id: "c1".to_string(),
        center: [3.0, 0.0],
        radius: 1.0,
    }];
    let err = apply_sketch_pattern_circular(&profile, &[], [0.0, 0.0], 0, TAU).unwrap_err();
    assert!(matches!(
        err,
        KernelError::InvalidParameter {
            kind: "sketch_pattern_circular_count_zero"
        }
    ));
}

/// T_DEG_pattern_n1_linear: count=1 is a true no-op — selection contents are not consulted,
/// so an unknown id is tolerated and the profile comes back unchanged.
#[test]
fn t_deg_pattern_n1_linear() {
    let profile = vec![SketchElement::Line {
        id: "l1".to_string(),
        from: [0.0, 0.0],
        to: [2.0, 0.0],
    }];
    let out = apply_sketch_pattern_linear(
        &profile,
        &["definitely_not_in_profile".to_string()],
        1,
        [1.0, 0.0],
        5.0,
    )
    .unwrap();
    assert_eq!(out, profile);
}

/// T_DEG_pattern_n1_circular: same no-op contract on the circular path.
#[test]
fn t_deg_pattern_n1_circular() {
    let profile = vec![SketchElement::Circle {
        id: "c1".to_string(),
        center: [3.0, 0.0],
        radius: 1.0,
    }];
    let out = apply_sketch_pattern_circular(
        &profile,
        &["definitely_not_in_profile".to_string()],
        [0.0, 0.0],
        1,
        TAU,
    )
    .unwrap();
    assert_eq!(out, profile);
}

/// T_DEG_zero_direction: direction=(0,0) with count>=2 → `sketch_pattern_linear_direction_degenerate`.
#[test]
fn t_deg_zero_direction() {
    let profile = vec![SketchElement::Line {
        id: "l1".to_string(),
        from: [0.0, 0.0],
        to: [2.0, 0.0],
    }];
    let err = apply_sketch_pattern_linear(&profile, &[], 2, [0.0, 0.0], 1.0).unwrap_err();
    assert!(matches!(
        err,
        KernelError::InvalidParameter {
            kind: "sketch_pattern_linear_direction_degenerate"
        }
    ));
}

/// T_DEG_zero_distance: distance=0 with count>=2 → every copy would coincide with original.
#[test]
fn t_deg_zero_distance() {
    let profile = vec![SketchElement::Line {
        id: "l1".to_string(),
        from: [0.0, 0.0],
        to: [2.0, 0.0],
    }];
    let err = apply_sketch_pattern_linear(&profile, &[], 2, [1.0, 0.0], 0.0).unwrap_err();
    assert!(matches!(
        err,
        KernelError::InvalidParameter {
            kind: "sketch_pattern_linear_distance_zero"
        }
    ));
}

/// T_DEG_zero_angle: total_angle=0 with count>=2 → step=0, every copy coincides with original.
#[test]
fn t_deg_zero_angle() {
    let profile = vec![SketchElement::Circle {
        id: "c1".to_string(),
        center: [3.0, 0.0],
        radius: 1.0,
    }];
    let err = apply_sketch_pattern_circular(&profile, &[], [0.0, 0.0], 2, 0.0).unwrap_err();
    assert!(matches!(
        err,
        KernelError::InvalidParameter {
            kind: "sketch_pattern_circular_angle_degenerate"
        }
    ));
}

/// T_DEG_unknown_element_linear: count>=2 and selection id not in profile.
#[test]
fn t_deg_unknown_element_linear() {
    let profile = vec![SketchElement::Line {
        id: "l1".to_string(),
        from: [0.0, 0.0],
        to: [2.0, 0.0],
    }];
    let err = apply_sketch_pattern_linear(&profile, &["missing".to_string()], 2, [1.0, 0.0], 1.0)
        .unwrap_err();
    assert!(matches!(
        err,
        KernelError::InvalidParameter {
            kind: "sketch_pattern_linear_unknown_element_id"
        }
    ));
}

/// T_DEG_unknown_element_circular: count>=2 and selection id not in profile.
#[test]
fn t_deg_unknown_element_circular() {
    let profile = vec![SketchElement::Circle {
        id: "c1".to_string(),
        center: [3.0, 0.0],
        radius: 1.0,
    }];
    let err = apply_sketch_pattern_circular(&profile, &["missing".to_string()], [0.0, 0.0], 2, TAU)
        .unwrap_err();
    assert!(matches!(
        err,
        KernelError::InvalidParameter {
            kind: "sketch_pattern_circular_unknown_element_id"
        }
    ));
}

/// T_DEG_unsupported_type_linear: selection contains an Ellipse id.
#[test]
fn t_deg_unsupported_type_linear() {
    let profile = vec![SketchElement::Ellipse {
        id: "e1".to_string(),
        center: [0.0, 0.0],
        major: 2.0,
        minor: 1.0,
        rotation: 0.0,
    }];
    let err =
        apply_sketch_pattern_linear(&profile, &["e1".to_string()], 2, [1.0, 0.0], 1.0).unwrap_err();
    assert!(matches!(
        err,
        KernelError::UnsupportedFeature {
            kind: "sketch_pattern_linear_of_ellipse_or_conic"
        }
    ));
}

/// T_DEG_unsupported_type_circular: selection contains a Conic id.
#[test]
fn t_deg_unsupported_type_circular() {
    let profile = vec![SketchElement::Conic {
        id: "cn1".to_string(),
        coeffs: [1.0, 0.0, 1.0, 0.0, 0.0],
    }];
    let err = apply_sketch_pattern_circular(&profile, &["cn1".to_string()], [0.0, 0.0], 2, TAU)
        .unwrap_err();
    assert!(matches!(
        err,
        KernelError::UnsupportedFeature {
            kind: "sketch_pattern_circular_of_ellipse_or_conic"
        }
    ));
}

/// T_DEG_distance_invalid: distance が NaN/Inf → `sketch_pattern_linear_distance_invalid`.
/// (self-review C6: 宣言済み error kind の未テスト解消。Mirror #298 の NaN/Inf 軸テストと同水準)
#[test]
fn t_deg_distance_invalid() {
    let profile = vec![SketchElement::Line {
        id: "l1".to_string(),
        from: [0.0, 0.0],
        to: [2.0, 0.0],
    }];
    for bad in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        let err = apply_sketch_pattern_linear(&profile, &[], 2, [1.0, 0.0], bad).unwrap_err();
        assert!(
            matches!(
                err,
                KernelError::InvalidParameter {
                    kind: "sketch_pattern_linear_distance_invalid"
                }
            ),
            "distance={bad} must be rejected, got {err:?}"
        );
    }
}

/// T_DEG_angle_invalid: total_angle が NaN/Inf → `sketch_pattern_circular_angle_invalid`.
#[test]
fn t_deg_angle_invalid() {
    let profile = vec![SketchElement::Circle {
        id: "c1".to_string(),
        center: [3.0, 0.0],
        radius: 1.0,
    }];
    for bad in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        let err = apply_sketch_pattern_circular(&profile, &[], [0.0, 0.0], 2, bad).unwrap_err();
        assert!(
            matches!(
                err,
                KernelError::InvalidParameter {
                    kind: "sketch_pattern_circular_angle_invalid"
                }
            ),
            "total_angle={bad} must be rejected, got {err:?}"
        );
    }
}

/// T_DEG_center_degenerate: center が NaN/Inf → `sketch_pattern_circular_center_degenerate`.
/// center は total_angle より前に検査されるため、両方不正でも center 側の kind が返る。
#[test]
fn t_deg_center_degenerate() {
    let profile = vec![SketchElement::Circle {
        id: "c1".to_string(),
        center: [3.0, 0.0],
        radius: 1.0,
    }];
    for bad in [[f64::NAN, 0.0], [0.0, f64::INFINITY]] {
        let err = apply_sketch_pattern_circular(&profile, &[], bad, 2, TAU).unwrap_err();
        assert!(
            matches!(
                err,
                KernelError::InvalidParameter {
                    kind: "sketch_pattern_circular_center_degenerate"
                }
            ),
            "center={bad:?} must be rejected, got {err:?}"
        );
    }
}

/// T_DEG_duplicate_id_circular: derived id `"{elem_id}_pattern_circular_1"` が既に profile に
/// 存在する場合は `pattern_circular_duplicate_id` で拒否される (Linear 版の circular 対応)。
#[test]
fn t_deg_duplicate_id_circular() {
    let profile = vec![
        SketchElement::Circle {
            id: "c1".to_string(),
            center: [3.0, 0.0],
            radius: 1.0,
        },
        SketchElement::Circle {
            id: "c1_pattern_circular_1".to_string(),
            center: [0.0, 3.0],
            radius: 1.0,
        },
    ];
    let err = apply_sketch_pattern_circular(&profile, &["c1".to_string()], [0.0, 0.0], 4, TAU)
        .unwrap_err();
    assert!(matches!(
        err,
        KernelError::DegenerateSketchElement {
            element_id,
            reason: "pattern_circular_duplicate_id",
        } if element_id == "c1_pattern_circular_1"
    ));
}

/// T_DEG_count_too_large (self-review A1): `count` は u32 で上限が無いと `count: 30000000` のような
/// 桁ミス 1 つで数千万要素を生成して OOM する。`MAX_PATTERN_COUNT` を超える値は fail-fast する。
#[test]
fn t_deg_count_too_large() {
    let profile = vec![SketchElement::Line {
        id: "l1".to_string(),
        from: [0.0, 0.0],
        to: [2.0, 0.0],
    }];
    let err = apply_sketch_pattern_linear(&profile, &[], MAX_PATTERN_COUNT + 1, [1.0, 0.0], 1.0)
        .unwrap_err();
    assert!(matches!(
        err,
        KernelError::InvalidParameter {
            kind: "sketch_pattern_linear_count_too_large"
        }
    ));

    let circle = vec![SketchElement::Circle {
        id: "c1".to_string(),
        center: [3.0, 0.0],
        radius: 1.0,
    }];
    let err = apply_sketch_pattern_circular(&circle, &[], [0.0, 0.0], MAX_PATTERN_COUNT + 1, TAU)
        .unwrap_err();
    assert!(matches!(
        err,
        KernelError::InvalidParameter {
            kind: "sketch_pattern_circular_count_too_large"
        }
    ));
}

/// T_BOUNDARY_count_at_max: `count == MAX_PATTERN_COUNT` はちょうど通り、複製は count-1 個。
#[test]
fn t_boundary_count_at_max() {
    let profile = vec![SketchElement::Line {
        id: "l1".to_string(),
        from: [0.0, 0.0],
        to: [2.0, 0.0],
    }];
    let out =
        apply_sketch_pattern_linear(&profile, &[], MAX_PATTERN_COUNT, [1.0, 0.0], 1.0).unwrap();
    assert_eq!(out.len(), MAX_PATTERN_COUNT as usize);
}

/// T_DEG_duplicate_id: derived id `"{elem_id}_pattern_linear_1"` already exists in profile.
#[test]
fn t_deg_duplicate_id() {
    let profile = vec![
        SketchElement::Line {
            id: "l1".to_string(),
            from: [0.0, 0.0],
            to: [2.0, 0.0],
        },
        SketchElement::Line {
            id: "l1_pattern_linear_1".to_string(),
            from: [5.0, 0.0],
            to: [7.0, 0.0],
        },
    ];
    let err =
        apply_sketch_pattern_linear(&profile, &["l1".to_string()], 2, [1.0, 0.0], 1.0).unwrap_err();
    assert!(matches!(
        err,
        KernelError::DegenerateSketchElement {
            element_id,
            reason: "pattern_linear_duplicate_id",
        } if element_id == "l1_pattern_linear_1"
    ));
}

/// T_CRUD_pattern_n1_no_validation: count=1 (no-op) かつ selection に未知 id を含む
/// SketchPattern{Linear,Circular} の insert は、element-level gate が count>=2 限定のため
/// スキップされ Ok となる (build 側の no-op 仕様と一致)。Mirror の t10 系と対比される
/// Pattern 固有の仕様 (Codex R01 採択、plan 自律判断ログ §6)。
#[test]
fn t_crud_pattern_n1_no_validation() {
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
    let linear = Feature::SketchPatternLinear {
        id: "pl1".to_string(),
        sketch: "sk1".to_string(),
        selection: vec!["nope".to_string()],
        count: 1,
        direction: [1.0, 0.0],
        distance: 1.0,
        suppressed: false,
    };
    let r1 = FeatureCrud::insert(&doc, linear, 1);
    assert!(
        r1.is_ok(),
        "count=1 with unknown id must pass (no-op), got {r1:?}"
    );
    let circular = Feature::SketchPatternCircular {
        id: "pc1".to_string(),
        sketch: "sk1".to_string(),
        selection: vec!["nope".to_string()],
        center: [0.0, 0.0],
        count: 1,
        total_angle: TAU,
        suppressed: false,
    };
    let r2 = FeatureCrud::insert(&doc, circular, 1);
    assert!(
        r2.is_ok(),
        "count=1 with unknown id must pass (no-op), got {r2:?}"
    );
}

/// T_CRUD_pattern_edit_breaks_selection_linear (Codex STEP 7.5 A01 regression):
/// Pattern の `selection` が参照している要素を `CreateSketch` の edit で rename すると、
/// `refs_resolve_in_state` が element-level まで検証するため `EditBreaksConsumer` で拒否される。
/// 修正前は sketch id の存在だけを見ていたため edit が成功し、後続の
/// `build_bodies_from_features` で初めて `sketch_pattern_linear_unknown_element_id` になっていた
/// (build/CRUD の false-accept 非対称)。SketchFillet の T10
/// (`sketch_fillet_acceptance.rs::t10_crud_gate_rejects_rename_breaking_fillet`) と同じ契約。
#[test]
fn t_crud_pattern_edit_breaks_selection_linear() {
    let mut doc = Document::new("t");
    doc.root_component.features = vec![
        sketch_with_circle("sk1", "c1"),
        Feature::SketchPatternLinear {
            id: "pl1".to_string(),
            sketch: "sk1".to_string(),
            selection: vec!["c1".to_string()],
            count: 3,
            direction: [1.0, 0.0],
            distance: 3.0,
            suppressed: false,
        },
    ];
    // c1 → c_renamed: Pattern の selection が解決不能になる。
    let err = FeatureCrud::edit(&doc, "sk1", sketch_with_circle("sk1", "c_renamed")).unwrap_err();
    assert!(
        matches!(
            &err,
            FeatureCrudError::EditBreaksConsumer { broken_consumer_id, .. }
                if broken_consumer_id == "pl1"
        ),
        "expected EditBreaksConsumer(pl1), got {err:?}"
    );
}

/// T_CRUD_pattern_edit_breaks_selection_circular: Linear と同じ契約を Circular 側でも固定する。
#[test]
fn t_crud_pattern_edit_breaks_selection_circular() {
    let mut doc = Document::new("t");
    doc.root_component.features = vec![
        sketch_with_circle("sk1", "c1"),
        Feature::SketchPatternCircular {
            id: "pc1".to_string(),
            sketch: "sk1".to_string(),
            selection: vec!["c1".to_string()],
            center: [0.0, 0.0],
            count: 4,
            total_angle: TAU,
            suppressed: false,
        },
    ];
    let err = FeatureCrud::edit(&doc, "sk1", sketch_with_circle("sk1", "c_renamed")).unwrap_err();
    assert!(
        matches!(
            &err,
            FeatureCrudError::EditBreaksConsumer { broken_consumer_id, .. }
                if broken_consumer_id == "pc1"
        ),
        "expected EditBreaksConsumer(pc1), got {err:?}"
    );
}

/// T_CRUD_pattern_edit_n1_selection_ignored: count=1 は kernel 側で selection を一切参照しない
/// no-op なので、同じ rename edit でも Pattern は壊れず edit は成功する
/// (Codex R01 で決めた `count >= 2` 限定 gate が edit 経路でも一貫していることの固定)。
#[test]
fn t_crud_pattern_edit_n1_selection_ignored() {
    let mut doc = Document::new("t");
    doc.root_component.features = vec![
        sketch_with_circle("sk1", "c1"),
        Feature::SketchPatternLinear {
            id: "pl1".to_string(),
            sketch: "sk1".to_string(),
            selection: vec!["c1".to_string()],
            count: 1,
            direction: [1.0, 0.0],
            distance: 3.0,
            suppressed: false,
        },
    ];
    let r = FeatureCrud::edit(&doc, "sk1", sketch_with_circle("sk1", "c_renamed"));
    assert!(
        r.is_ok(),
        "count=1 pattern must not block the edit, got {r:?}"
    );
}

/// T_CRUD_pattern_unknown_element: count>=2 かつ selection に未知 id を含む場合は
/// element-level gate が発動し、`sketch_pattern_linear_elem_not_found` で拒否される。
#[test]
fn t_crud_pattern_unknown_element() {
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
    let bad = Feature::SketchPatternLinear {
        id: "pl1".to_string(),
        sketch: "sk1".to_string(),
        selection: vec!["nope".to_string()],
        count: 3,
        direction: [1.0, 0.0],
        distance: 1.0,
        suppressed: false,
    };
    let err = FeatureCrud::insert(&doc, bad, 1).unwrap_err();
    match err {
        FeatureCrudError::SketchElementNotResolved {
            reason,
            elem1_id,
            sketch_ref,
            feature_id,
            ..
        } if reason == "sketch_pattern_linear_elem_not_found"
            && elem1_id == "nope"
            && sketch_ref == "sk1"
            && feature_id == "pl1" => {}
        other => panic!("expected SketchElementNotResolved, got {other:?}"),
    }
}

/// T_CRUD_pattern_unknown_element_circular (self-review C3):
/// Circular 側の element-level gate (`sketch_pattern_circular_elem_not_found`) は
/// Linear 版の 34 行コピペブロックであり、reason 文字列を取り違えても検出できない状態だった。
/// gate が発火する側 (count>=2 + 未知 id) を Circular でも固定する。
#[test]
fn t_crud_pattern_unknown_element_circular() {
    let mut doc = Document::new("t");
    doc.root_component.features = vec![sketch_with_circle("sk1", "c1")];
    let bad = Feature::SketchPatternCircular {
        id: "pc1".to_string(),
        sketch: "sk1".to_string(),
        selection: vec!["nope".to_string()],
        center: [0.0, 0.0],
        count: 3,
        total_angle: TAU,
        suppressed: false,
    };
    let err = FeatureCrud::insert(&doc, bad, 1).unwrap_err();
    match err {
        FeatureCrudError::SketchElementNotResolved {
            reason,
            elem1_id,
            sketch_ref,
            feature_id,
            ..
        } if reason == "sketch_pattern_circular_elem_not_found"
            && elem1_id == "nope"
            && sketch_ref == "sk1"
            && feature_id == "pc1" => {}
        other => panic!("expected SketchElementNotResolved, got {other:?}"),
    }
}
