//! Acceptance tests for #297 (Sketch Chamfer)
//! テスト計画: features/297-phase10-sketch-chamfer-engawa/plan.md 「## テスト計画（ID 付き）」参照

use engawa_build::build_bodies_from_features;
use engawa_build::{FeatureCrud, FeatureCrudError};
use engawa_format::{Document, Feature, RefPlane, SketchElement, SketchPlane};
use engawa_kernel::brep::topology::IdGenerator;
use engawa_kernel::error::KernelError;
use engawa_kernel::geometry::math::LENGTH_TOLERANCE;
use engawa_kernel::geometry::sketch_chamfer::apply_sketch_chamfer_build;

// === T01: kernel pure-function determinism + derived Line id ===
#[test]
fn t01_determinism() {
    let profile = rect_profile();
    let out1 = apply_sketch_chamfer_build(&profile, "l1", "l2", 1.0).unwrap();
    let out2 = apply_sketch_chamfer_build(&profile, "l1", "l2", 1.0).unwrap();
    assert_eq!(out1, out2);

    let line = out1
        .iter()
        .find(|e| matches!(e, SketchElement::Line { id, .. } if id.ends_with("_chamfer_line")))
        .expect("chamfer should insert a derived Line");
    let id = match line {
        SketchElement::Line { id, .. } => id.as_str(),
        _ => unreachable!(),
    };
    assert_eq!(id, "l1_l2_chamfer_line");
}

