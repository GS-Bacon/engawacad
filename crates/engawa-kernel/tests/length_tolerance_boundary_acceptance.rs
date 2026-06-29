//! Acceptance tests for #289: 退化判定境界 `<=` 統一規約 (ADR-018 draft).
//!
//! STEP 6.6 で GLM-test-implementer が実装。
//!
//! Ellipse テストの注意:
//! - ADR-017 §1 で `major >= minor` 不変条件がある
//! - `EPS_AXIS_RATIO = 1e-6` 判定がある（本 Issue スコープ外）
//! - これらの制約を満たすようテスト値を調整している

use engawa_format::SketchElement;
use engawa_kernel::geometry::math::LENGTH_TOLERANCE;
use engawa_kernel::tessellation::sketch::tessellate_sketch_element;

/// T01: 決定性 — 同一入力で完全一致する出力を得る。
#[test]
fn t01_determinism() {
    let circle = SketchElement::Circle {
        id: "c0".into(),
        center: [0.0, 0.0],
        radius: 1.0,
    };
    let a = tessellate_sketch_element(&circle, 32).unwrap();
    let b = tessellate_sketch_element(&circle, 32).unwrap();
    assert_eq!(a, b, "同一入力で同一出力が必要");
}

/// T_BOUNDARY_above_tolerance_circle: radius = LENGTH_TOLERANCE * 1.000_001 で pass。
#[test]
fn t_boundary_above_tolerance_circle() {
    let circle = SketchElement::Circle {
        id: "c0".into(),
        center: [0.0, 0.0],
        radius: LENGTH_TOLERANCE * 1.000_001,
    };
    let result = tessellate_sketch_element(&circle, 32);
    assert!(result.is_ok(), "境界直上は許容されるべき");
    let points = result.unwrap();
    assert_eq!(points.len(), 32, "Circle は 32 点を生成");
}

/// T_BOUNDARY_above_tolerance_arc: radius = LENGTH_TOLERANCE * 1.000_001 で pass。
#[test]
fn t_boundary_above_tolerance_arc() {
    let arc = SketchElement::Arc {
        id: "a0".into(),
        center: [0.0, 0.0],
        radius: LENGTH_TOLERANCE * 1.000_001,
        start_angle: 0.0,
        end_angle: std::f64::consts::PI / 2.0,
    };
    let result = tessellate_sketch_element(&arc, 32);
    assert!(result.is_ok(), "境界直上は許容されるべき");
    let points = result.unwrap();
    assert_eq!(points.len(), 8, "Arc は 8 点を生成 (90度/360度 * 32)");
}

/// T_BOUNDARY_above_tolerance_ellipse_major: major = LENGTH_TOLERANCE * 1.000_001 passes。
/// 偽陽性ガード: `<` strict に戻すとテストが fail する。
#[test]
fn t_boundary_above_tolerance_ellipse_major() {
    let major = LENGTH_TOLERANCE * 1.000_001;
    let ellipse = SketchElement::Ellipse {
        id: "e1".into(),
        center: [0.0, 0.0],
        major,
        minor: LENGTH_TOLERANCE * 1.000_001,
        rotation: 0.0,
    };
    let result = tessellate_sketch_element(&ellipse, 32);
    assert!(result.is_ok(), "境界直上は許容されるべき");
    let points = result.unwrap();
    assert_eq!(points.len(), 32, "Ellipse は 32 点を生成");
}

/// T_BOUNDARY_above_tolerance_ellipse_minor: minor = LENGTH_TOLERANCE * 1.000_001 passes。
/// 偽陽性ガード: `<` strict に戻すとテストが fail する。
#[test]
fn t_boundary_above_tolerance_ellipse_minor() {
    let major = LENGTH_TOLERANCE * 1.000_001;
    let ellipse = SketchElement::Ellipse {
        id: "e1".into(),
        center: [0.0, 0.0],
        major,
        minor: LENGTH_TOLERANCE * 1.000_001,
        rotation: 0.0,
    };
    let result = tessellate_sketch_element(&ellipse, 32);
    assert!(result.is_ok(), "境界直上は許容されるべき");
    let points = result.unwrap();
    assert_eq!(points.len(), 32, "Ellipse は 32 点を生成");
}

