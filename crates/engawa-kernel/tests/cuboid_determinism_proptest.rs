//! Property-based tests for make_cuboid determinism (#243).
//!
//! Tests: T01 (determinism), T_BOUNDARY (minimal dimensions), T_DEG (zero dim excluded).

use engawa_kernel::brep::topology::IdGenerator;
use engawa_kernel::primitives::make_cuboid;
use engawa_kernel::tessellation::tessellate_solid;
use proptest::prelude::*;

// ---------------------------------------------------------------------------
// T01: Determinism — same inputs produce identical outputs (256 cases)
// ---------------------------------------------------------------------------
proptest! {
    #[test]
    fn t01_proptest_make_cuboid_determinism(
        w in 0.01_f64..1000.0_f64,
        h in 0.01_f64..1000.0_f64,
        d in 0.01_f64..1000.0_f64,
    ) {
        let seed = 42;
        let build = || {
            let mut gen = IdGenerator::new(seed);
            make_cuboid(w, h, d, "cuboid", &mut gen).unwrap()
        };

        let a = build();
        let b = build();

        prop_assert_eq!(format!("{:?}", a), format!("{:?}", b));
    }
}

// ---------------------------------------------------------------------------
// T_BOUNDARY: Minimal dimension (0.01) does not cause panic
// ---------------------------------------------------------------------------
#[test]
fn t_boundary_proptest_minimal_dim() {
    let mut gen = IdGenerator::new(1);
    let result = make_cuboid(0.01, 0.01, 0.01, "cuboid", &mut gen);
    assert!(result.is_ok(), "minimal dimensions should succeed");
    let solid = result.unwrap();
    let mesh = tessellate_solid(&solid);
    assert!(mesh.is_ok(), "minimal cuboid should tessellate");
}

// ---------------------------------------------------------------------------
// T_DEG: Zero dimensions are intentionally excluded (range starts at 0.01)
// ---------------------------------------------------------------------------
#[test]
fn t_deg_zero_dim_excluded() {
    let mut gen = IdGenerator::new(1);
    // Zero dimension should be rejected (degenerate input)
    assert!(
        make_cuboid(0.0, 1.0, 1.0, "test", &mut gen).is_err(),
        "zero width should be rejected"
    );
}