// === T01d: IdGenerator-inclusive build determinism ===
#[test]
fn t01d_build_determinism_with_id_generator() {
    let features = chamfer_rect_features("sk", "c1", "ext", 1.0);
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

// === T01e: elem1_id/elem2_id input order invariance (GLM ambig r1 AM01 採用) ===
#[test]
fn t01e_input_order_invariance() {
    let profile = rect_profile();
    let forward = apply_sketch_chamfer_build(&profile, "l1", "l2", 1.0).unwrap();
    let reverse = apply_sketch_chamfer_build(&profile, "l2", "l1", 1.0).unwrap();
    assert_eq!(forward, reverse);

    let fwd_wrap = apply_sketch_chamfer_build(&profile, "l4", "l1", 1.0).unwrap();
    let rev_wrap = apply_sketch_chamfer_build(&profile, "l1", "l4", 1.0).unwrap();
    assert_eq!(fwd_wrap, rev_wrap);
}

// === T02: 90°コーナーの解析解一致 ===
#[test]
fn t02_analytic_90deg_corner() {
    let profile = rect_profile();
    let out = apply_sketch_chamfer_build(&profile, "l1", "l2", 1.0).unwrap();
    // l1 was (0,0)→(10,0); after chamfer it should be (0,0)→(9,0)
    // Derived chamfer Line: (9,0)→(10,1)
    // l2 was (10,0)→(10,5); after chamfer it should be (10,1)→(10,5)
    let l1 = find_line(&out, "l1").expect("l1 should remain");
    match l1 {
        SketchElement::Line { from, to, .. } => {
            assert!((from[0] - 0.0).abs() < LENGTH_TOLERANCE);
            assert!((from[1] - 0.0).abs() < LENGTH_TOLERANCE);
            assert!((to[0] - 9.0).abs() < LENGTH_TOLERANCE);
            assert!((to[1] - 0.0).abs() < LENGTH_TOLERANCE);
        }
        _ => unreachable!(),
    }
    let chamfer_line = find_line(&out, "l1_l2_chamfer_line").expect("derived chamfer Line");
    match chamfer_line {
        SketchElement::Line { from, to, .. } => {
            assert!((from[0] - 9.0).abs() < LENGTH_TOLERANCE);
            assert!((from[1] - 0.0).abs() < LENGTH_TOLERANCE);
            assert!((to[0] - 10.0).abs() < LENGTH_TOLERANCE);
            assert!((to[1] - 1.0).abs() < LENGTH_TOLERANCE);
        }
        _ => unreachable!(),
    }
    let l2 = find_line(&out, "l2").expect("l2 should remain");
    match l2 {
        SketchElement::Line { from, to, .. } => {
            assert!((from[0] - 10.0).abs() < LENGTH_TOLERANCE);
            assert!((from[1] - 1.0).abs() < LENGTH_TOLERANCE);
            assert!((to[0] - 10.0).abs() < LENGTH_TOLERANCE);
            assert!((to[1] - 5.0).abs() < LENGTH_TOLERANCE);
        }
        _ => unreachable!(),
    }
}

// === T03: 60°コーナーの解析解一致 ===
// Setup: l1 from (5,0)→(0,0), l2 from (0,0)→(0.5*5, √3/2*5)=(2.5, 5√3/2).
// corner=(0,0). d_a = normalize((5,0)-(0,0))=(1,0). d_b = (0.5, √3/2). theta = acos(0.5) = π/3.
// length=1.0. cut_a = corner + d_a*1 = (1, 0). cut_b = corner + d_b*1 = (0.5, √3/2).
#[test]
fn t03_analytic_60deg_corner() {
    let profile = vec![
        line("l1", [5.0, 0.0], [0.0, 0.0]),
        line("l2", [0.0, 0.0], [2.5, 5.0 * 3.0_f64.sqrt() / 2.0]),
    ];
    let out = apply_sketch_chamfer_build(&profile, "l1", "l2", 1.0).unwrap();
    let l1 = find_line(&out, "l1").expect("l1 should remain");
    match l1 {
        SketchElement::Line { to, .. } => {
            assert!((to[0] - 1.0).abs() < LENGTH_TOLERANCE);
            assert!((to[1] - 0.0).abs() < LENGTH_TOLERANCE);
        }
        _ => unreachable!(),
    }
    let chamfer_line = find_line(&out, "l1_l2_chamfer_line").expect("derived chamfer Line");
    match chamfer_line {
        SketchElement::Line { from, to, .. } => {
            assert!((from[0] - 1.0).abs() < LENGTH_TOLERANCE);
            assert!((from[1] - 0.0).abs() < LENGTH_TOLERANCE);
            assert!((to[0] - 0.5).abs() < LENGTH_TOLERANCE);
            assert!((to[1] - 3.0_f64.sqrt() / 2.0).abs() < LENGTH_TOLERANCE);
        }
        _ => unreachable!(),
    }
}

// === T05: L字 line ペアへの正常系 chamfer (#297 完了条件必須) ===
#[test]
fn t05_normal_l_shape_chamfer() {
    let profile = vec![
        line("l1", [0.0, 0.0], [10.0, 0.0]),
        line("l2", [10.0, 0.0], [10.0, 5.0]),
    ];
    let out = apply_sketch_chamfer_build(&profile, "l1", "l2", 1.0).unwrap();
    assert_eq!(out.len(), 3);
    // Trimmed l1 + chamfer Line + trimmed l2
    assert!(matches!(&out[0], SketchElement::Line { id, .. } if id == "l1"));
    assert!(matches!(&out[1], SketchElement::Line { id, .. } if id == "l1_l2_chamfer_line"));
    assert!(matches!(&out[2], SketchElement::Line { id, .. } if id == "l2"));
}

// === T06: chamfer後のprofileが閉ループ連続性を保つ ===
#[test]
fn t06_profile_chain_continuity() {
    let profile = rect_profile();
    for (a, b) in [("l1", "l2"), ("l4", "l1")] {
        let out = apply_sketch_chamfer_build(&profile, a, b, 1.0).unwrap();
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

// === T07: CRUD gate — rename/reorder が consumer を破壊する edit ===
#[test]
fn t07_crud_gate_rename_breaks_consumer() {
    let mut doc = Document::new("t");
    doc.root_component.features = chamfer_rect_features("sk", "c1", "ext", 1.0);

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
        } if broken_consumer_id == "c1"
    ));
}

