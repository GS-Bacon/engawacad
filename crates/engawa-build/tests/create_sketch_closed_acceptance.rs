//! #288: Phase 10 — Multi-closed-primitive profile rejection.
//!
//! Codex 7.5 round 2 (#275 review C-F02) で発見された構造的バグ。
//! `crates/engawa-build/src/lib.rs:271` 付近で複数 SketchElement を flat_map で
//! 1 本の polyline に潰しているため、`[Circle, Circle]` のような複数 closed primitive
//! profile は壊れた 1 本ポリラインとして make_extrusion に渡される。
//!
//! Phase 10 範囲では各 create_sketch に SketchElement 1 つしか含めないため
//! triggered されないが、将来の sketch 編集 / 複数閉曲線 profile で必ず破綻する。
//! 本テストは Phase 11+ multi-contour 正規実装までの defensive guard を検証する。

use engawa_build::{build_bodies_from_features, BuiltBodies};
use engawa_format::{Feature, SketchElement, SketchPlane};
use engawa_kernel::brep::topology::IdGenerator;
use engawa_kernel::error::KernelError;

fn build_with_profile(profile: Vec<SketchElement>) -> Result<BuiltBodies, KernelError> {
    let features = vec![
        Feature::CreateSketch {
            id: "sketch_1".to_string(),
            plane: SketchPlane::Xy,
            offset: 0.0,
            variables: vec![],
            profile,
            plane_ref: None,
            suppressed: false,
        },
        Feature::Extrude {
            id: "extrude_1".to_string(),
            sketch: "sketch_1".to_string(),
            depth: 5.0,
            fuse_target: None,
            suppressed: false,
        },
    ];
    let mut gen = IdGenerator::new(0);
    build_bodies_from_features(&features, &[], &mut gen)
}

fn build_with_profile_extrude_cut(profile: Vec<SketchElement>) -> Result<BuiltBodies, KernelError> {
    let features = vec![
        Feature::CreateBox {
            id: "target".to_string(),
            width: 20.0,
            height: 20.0,
            depth: 20.0,
            suppressed: false,
        },
        Feature::CreateSketch {
            id: "sketch_1".to_string(),
            plane: SketchPlane::Xy,
            offset: 0.0,
            variables: vec![],
            profile,
            plane_ref: None,
            suppressed: false,
        },
        Feature::ExtrudeCut {
            id: "cut_1".to_string(),
            sketch: "sketch_1".to_string(),
            depth: 5.0,
            target: "target".to_string(),
            suppressed: false,
        },
    ];
    let mut gen = IdGenerator::new(0);
    build_bodies_from_features(&features, &[], &mut gen)
}

/// T01: 正常系 — Circle 1 要素のみの profile は OK (単一 closed primitive)
///
/// guard 通過に加えて plan の「1 Solid 生成」契約も assert する (Codex round 2 M-F01)。
#[test]
fn t01_single_circle_ok() {
    let result = build_with_profile(vec![SketchElement::Circle {
        id: "c1".to_string(),
        center: [0.0, 0.0],
        radius: 3.0,
    }]);
    let bodies = result.expect("single Circle profile should build");
    assert_eq!(
        bodies.all().len(),
        1,
        "single Circle Extrude should produce exactly 1 body, got {}",
        bodies.all().len()
    );
}

/// T02: 正常系 — Line 4 本で Rectangle 形状 (複数 open elements は OK)
#[test]
fn t02_lines_rectangle_ok() {
    let lines = vec![
        SketchElement::Line {
            id: "l1".to_string(),
            from: [0.0, 0.0],
            to: [4.0, 0.0],
        },
        SketchElement::Line {
            id: "l2".to_string(),
            from: [4.0, 0.0],
            to: [4.0, 3.0],
        },
        SketchElement::Line {
            id: "l3".to_string(),
            from: [4.0, 3.0],
            to: [0.0, 3.0],
        },
        SketchElement::Line {
            id: "l4".to_string(),
            from: [0.0, 3.0],
            to: [0.0, 0.0],
        },
    ];
    let result = build_with_profile(lines);
    let bodies = result.expect("rectangle via 4 Lines should build");
    assert_eq!(
        bodies.all().len(),
        1,
        "4-Line rectangle Extrude should produce exactly 1 body, got {}",
        bodies.all().len()
    );
}

/// T03_degen: 退化系 — `[Circle, Circle]` (2 closed primitives) は reject される
///
/// 注意: 現行 (pre-fix) コードでも make_extrusion downstream の is_convex/is_simple
/// チェックで同 error variant を返すため pre-fix でも pass する (downstream luck)。
/// post-fix は validate_sketch_profile_contours で早期に reject する設計で、
/// 本テストは「明示的契約 vs downstream 偶然依存」の regression guard として機能する。
#[test]
fn t03_degen_two_circles_rejected() {
    let profile = vec![
        SketchElement::Circle {
            id: "c1".to_string(),
            center: [0.0, 0.0],
            radius: 2.0,
        },
        SketchElement::Circle {
            id: "c2".to_string(),
            center: [10.0, 0.0],
            radius: 2.0,
        },
    ];
    let result = build_with_profile(profile);
    assert!(
        matches!(
            result,
            Err(KernelError::InvalidParameter { kind: "profile" })
        ),
        "expected InvalidParameter {{ kind: \"profile\" }} but got {:?}",
        result
    );
}

