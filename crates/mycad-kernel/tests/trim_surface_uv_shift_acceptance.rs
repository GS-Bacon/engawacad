//! Tests for Issue #136: periodic_u_shift implementation.
//!
//! T01-T08: unit tests for periodic_u_shift function
//! T09: regression test for existing boolean Cut path

use mycad_kernel::brep::topology::IdGenerator;
use mycad_kernel::geometry::Point;
use mycad_kernel::primitives::{make_cuboid, make_cylinder};
use mycad_kernel::tessellation::periodic_u_shift;
use std::f64::consts::PI;

// ── T01-T08: periodic_u_shift unit tests ────────────────────────────────────────

/// T01: 決定性 — 同一入力で 2 回呼び、結果が完全一致。
#[test]
fn t01_determinism() {
    let outer = vec![0.0, PI, 2.0 * PI];
    let mut inner1 = vec![0.1, 0.2, 0.3];
    let mut inner2 = inner1.clone();

    periodic_u_shift(&outer, &mut inner1);
    periodic_u_shift(&outer, &mut inner2);

    assert_eq!(inner1, inner2, "periodic_u_shift must be deterministic");
}

/// T02: outer 中央 0, inner avg 0 → shift = 0 (no shift)。
#[test]
fn t02_outer_center_0_inner_0_no_shift() {
    // outer u span: [-0.1, 0.1] → center = 0.0
    let outer = vec![-0.1, 0.0, 0.1];
    let mut inner = vec![0.0, 0.05, -0.05]; // avg ≈ 0.0

    let inner_orig = inner.clone();
    periodic_u_shift(&outer, &mut inner);

    assert_eq!(inner, inner_orig, "no shift expected when centers align");
}

/// T03: outer 中央 π/2, inner avg π/2 → shift = 0。
#[test]
fn t03_outer_center_pi_2_inner_pi_2_no_shift() {
    let outer = vec![PI / 2.0 - 0.1, PI / 2.0, PI / 2.0 + 0.1];
    let mut inner = vec![PI / 2.0 - 0.05, PI / 2.0 + 0.05];

    let inner_orig = inner.clone();
    periodic_u_shift(&outer, &mut inner);

    assert_eq!(inner, inner_orig, "no shift expected for same center");
}

/// T04: outer 中央 0, inner avg 0.5 → shift = 0 (|raw| < π)。
#[test]
fn t04_small_diff_no_shift() {
    let outer = vec![-PI + 0.1, 0.0, PI - 0.1]; // center = 0
    let mut inner = vec![0.5]; // avg = 0.5, raw_shift = -0.5 (< π)

    let inner_orig = inner.clone();
    periodic_u_shift(&outer, &mut inner);

    assert_eq!(inner, inner_orig, "|raw_shift| < π should produce no shift");
}

/// T05_boundary_seam_pos: outer 中央 = π/2, inner avg = -3π/2 (= π/2 - 2π) → shift = +2π。
#[test]
fn t05_boundary_seam_pos_plus_2pi() {
    let outer = vec![0.0, PI / 2.0, PI]; // center = π/2
    let mut inner = vec![-3.0 * PI / 2.0]; // avg = -3π/2 (= π/2 - 2π周期表現)

    periodic_u_shift(&outer, &mut inner);

    // After shift: -3π/2 + 2π = π/2 (matches outer center)
    let expected = PI / 2.0;
    assert!(
        (inner[0] - expected).abs() < 1e-10,
        "shifted inner should align with outer center, got {}",
        inner[0]
    );
}

/// T06_boundary_seam_neg: outer 中央 = -π/2, inner avg = 3π/2 → shift = -2π。
#[test]
fn t06_boundary_seam_neg_minus_2pi() {
    let outer = vec![-PI, -PI / 2.0, 0.0]; // center = -π/2
    let mut inner = vec![3.0 * PI / 2.0]; // avg = 3π/2 (= -π/2 + 2π)

    periodic_u_shift(&outer, &mut inner);

    // After shift: 3π/2 - 2π = -π/2 (matches outer center)
    let expected = -PI / 2.0;
    assert!(
        (inner[0] - expected).abs() < 1e-10,
        "shifted inner should align with outer center, got {}",
        inner[0]
    );
}

/// T07_degen_empty_outer: outer 空 → no-op (inner unchanged)。
#[test]
fn t07_degen_empty_outer_no_op() {
    let outer: Vec<f64> = vec![];
    let mut inner = vec![0.5, 1.0, 1.5];

    let inner_orig = inner.clone();
    periodic_u_shift(&outer, &mut inner);

    assert_eq!(
        inner, inner_orig,
        "empty outer should leave inner unchanged"
    );
}

/// T08_boundary_inner_centered: outer 中央 = inner avg → shift exact 0.0 (浮動小数ノイズなし)。
#[test]
fn t08_boundary_inner_centered_exact_zero_shift() {
    let outer = vec![-1.0, 0.0, 1.0]; // center = 0.0
    let mut inner = vec![0.0]; // avg = 0.0 (exact match)

    periodic_u_shift(&outer, &mut inner);

    assert_eq!(inner[0], 0.0, "exact center match should produce 0.0 shift");
}

// ── T09: 回帰テスト ─────────────────────────────────────────────────────────────

/// T09: #130 既存 boolean Cut 経路 (`box - cylinder Cut`) が依然 pass。
#[test]
fn t09_box_cut_regression() {
    use mycad_kernel::booleans::{boolean, BooleanOp};
    use mycad_kernel::tessellation::tessellate_solid;

    let mut gen = IdGenerator::new(0);
    let box_solid = make_cuboid(4.0, 4.0, 4.0, &mut gen).expect("cuboid");
    let cyl = make_cylinder(1.0, 6.0, Point::new(0.0, 0.0, -3.0), &mut gen).expect("cylinder");
    let result = boolean(&box_solid, &cyl, BooleanOp::Cut, &mut gen).expect("cut");
    let mesh = tessellate_solid(&result).expect("tessellate");

    // Mesh should be non-empty and produce triangles
    assert!(mesh.triangle_count() > 0, "T09: mesh must be non-empty");
}