// === T08: CRUD gate — 存在しない要素IDを参照する Chamfer の insert ===
#[test]
fn t08_crud_gate_insert_nonexistent_element() {
    let mut doc = Document::new("t");
    doc.root_component.features = vec![rect_create_sketch("sk")];

    let bad_chamfer = Feature::SketchChamfer {
        id: "c1".to_string(),
        sketch: "sk".to_string(),
        elem1_id: "l1".to_string(),
        elem2_id: "nope".to_string(),
        length: 1.0,
        suppressed: false,
    };
    let err = FeatureCrud::insert(&doc, bad_chamfer, 1).unwrap_err();
    match err {
        FeatureCrudError::SketchElementNotResolved {
            reason: "sketch_fillet_elem_not_found",
            elem1_id,
            elem2_id,
            sketch_ref,
            feature_id,
        } if elem1_id == "l1" && elem2_id == "nope" && sketch_ref == "sk" && feature_id == "c1" => {
        }
        other => panic!("expected SketchElementNotResolved(elem_not_found), got {other:?}"),
    }
}

// === T09: CRUD gate 境界 — 座標のみ変更は gate を通すが validate_profile_closed で reject ===
#[test]
fn t09_crud_gate_coordinate_only_edit_boundary() {
    let mut doc = Document::new("t");
    doc.root_component.features = chamfer_rect_features("sk", "c1", "ext", 1.0);

    let moved_profile = vec![
        line("l1", [0.0, 0.0], [11.0, 0.0]),
        line("l2", [10.0, 0.0], [10.0, 5.0]),
        line("l3", [10.0, 5.0], [0.0, 5.0]),
        line("l4", [0.0, 5.0], [0.0, 0.0]),
    ];
    let new_sketch = Feature::CreateSketch {
        id: "sk".to_string(),
        plane: SketchPlane::Xy,
        offset: 0.0,
        variables: vec![],
        profile: moved_profile,
        plane_ref: None,
        suppressed: false,
    };

    let edited = FeatureCrud::edit(&doc, "sk", new_sketch).expect("gate should accept edit");

    let ref_planes = RefPlane::default_canonical_three();
    let mut gen = IdGenerator::new(0);
    let err = build_bodies_from_features(&edited.root_component.features, &ref_planes, &mut gen)
        .unwrap_err();
    assert!(
        matches!(err, KernelError::InvalidParameter { kind: "profile" }),
        "expected profile-closure rejection, got {err:?}"
    );
}

// === T10: CRUD gate 既知制約 (Codex R01 部分採用・false-reject) ===
#[test]
fn t10_crud_gate_known_limitation_false_reject() {
    let features = chamfer_rect_features("sk", "c1", "ext", 1.0);
    // (a) build succeeds: current profile has l1 and derived l1_l2_chamfer_line both as Lines
    // (chamfer cuts l1 short but does not remove it; the derived Line is adjacent to l1 at the
    // tail when l2 wraps around — but here we re-chamfer l1/l2 which is the *same* pair, so
    // instead target l1 / l1_l2_chamfer_line which are adjacent in current profile).
    // Build path uses current profile; derived Line id resolves.
    let mut features_build = features.clone();
    features_build.push(Feature::SketchChamfer {
        id: "c2".to_string(),
        sketch: "sk".to_string(),
        elem1_id: "l1".to_string(),
        elem2_id: "l1_l2_chamfer_line".to_string(),
        length: 0.5,
        suppressed: false,
    });
    let ref_planes = RefPlane::default_canonical_three();
    let mut gen = IdGenerator::new(0);
    let build_result = build_bodies_from_features(&features_build, &ref_planes, &mut gen);
    assert!(
        build_result.is_ok(),
        "build should accept derived Line ref, got {build_result:?}"
    );

    // (b) CRUD gate rejects: derived Line id does not exist in original CreateSketch profile.
    let mut doc = Document::new("t");
    doc.root_component.features = features;
    let bad_chamfer = Feature::SketchChamfer {
        id: "c2".to_string(),
        sketch: "sk".to_string(),
        elem1_id: "l1".to_string(),
        elem2_id: "l1_l2_chamfer_line".to_string(),
        length: 0.5,
        suppressed: false,
    };
    let err = FeatureCrud::insert(&doc, bad_chamfer, 2).unwrap_err();
    match err {
        FeatureCrudError::SketchElementNotResolved {
            reason: "sketch_fillet_elem_not_found",
            ..
        } => {}
        other => panic!("expected SketchElementNotResolved(elem_not_found), got {other:?}"),
    }
}