/// T04_boundary_reject: 境界系 — `[Circle, Line]` (closed + open mix) も reject
#[test]
fn t04_boundary_reject_circle_plus_line() {
    let profile = vec![
        SketchElement::Circle {
            id: "c1".to_string(),
            center: [0.0, 0.0],
            radius: 2.0,
        },
        SketchElement::Line {
            id: "l1".to_string(),
            from: [5.0, 0.0],
            to: [10.0, 0.0],
        },
    ];
    let result = build_with_profile(profile);
    assert!(
        matches!(
            result,
            Err(KernelError::InvalidParameter { kind: "profile" })
        ),
        "expected InvalidParameter {{ kind: \"profile\" }} but got {:?}",
        result
    );
}

/// T05: ExtrudeCut 経路でも同 guard が効くこと
///
/// plan.md 通り `[Circle, Line]` で reject されることを確認 (Codex round 2 M-F02 で
/// `[Circle, Circle]` は downstream luck で同 error variant を返す余地があるため
/// guard 固有の固定にならないと指摘された)。`[Circle, Line]` は guard を外すと
/// downstream で `is_convex` / line-overshoot 等別経路の error variant に落ちる
/// 可能性があり、本ケースは ExtrudeCut の call site (`lib.rs:367`) を明示的に固定する。
#[test]
fn t05_extrudecut_rejects_circle_plus_line() {
    let profile = vec![
        SketchElement::Circle {
            id: "c1".to_string(),
            center: [0.0, 0.0],
            radius: 2.0,
        },
        SketchElement::Line {
            id: "l1".to_string(),
            from: [5.0, 0.0],
            to: [10.0, 0.0],
        },
    ];
    let result = build_with_profile_extrude_cut(profile);
    assert!(
        matches!(
            result,
            Err(KernelError::InvalidParameter { kind: "profile" })
        ),
        "expected InvalidParameter {{ kind: \"profile\" }} for ExtrudeCut but got {:?}",
        result
    );
}

/// T06_mid_closed: 境界系 — `[Line, Circle, Line]` (中央に closed、3 要素) → reject
///
/// `len > 1 && any(closed)` の条件確認。closed primitive が複数要素中のどこに
/// あっても guard が機能することを検証する。
#[test]
fn t06_mid_closed_line_circle_line_rejected() {
    let profile = vec![
        SketchElement::Line {
            id: "l1".to_string(),
            from: [-5.0, 0.0],
            to: [-2.0, 0.0],
        },
        SketchElement::Circle {
            id: "c1".to_string(),
            center: [0.0, 0.0],
            radius: 2.0,
        },
        SketchElement::Line {
            id: "l2".to_string(),
            from: [2.0, 0.0],
            to: [5.0, 0.0],
        },
    ];
    let result = build_with_profile(profile);
    assert!(
        matches!(
            result,
            Err(KernelError::InvalidParameter { kind: "profile" })
        ),
        "expected InvalidParameter {{ kind: \"profile\" }} but got {:?}",
        result
    );
}

/// T07_two_open_ok: 正常系 — `[Line, Arc]` (2 要素・全 open) → guard 通過
///
/// guard の条件は `len > 1 && any(closed)` のため、open primitive のみでは
/// 通過する。downstream の make_extrusion が別 error (n < 3 等) を出す可能性は
/// あるが、guard レベルでは `InvalidParameter { kind: "profile" }` は返されない。
#[test]
fn t07_two_open_ok_line_arc_not_profile_error() {
    let profile = vec![
        SketchElement::Line {
            id: "l1".to_string(),
            from: [0.0, 0.0],
            to: [2.0, 0.0],
        },
        SketchElement::Arc {
            id: "a1".to_string(),
            center: [2.0, 2.0],
            radius: 2.0,
            start_angle: -std::f64::consts::FRAC_PI_2,
            end_angle: 0.0,
        },
    ];
    let result = build_with_profile(profile);
    // guard では profile error が出ないこと（別 error or OK は問わない）
    assert!(
        !matches!(
            result,
            Err(KernelError::InvalidParameter { kind: "profile" })
        ),
        "guard should not reject open primitives: got {:?}",
        result
    );
}

/// T08_single_ellipse_ok: 正常系 — `[Ellipse]` (単一 closed primitive) → guard 通過
///
/// `len > 1` が false のため guard 通過。単一 closed primitive は Phase 10 の
/// 通常用法として許可される。
#[test]
fn t08_single_ellipse_ok() {
    let result = build_with_profile(vec![SketchElement::Ellipse {
        id: "e1".to_string(),
        center: [0.0, 0.0],
        major: 5.0,
        minor: 3.0,
        rotation: 0.0,
    }]);
    assert!(
        !matches!(
            result,
            Err(KernelError::InvalidParameter { kind: "profile" })
        ),
        "single Ellipse should pass guard: got {:?}",
        result
    );
}