/// T_BOUNDARY_exact_tolerance_circle: radius == LENGTH_TOLERANCE rejected (`<=` convention)。
/// 偽陽性ガード: `<` strict に戻すとテストが fail する（reason 一致）。
#[test]
fn t_boundary_exact_tolerance_circle() {
    let circle = SketchElement::Circle {
        id: "c0".into(),
        center: [0.0, 0.0],
        radius: LENGTH_TOLERANCE,
    };
    let result = tessellate_sketch_element(&circle, 32);
    assert!(
        matches!(
            result,
            Err(engawa_kernel::error::KernelError::DegenerateSketchElement {
                reason,
                ..
            }) if reason == "radius <= ε_radius"
        ),
        "radius == TOL は退化と判定されるべき (`<=` 規約, reason 一致)"
    );
}

/// T_BOUNDARY_exact_tolerance_arc: radius == LENGTH_TOLERANCE rejected (`<=` convention)。
/// 偽陽性ガード: `<` strict に戻すとテストが fail する（reason 一致）。
#[test]
fn t_boundary_exact_tolerance_arc() {
    let arc = SketchElement::Arc {
        id: "a0".into(),
        center: [0.0, 0.0],
        radius: LENGTH_TOLERANCE,
        start_angle: 0.0,
        end_angle: std::f64::consts::PI / 2.0,
    };
    let result = tessellate_sketch_element(&arc, 32);
    assert!(
        matches!(
            result,
            Err(engawa_kernel::error::KernelError::DegenerateSketchElement {
                reason,
                ..
            }) if reason == "radius <= ε_radius"
        ),
        "radius == TOL は退化と判定されるべき (reason 一致)"
    );
}

/// T_BOUNDARY_exact_tolerance_ellipse_major: major == LENGTH_TOLERANCE rejected (`<=` convention)。
/// 偽陽性ガード: `<` strict に戻すとテストが fail する（reason 一致）。
#[test]
fn t_boundary_exact_tolerance_ellipse_major() {
    let ellipse = SketchElement::Ellipse {
        id: "e0".into(),
        center: [0.0, 0.0],
        major: LENGTH_TOLERANCE,
        minor: LENGTH_TOLERANCE,
        rotation: 0.0,
    };
    let result = tessellate_sketch_element(&ellipse, 32);
    assert!(
        matches!(
            result,
            Err(engawa_kernel::error::KernelError::DegenerateSketchElement {
                reason,
                ..
            }) if reason == "major <= ε_radius"
        ),
        "major == TOL は退化と判定されるべき (reason 一致)"
    );
}

/// T_BOUNDARY_exact_tolerance_ellipse_minor: minor == LENGTH_TOLERANCE rejected (`<=` convention)。
/// 偽陽性ガード: `<` strict に戻すとテストが fail する（reason 一致）。
#[test]
fn t_boundary_exact_tolerance_ellipse_minor() {
    let ellipse = SketchElement::Ellipse {
        id: "e0".into(),
        center: [0.0, 0.0],
        major: LENGTH_TOLERANCE * 1.000_001,
        minor: LENGTH_TOLERANCE,
        rotation: 0.0,
    };
    let result = tessellate_sketch_element(&ellipse, 32);
    assert!(
        matches!(
            result,
            Err(engawa_kernel::error::KernelError::DegenerateSketchElement {
                reason,
                ..
            }) if reason == "minor <= ε_radius"
        ),
        "minor == TOL は退化と判定されるべき (reason 一致)"
    );
}

/// T_DEG_below_tolerance: radius = LENGTH_TOLERANCE / 2.0 で reject (既存挙動回帰)。
#[test]
fn t_deg_below_tolerance() {
    let circle = SketchElement::Circle {
        id: "c0".into(),
        center: [0.0, 0.0],
        radius: LENGTH_TOLERANCE / 2.0,
    };
    let result = tessellate_sketch_element(&circle, 32);
    assert!(
        matches!(
            result,
            Err(engawa_kernel::error::KernelError::DegenerateSketchElement {
                reason,
                ..
            }) if reason == "radius <= ε_radius"
        ),
        "TOL/2 は退化と判定されるべき (reason 一致)"
    );
}