// === T11: CRUD gate 既知制約 (false-accept, #296 T13 の Chamfer 版) ===
#[test]
fn t11_crud_gate_known_limitation_false_accept() {
    // history: CreateSketch(sk, [l1..l4]) + SketchChamfer(c1, sk, l1, l2) [non-adjacency
    // for original l1/l2 is impossible — they ARE adjacent. Use c2 on l1/l2 again, which
    // the gate accepts but build rejects since l1 is now trimmed and not adjacent to l2 anymore].
    let mut features = vec![rect_create_sketch("sk")];
    features.push(Feature::SketchChamfer {
        id: "c1".to_string(),
        sketch: "sk".to_string(),
        elem1_id: "l1".to_string(),
        elem2_id: "l2".to_string(),
        length: 1.0,
        suppressed: false,
    });

    let mut doc = Document::new("t");
    doc.root_component.features = features;

    let second_chamfer = Feature::SketchChamfer {
        id: "c2".to_string(),
        sketch: "sk".to_string(),
        elem1_id: "l1".to_string(),
        elem2_id: "l2".to_string(),
        length: 1.0,
        suppressed: false,
    };
    // (a) gate accepts: original profile still has l1/l2 adjacent
    let inserted = FeatureCrud::insert(&doc, second_chamfer, 2).expect("gate should accept");

    // (b) build fails: after first chamfer, l1/l2 are no longer adjacent (separated by the
    // derived chamfer Line). Build uses current profile, not original.
    let ref_planes = RefPlane::default_canonical_three();
    let mut gen = IdGenerator::new(0);
    let err = build_bodies_from_features(&inserted.root_component.features, &ref_planes, &mut gen)
        .unwrap_err();
    assert!(
        matches!(
            err,
            KernelError::InvalidParameter {
                kind: "sketch_fillet_elems_not_adjacent"
            }
        ),
        "expected not-adjacent rejection, got {err:?}"
    );
}

// === T12: 連鎖 正常系 (Codex R01: fillet → chamfer、元profile要素のみを対象) ===
#[test]
fn t12_chain_fillet_then_chamfer() {
    let mut features = vec![rect_create_sketch("sk")];
    features.push(Feature::SketchFillet {
        id: "f1".to_string(),
        sketch: "sk".to_string(),
        elem1_id: "l1".to_string(),
        elem2_id: "l2".to_string(),
        radius: 1.0,
        suppressed: false,
    });
    features.push(Feature::SketchChamfer {
        id: "c1".to_string(),
        sketch: "sk".to_string(),
        elem1_id: "l3".to_string(),
        elem2_id: "l4".to_string(),
        length: 1.0,
        suppressed: false,
    });
    features.push(Feature::Extrude {
        id: "ext".to_string(),
        sketch: "sk".to_string(),
        depth: 3.0,
        fuse_target: None,
        suppressed: false,
    });

    let ref_planes = RefPlane::default_canonical_three();
    let mut gen = IdGenerator::new(0);
    let built = build_bodies_from_features(&features, &ref_planes, &mut gen).unwrap();
    assert_eq!(built.all().len(), 1);
    assert_eq!(built.all()[0].solid.euler_poincare(), 0);

    // Codex final gate A01 (#297): insert する対象は index 2 の SketchChamfer (l3/l4)
    // でなければならない。旧版は features.len()-1 (= Extrude, index 3) を insert して
    // いたため、意図した「fillet -> chamfer 連鎖の CRUD gate 正常系」を固定できていなかった。
    let mut doc = Document::new("t");
    doc.root_component.features = features[..2].to_vec(); // [CreateSketch, SketchFillet]
    let inserted = FeatureCrud::insert(
        &doc,
        features[2].clone(), // SketchChamfer { elem1_id: "l3", elem2_id: "l4" }
        doc.root_component.features.len(),
    );
    assert!(
        inserted.is_ok(),
        "SketchChamfer targeting original profile element should pass CRUD gate, got {inserted:?}"
    );
}