/// T09_empty_profile: 退化系 — `[]` (空 profile) → downstream で reject
///
/// `len > 1` が false のため guard 関数は通過するが、downstream の
/// make_extrusion 側で n < 3 check が `InvalidParameter { kind: "profile" }`
/// を返す既存挙動を維持する regression test。
#[test]
fn t09_empty_profile_downstream_rejects() {
    let result = build_with_profile(vec![]);
    assert!(
        matches!(
            result,
            Err(KernelError::InvalidParameter { kind: "profile" })
        ),
        "empty profile should be rejected by downstream: got {:?}",
        result
    );
}

/// T10_full_arc_rejected: 境界系 — `[Arc(0, 2π), Line]` → reject
///
/// sweep = 2π の Arc は full-circle 相当として closed 扱いし、
/// 他の要素と組み合わさると guard で reject される。
#[test]
fn t10_full_arc_rejected() {
    use std::f64::consts::TAU;
    let profile = vec![
        SketchElement::Arc {
            id: "a1".to_string(),
            center: [0.0, 0.0],
            radius: 2.0,
            start_angle: 0.0,
            end_angle: TAU,
        },
        SketchElement::Line {
            id: "l1".to_string(),
            from: [3.0, 0.0],
            to: [5.0, 0.0],
        },
    ];
    let result = build_with_profile(profile);
    assert!(
        matches!(
            result,
            Err(KernelError::InvalidParameter { kind: "profile" })
        ),
        "expected InvalidParameter {{ kind: \"profile\" }} but got {:?}",
        result
    );
}

/// T11_two_full_arcs_rejected: 境界系 — `[Arc(0, 2π), Arc(0, 2π)]` → reject
///
/// 複数の full-circle Arc も closed primitive 複数として reject される。
#[test]
fn t11_two_full_arcs_rejected() {
    use std::f64::consts::TAU;
    let profile = vec![
        SketchElement::Arc {
            id: "a1".to_string(),
            center: [0.0, 0.0],
            radius: 2.0,
            start_angle: 0.0,
            end_angle: TAU,
        },
        SketchElement::Arc {
            id: "a2".to_string(),
            center: [5.0, 0.0],
            radius: 2.0,
            start_angle: 0.0,
            end_angle: TAU,
        },
    ];
    let result = build_with_profile(profile);
    assert!(
        matches!(
            result,
            Err(KernelError::InvalidParameter { kind: "profile" })
        ),
        "expected InvalidParameter {{ kind: \"profile\" }} but got {:?}",
        result
    );
}

/// T12_partial_arc_ok: 正常系 — `[Arc(0, π/2), Line]` (sweep < 2π) → guard 通過
///
/// 部分 Arc (sweep < 2π) は open primitive として扱われるため、`len > 1` でも
/// `any(closed)` が false となり guard を通過する。downstream の挙動は問わない
/// (別 error or Ok)、guard が profile error を返さないことのみを assert。
#[test]
fn t12_partial_arc_ok_quarter_arc_line() {
    use std::f64::consts::FRAC_PI_2;
    let profile = vec![
        SketchElement::Arc {
            id: "a1".to_string(),
            center: [0.0, 0.0],
            radius: 2.0,
            start_angle: 0.0,
            end_angle: FRAC_PI_2,
        },
        SketchElement::Line {
            id: "l1".to_string(),
            from: [0.0, 2.0],
            to: [0.0, 0.0],
        },
    ];
    let result = build_with_profile(profile);
    assert!(
        !matches!(
            result,
            Err(KernelError::InvalidParameter { kind: "profile" })
        ),
        "partial arc + line should pass guard (sweep < 2π): got {:?}",
        result
    );
}

/// T13_negative_full_arc_rejected: 境界系 — `[Arc(2π, 0), Line]` (逆方向 sweep) → reject
///
/// `sweep = (end_angle - start_angle).abs()` の abs 処理を担保する regression test。
/// 逆方向の full-circle sweep も closed 扱いで reject される。
#[test]
fn t13_negative_full_arc_rejected() {
    use std::f64::consts::TAU;
    let profile = vec![
        SketchElement::Arc {
            id: "a1".to_string(),
            center: [0.0, 0.0],
            radius: 2.0,
            start_angle: TAU,
            end_angle: 0.0,
        },
        SketchElement::Line {
            id: "l1".to_string(),
            from: [3.0, 0.0],
            to: [5.0, 0.0],
        },
    ];
    let result = build_with_profile(profile);
    assert!(
        matches!(
            result,
            Err(KernelError::InvalidParameter { kind: "profile" })
        ),
        "expected InvalidParameter {{ kind: \"profile\" }} but got {:?}",
        result
    );
}