// === T_DEG_chamfer_too_long: 長さ過大 (#297 完了条件必須) ===
#[test]
fn t_deg_chamfer_too_long() {
    let profile = vec![
        line("l1", [0.0, 0.0], [1.0, 0.0]),
        line("l2", [1.0, 0.0], [1.0, 1.0]),
    ];
    let err = apply_sketch_chamfer_build(&profile, "l1", "l2", 10.0).unwrap_err();
    assert!(matches!(
        err,
        KernelError::ChamferLengthTooLarge {
            elem1_id,
            elem2_id,
            length: 10.0,
        } if elem1_id == "l1" && elem2_id == "l2"
    ));
}

// === T_DEG_corner_angle_flat: ~180° (実質コーナー無し) ===
// l1 (0,0)→(5,0), l2 (5,0)→(10,0): 完全に一直線 (θ=π) で、かつ両 Line とも十分な長さを
// 持つため zero_length_input_line 分岐には落ちない。これにより corner_angle_degenerate
// 分岐そのものを exercise する (Claude self-review #297 STEP 6.7 で発見・修正: 旧版は
// l2 を 1e-12 長にしていたため zero_length 判定が先に発火し、assert の OR がそれを隠していた)。
#[test]
fn t_deg_corner_angle_flat() {
    let profile = vec![
        line("l1", [0.0, 0.0], [5.0, 0.0]),
        line("l2", [5.0, 0.0], [10.0, 0.0]),
    ];
    let err = apply_sketch_chamfer_build(&profile, "l1", "l2", 0.5).unwrap_err();
    assert!(matches!(
        err,
        KernelError::DegenerateSketchElement {
            reason: "chamfer_corner_angle_degenerate",
            ..
        }
    ));
}

// === T_DEG_corner_angle_zero: ~0° (折り返し) ===
#[test]
fn t_deg_corner_angle_zero() {
    let profile = vec![
        line("l1", [-5.0, 0.0], [0.0, 0.0]),
        line("l2", [0.0, 0.0], [-5.0, 0.0]),
    ];
    let err = apply_sketch_chamfer_build(&profile, "l1", "l2", 0.5).unwrap_err();
    assert!(matches!(
        err,
        KernelError::DegenerateSketchElement {
            reason: "chamfer_corner_angle_degenerate",
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
    let err = apply_sketch_chamfer_build(&profile, "l1", "l2", 0.5).unwrap_err();
    assert!(matches!(
        err,
        KernelError::InvalidParameter {
            kind: "sketch_fillet_no_shared_corner"
        }
    ));
}

// === T_DEG_non_line_element_rejected: 対象要素がCircle等 ===
#[test]
fn t_deg_non_line_element_rejected() {
    let mut profile = rect_profile();
    profile[1] = SketchElement::Circle {
        id: "c1".to_string(),
        center: [0.0, 0.0],
        radius: 1.0,
    };
    let err = apply_sketch_chamfer_build(&profile, "l1", "c1", 0.5).unwrap_err();
    assert!(matches!(
        err,
        KernelError::UnsupportedFeature {
            kind: "sketch_fillet_only_line_line"
        }
    ));
}

// === T_DEG_same_element_rejected: elem1_id == elem2_id ===
#[test]
fn t_deg_same_element_rejected() {
    let profile = rect_profile();
    let err = apply_sketch_chamfer_build(&profile, "l1", "l1", 0.5).unwrap_err();
    assert!(matches!(
        err,
        KernelError::InvalidParameter {
            kind: "sketch_fillet_same_element"
        }
    ));
}

// === T_DEG_not_adjacent_rejected: 配列上で1個飛び ===
#[test]
fn t_deg_not_adjacent_rejected() {
    let profile = rect_profile();
    let err = apply_sketch_chamfer_build(&profile, "l1", "l3", 0.5).unwrap_err();
    assert!(matches!(
        err,
        KernelError::InvalidParameter {
            kind: "sketch_fillet_elems_not_adjacent"
        }
    ));
}

// === T_DEG_elem_not_found: 存在しないID ===
#[test]
fn t_deg_elem_not_found() {
    let profile = rect_profile();
    let err = apply_sketch_chamfer_build(&profile, "l1", "nope", 0.5).unwrap_err();
    assert!(matches!(
        err,
        KernelError::InvalidParameter {
            kind: "sketch_fillet_elem_not_found"
        }
    ));
}

// === T_DEG_negative_length: length = -1.0 ===
#[test]
fn t_deg_negative_length() {
    let profile = rect_profile();
    let err = apply_sketch_chamfer_build(&profile, "l1", "l2", -1.0).unwrap_err();
    assert!(matches!(
        err,
        KernelError::InvalidParameter { kind: "length" }
    ));
}

// === T_DEG_nan_length: length = NaN ===
#[test]
fn t_deg_nan_length() {
    let profile = rect_profile();
    let err = apply_sketch_chamfer_build(&profile, "l1", "l2", f64::NAN).unwrap_err();
    assert!(matches!(
        err,
        KernelError::InvalidParameter { kind: "length" }
    ));
}

// === T_DEG_inf_length: length = Inf ===
#[test]
fn t_deg_inf_length() {
    let profile = rect_profile();
    let err = apply_sketch_chamfer_build(&profile, "l1", "l2", f64::INFINITY).unwrap_err();
    assert!(matches!(
        err,
        KernelError::InvalidParameter { kind: "length" }
    ));
}

// === T_DEG_zero_length_input_line: elem_a が零長 ===
#[test]
fn t_deg_zero_length_input_line() {
    let profile = vec![
        line("l1", [5.0, 0.0], [5.0, 0.0]),
        line("l2", [5.0, 0.0], [5.0, 5.0]),
    ];
    let err = apply_sketch_chamfer_build(&profile, "l1", "l2", 0.5).unwrap_err();
    assert!(matches!(
        err,
        KernelError::DegenerateSketchElement {
            element_id,
            reason: "chamfer_zero_length_input_line",
        } if element_id == "l1"
    ));
}

// === T_DEG_zero_length_input_line_b: elem_b が零長 ===
#[test]
fn t_deg_zero_length_input_line_b() {
    let profile = vec![
        line("l1", [5.0, 0.0], [5.0, 5.0]),
        line("l2", [5.0, 5.0], [5.0, 5.0]),
    ];
    let err = apply_sketch_chamfer_build(&profile, "l1", "l2", 0.5).unwrap_err();
    assert!(matches!(
        err,
        KernelError::DegenerateSketchElement {
            element_id,
            reason: "chamfer_zero_length_input_line",
        } if element_id == "l2"
    ));
}

// === T_DEG_line_id_collision: 派生Line IDが既存要素IDと衝突 ===
#[test]
fn t_deg_line_id_collision() {
    let mut profile = rect_profile();
    profile.push(line("l1_l2_chamfer_line", [0.0, 0.0], [1.0, 1.0]));
    let err = apply_sketch_chamfer_build(&profile, "l1", "l2", 0.5).unwrap_err();
    assert!(matches!(
        err,
        KernelError::InvalidParameter {
            kind: "sketch_chamfer_line_id_collision"
        }
    ));
}

// === T_DEG_sketch_ref_not_found: 存在しないsketchを参照 ===
#[test]
fn t_deg_sketch_ref_not_found() {
    let features = vec![Feature::SketchChamfer {
        id: "c1".to_string(),
        sketch: "missing_sketch".to_string(),
        elem1_id: "l1".to_string(),
        elem2_id: "l2".to_string(),
        length: 1.0,
        suppressed: false,
    }];
    let ref_planes = RefPlane::default_canonical_three();
    let mut gen = IdGenerator::new(0);
    let err = build_bodies_from_features(&features, &ref_planes, &mut gen).unwrap_err();
    assert!(matches!(
        err,
        KernelError::SketchNotFound { sketch } if sketch == "missing_sketch"
    ));
}

// === T_DEG_length_f64_max: 数値境界 — f64::MAX は有限だが実線長を逸脱 (ChamferLengthTooLarge) ===
#[test]
fn t_deg_length_f64_max_rejected() {
    let profile = rect_profile();
    let err = apply_sketch_chamfer_build(&profile, "l1", "l2", f64::MAX).unwrap_err();
    assert!(matches!(err, KernelError::ChamferLengthTooLarge { .. }));
}

// === T_DEG_length_min_positive: f64::MIN_POSITIVE (≈4.9e-324) は LENGTH_TOLERANCE 未満 ===
#[test]
fn t_deg_length_min_positive_rejected() {
    let profile = rect_profile();
    let err = apply_sketch_chamfer_build(&profile, "l1", "l2", f64::MIN_POSITIVE).unwrap_err();
    assert!(matches!(
        err,
        KernelError::InvalidParameter { kind: "length" }
    ));
}

// === T_DEG_length_negative_zero: -0.0 == 0.0 なので length<=EPS で reject ===
#[test]
fn t_deg_length_negative_zero_rejected() {
    let profile = rect_profile();
    let err = apply_sketch_chamfer_build(&profile, "l1", "l2", -0.0).unwrap_err();
    assert!(matches!(
        err,
        KernelError::InvalidParameter { kind: "length" }
    ));
}

// === T_DEG_empty_profile: source が空 (エッジ0) → find_adjacent_pair が elem_not_found ===
#[test]
fn t_deg_empty_profile_rejected() {
    let err = apply_sketch_chamfer_build(&[], "l1", "l2", 1.0).unwrap_err();
    assert!(matches!(
        err,
        KernelError::InvalidParameter {
            kind: "sketch_fillet_elem_not_found"
        }
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

/// `[CreateSketch(sk, rectangle), SketchChamfer(c1, sk, l1, l2, length), Extrude(ext, sk)]`
fn chamfer_rect_features(sk: &str, c1: &str, ext: &str, length: f64) -> Vec<Feature> {
    vec![
        rect_create_sketch(sk),
        Feature::SketchChamfer {
            id: c1.to_string(),
            sketch: sk.to_string(),
            elem1_id: "l1".to_string(),
            elem2_id: "l2".to_string(),
            length,
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

fn find_line<'a>(out: &'a [SketchElement], id: &str) -> Option<&'a SketchElement> {
    out.iter()
        .find(|e| matches!(e, SketchElement::Line { id: eid, .. } if eid == id))
}

fn endpoint_of(elem: &SketchElement) -> [f64; 2] {
    match elem {
        SketchElement::Line { to, .. } => *to,
        _ => panic!("non-Line element"),
    }
}

fn startpoint_of(elem: &SketchElement) -> [f64; 2] {
    match elem {
        SketchElement::Line { from, .. } => *from,
        _ => panic!("non-Line element"),
    }
}
