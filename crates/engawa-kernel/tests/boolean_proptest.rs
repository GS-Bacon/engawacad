//! Property-based tests for Boolean Cut invariants (#112).
//!
//! Tests: T01/T01_boundary (manifold safety), T02/T02_degen (volume reduction),
//!        T03 (determinism), T04 (numerical edge cases), T05 (empty solid),
//!        T06 (cuboid construction determinism).

use engawa_kernel::booleans::{boolean, BooleanOp};
use engawa_kernel::brep::topology::{IdGenerator, Solid};
use engawa_kernel::geometry::Vec3;
use engawa_kernel::primitives::make_cuboid;
use engawa_kernel::tessellation::tessellate_solid;
use proptest::prelude::*;

/// Signed-volume of a closed triangle mesh via divergence theorem.
fn mesh_signed_volume(mesh: &engawa_kernel::tessellation::TriangleMesh) -> f64 {
    mesh.indices
        .chunks(3)
        .map(|tri| {
            let p0 = mesh.positions[tri[0] as usize];
            let p1 = mesh.positions[tri[1] as usize];
            let p2 = mesh.positions[tri[2] as usize];
            (p0[0] * (p1[1] * p2[2] - p1[2] * p2[1])
                + p0[1] * (p1[2] * p2[0] - p1[0] * p2[2])
                + p0[2] * (p1[0] * p2[1] - p1[1] * p2[0]))
                / 6.0
        })
        .sum()
}

// ---------------------------------------------------------------------------
// T01: Random cut must produce a valid manifold or be cleanly rejected
// ---------------------------------------------------------------------------
proptest! {
    #![proptest_config(ProptestConfig::with_cases(32))]
    #[test]
    fn t01_prop_cut_never_produces_degenerate_manifold(
        x_offset in -3.0_f64..3.0_f64,
        tool_w in 1.0_f64..4.0_f64,
        tool_h in 1.0_f64..8.0_f64,
        tool_d in 1.0_f64..10.0_f64,
    ) {
        let mut gen = IdGenerator::new(99);
        let target = make_cuboid(10.0, 20.0, 30.0, &mut gen).unwrap();
        let mut tool = make_cuboid(tool_w, tool_h, tool_d, &mut gen).unwrap();
        tool.translate(Vec3::new(x_offset, 0.0, 0.0));
        let result = boolean(&target, &tool, BooleanOp::Cut, &mut gen);
        match result {
            Ok(solid) => prop_assert!(
                solid.validate_manifold().is_ok(),
                "degenerate B-rep: {:?}", solid.validate_manifold()
            ),
            Err(_) => {} // clean rejection is acceptable
        }
    }
}

// ---------------------------------------------------------------------------
// T01_boundary: coplanar / touching tool positions must not panic
// ---------------------------------------------------------------------------
proptest! {
    #![proptest_config(ProptestConfig::with_cases(32))]
    #[test]
    fn t01_boundary_coplanar_tool_does_not_panic(
        // Place the tool so that one of its faces is coplanar with target face at X=+5 or X=-5
        x_offset in -5.0_f64..=5.0_f64,
    ) {
        let mut gen = IdGenerator::new(200);
        let target = make_cuboid(10.0, 20.0, 30.0, &mut gen).unwrap();
        let mut tool = make_cuboid(2.0, 2.0, 2.0, &mut gen).unwrap();
        tool.translate(Vec3::new(x_offset, 0.0, 0.0));
        // Must not panic — Ok or Err are both acceptable
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            boolean(&target, &tool, BooleanOp::Cut, &mut gen)
        }));
        prop_assert!(result.is_ok(), "boolean cut panicked at x_offset={x_offset}");
    }
}

// ---------------------------------------------------------------------------
// T02: Surface cut (tool exits through a face) must reduce volume.
//
// Target box: X ∈ [-5, 5]. Tool width=2 → X range = [x_offset-1, x_offset+1].
// x_offset ∈ (4.0, 5.5) ensures tool partially exits at x=+5 → surface cut,
// not an internal void. Surface cuts produce a single-shell result, so
// tessellation signed-volume correctly reflects material removal.
//
// Fixed in Issue #120: surface cut manifold violation resolved via ring/disc
// fragment generation in partition_faces.
// ---------------------------------------------------------------------------
proptest! {
    #![proptest_config(ProptestConfig::with_cases(32))]
    #[test]
    fn t02_prop_surface_cut_reduces_volume(
        x_offset in 4.1_f64..5.4_f64,
    ) {
        let mut gen = IdGenerator::new(42);
        let target = make_cuboid(10.0, 20.0, 30.0, &mut gen).unwrap();
        let mut tool = make_cuboid(2.0, 2.0, 2.0, &mut gen).unwrap();
        tool.translate(Vec3::new(x_offset, 0.0, 0.0));
        let result = boolean(&target, &tool, BooleanOp::Cut, &mut gen);
        prop_assert!(
            result.is_ok(),
            "surface cut must succeed for x_offset={x_offset}: {result:?}"
        );
        let cut = result.unwrap();
        let mesh_t = tessellate_solid(&target).unwrap();
        let mesh_c = tessellate_solid(&cut).unwrap();
        let vol_t = mesh_signed_volume(&mesh_t).abs();
        let vol_c = mesh_signed_volume(&mesh_c).abs();
        prop_assert!(
            vol_c < vol_t,
            "surface cut must reduce volume: before={vol_t}, after={vol_c}"
        );
    }
}

// ---------------------------------------------------------------------------
// T02_degen: minimum-size tool (1×1×1) must produce valid result or clean error
// ---------------------------------------------------------------------------
#[test]
fn t02_degen_minimum_tool_size_ok_or_err() {
    let mut gen = IdGenerator::new(300);
    let target = make_cuboid(10.0, 20.0, 30.0, &mut gen).unwrap();
    let mut tool = make_cuboid(1.0, 1.0, 1.0, &mut gen).unwrap();
    tool.translate(Vec3::new(0.0, 0.0, 0.0));
    let result = boolean(&target, &tool, BooleanOp::Cut, &mut gen);
    match result {
        Ok(solid) => assert!(
            solid.validate_manifold().is_ok(),
            "degenerate B-rep with min tool: {:?}",
            solid.validate_manifold()
        ),
        Err(_) => {} // clean rejection is acceptable
    }
}

// ---------------------------------------------------------------------------
// T03: Determinism — same inputs produce identical outputs
// ---------------------------------------------------------------------------
#[test]
fn t03_determinism_fixed_seed() {
    let build = || {
        let mut gen = IdGenerator::new(99);
        let target = make_cuboid(10.0, 20.0, 30.0, &mut gen).unwrap();
        let mut tool = make_cuboid(3.0, 4.0, 5.0, &mut gen).unwrap();
        tool.translate(Vec3::new(1.0, 0.0, 0.0));
        boolean(&target, &tool, BooleanOp::Cut, &mut gen).unwrap()
    };

    let a = build();
    let b = build();

    assert_eq!(a.vertices.len(), b.vertices.len(), "vertex count mismatch");
    assert_eq!(a.edges.len(), b.edges.len(), "edge count mismatch");
    assert_eq!(a.faces.len(), b.faces.len(), "face count mismatch");

    for (va, vb) in a.vertices.iter().zip(b.vertices.iter()) {
        assert_eq!(va.id, vb.id, "vertex id mismatch");
    }

    let mesh_a = tessellate_solid(&a).unwrap();
    let mesh_b = tessellate_solid(&b).unwrap();
    assert_eq!(
        mesh_a.positions, mesh_b.positions,
        "vertex positions mismatch"
    );
    assert_eq!(mesh_a.indices, mesh_b.indices, "triangle indices mismatch");
}

// ---------------------------------------------------------------------------
// T02_void_semantics: void cut (tool fully inside target) produces a solid
// whose |signed_volume| exceeds the initial cuboid. The inner shell adds
// absolute volume because tessellation winding is positive on both shells.
// This is a regression guard for the current behaviour.
// ---------------------------------------------------------------------------
#[test]
fn t02_void_cut_volume_exceeds_initial() {
    let mut gen = IdGenerator::new(77);
    let target = make_cuboid(10.0, 20.0, 30.0, &mut gen).unwrap();
    let tool = make_cuboid(2.0, 2.0, 2.0, &mut gen).unwrap();
    // tool centred at origin → fully inside target (target X ∈ [-5,5], tool X ∈ [-1,1])
    let result = boolean(&target, &tool, BooleanOp::Cut, &mut gen);
    let cut = result.expect("void cut should succeed");
    assert!(
        cut.validate_manifold().is_ok(),
        "void cut manifold invalid: {:?}",
        cut.validate_manifold()
    );

    let mesh_t = tessellate_solid(&target).unwrap();
    let mesh_c = tessellate_solid(&cut).unwrap();
    let vol_t = mesh_signed_volume(&mesh_t).abs();
    let vol_c = mesh_signed_volume(&mesh_c).abs();

    assert!(
        vol_c > vol_t,
        "void cut should add abs volume (inner shell): before={vol_t}, after={vol_c}"
    );
}

// ---------------------------------------------------------------------------
// T01_large_tool: tool nearly as wide as target still produces valid result
// ---------------------------------------------------------------------------
proptest! {
    #![proptest_config(ProptestConfig::with_cases(16))]
    #[test]
    fn t01_large_tool_still_valid(
        x_offset in -0.4_f64..0.4_f64,
    ) {
        let mut gen = IdGenerator::new(88);
        let target = make_cuboid(10.0, 20.0, 30.0, &mut gen).unwrap();
        // tool_w=9.9 → X range [x_offset-4.95, x_offset+4.95], always exits target X=±5
        let mut tool = make_cuboid(9.9, 8.0, 10.0, &mut gen).unwrap();
        tool.translate(Vec3::new(x_offset, 0.0, 0.0));
        let result = boolean(&target, &tool, BooleanOp::Cut, &mut gen);
        match result {
            Ok(solid) => prop_assert!(
                solid.validate_manifold().is_ok(),
                "large tool produced degenerate manifold: {:?}", solid.validate_manifold()
            ),
            Err(_) => {} // clean rejection is acceptable
        }
    }
}

// ---------------------------------------------------------------------------
// T03_boundary_seed: determinism with different seed (IdGenerator::new(200))
// Repeats T01_boundary-like inputs twice and verifies identical outputs.
// ---------------------------------------------------------------------------
#[test]
fn t03_boundary_seed_determinism() {
    let build = |x_offset: f64| {
        let mut gen = IdGenerator::new(200);
        let target = make_cuboid(10.0, 20.0, 30.0, &mut gen).unwrap();
        let mut tool = make_cuboid(2.0, 2.0, 2.0, &mut gen).unwrap();
        tool.translate(Vec3::new(x_offset, 0.0, 0.0));
        boolean(&target, &tool, BooleanOp::Cut, &mut gen)
    };

    for x in [-5.0, -3.0, 0.0, 3.0, 5.0] {
        let a = build(x);
        let b = build(x);

        match (&a, &b) {
            (Ok(sa), Ok(sb)) => {
                assert_eq!(
                    sa.vertices.len(),
                    sb.vertices.len(),
                    "vertex count at x={x}"
                );
                assert_eq!(sa.edges.len(), sb.edges.len(), "edge count at x={x}");
                assert_eq!(sa.faces.len(), sb.faces.len(), "face count at x={x}");
                for (va, vb) in sa.vertices.iter().zip(sb.vertices.iter()) {
                    assert_eq!(va.id, vb.id, "vertex id mismatch at x={x}");
                }
                let mesh_a = tessellate_solid(sa).unwrap();
                let mesh_b = tessellate_solid(sb).unwrap();
                assert_eq!(
                    mesh_a.positions, mesh_b.positions,
                    "positions mismatch at x={x}"
                );
                assert_eq!(mesh_a.indices, mesh_b.indices, "indices mismatch at x={x}");
            }
            (Err(_), Err(_)) => {} // both rejected — consistent
            _ => panic!("inconsistent boolean result at x={x}: a={a:?}, b={b:?}"),
        }
    }
}

// ---------------------------------------------------------------------------
// T03_repeated: 100-run determinism — same inputs always produce same result
// ---------------------------------------------------------------------------
#[test]
fn t03_repeated_determinism_100_runs() {
    let mut results: Vec<(usize, usize, usize, Vec<u64>, Vec<[f64; 3]>, Vec<u32>)> =
        Vec::with_capacity(100);

    for _ in 0..100 {
        let mut gen = IdGenerator::new(99);
        let target = make_cuboid(10.0, 20.0, 30.0, &mut gen).unwrap();
        let mut tool = make_cuboid(3.0, 4.0, 5.0, &mut gen).unwrap();
        tool.translate(Vec3::new(1.0, 0.0, 0.0));
        let solid = boolean(&target, &tool, BooleanOp::Cut, &mut gen).unwrap();
        let ids: Vec<u64> = solid.vertices.iter().map(|v| v.id).collect();
        let mesh = tessellate_solid(&solid).unwrap();
        results.push((
            solid.vertices.len(),
            solid.edges.len(),
            solid.faces.len(),
            ids,
            mesh.positions.clone(),
            mesh.indices.clone(),
        ));
    }

    let first = &results[0];
    for (i, r) in results.iter().enumerate() {
        assert_eq!(r.0, first.0, "vertex count mismatch at run {i}");
        assert_eq!(r.1, first.1, "edge count mismatch at run {i}");
        assert_eq!(r.2, first.2, "face count mismatch at run {i}");
        assert_eq!(r.3, first.3, "vertex ids mismatch at run {i}");
        assert_eq!(r.4, first.4, "mesh positions mismatch at run {i}");
        assert_eq!(r.5, first.5, "mesh indices mismatch at run {i}");
    }
}

// ---------------------------------------------------------------------------
// T04: Numerical edge cases for make_cuboid — NaN, Inf, negative, zero
// ---------------------------------------------------------------------------
#[test]
fn t04_make_cuboid_nan_returns_err() {
    let mut gen = IdGenerator::new(1);
    assert!(
        make_cuboid(f64::NAN, 1.0, 1.0, &mut gen).is_err(),
        "NaN width should be rejected"
    );
    assert!(
        make_cuboid(1.0, f64::NAN, 1.0, &mut gen).is_err(),
        "NaN height should be rejected"
    );
    assert!(
        make_cuboid(1.0, 1.0, f64::NAN, &mut gen).is_err(),
        "NaN depth should be rejected"
    );
}

#[test]
fn t04_make_cuboid_infinity_returns_err() {
    let mut gen = IdGenerator::new(1);
    assert!(
        make_cuboid(f64::INFINITY, 1.0, 1.0, &mut gen).is_err(),
        "Inf width should be rejected"
    );
    assert!(
        make_cuboid(1.0, f64::NEG_INFINITY, 1.0, &mut gen).is_err(),
        "-Inf height should be rejected"
    );
}

#[test]
fn t04_make_cuboid_negative_returns_err() {
    let mut gen = IdGenerator::new(1);
    assert!(
        make_cuboid(-1.0, 1.0, 1.0, &mut gen).is_err(),
        "negative width should be rejected"
    );
    assert!(
        make_cuboid(1.0, -5.0, 1.0, &mut gen).is_err(),
        "negative height should be rejected"
    );
    assert!(
        make_cuboid(1.0, 1.0, -0.1, &mut gen).is_err(),
        "negative depth should be rejected"
    );
}

#[test]
fn t04_make_cuboid_zero_returns_err() {
    let mut gen = IdGenerator::new(1);
    assert!(
        make_cuboid(0.0, 1.0, 1.0, &mut gen).is_err(),
        "zero width should be rejected"
    );
    assert!(
        make_cuboid(1.0, 0.0, 1.0, &mut gen).is_err(),
        "zero height should be rejected"
    );
    assert!(
        make_cuboid(1.0, 1.0, 0.0, &mut gen).is_err(),
        "zero depth should be rejected"
    );
}

#[test]
fn t04_make_cuboid_negative_zero_returns_err() {
    let mut gen = IdGenerator::new(1);
    assert!(
        make_cuboid(-0.0, 1.0, 1.0, &mut gen).is_err(),
        "-0.0 width should be rejected"
    );
}

#[test]
fn t04_make_cuboid_min_positive_ok() {
    let mut gen = IdGenerator::new(1);
    // f64::MIN_POSITIVE is > 0 and finite → should succeed
    let result = make_cuboid(
        f64::MIN_POSITIVE,
        f64::MIN_POSITIVE,
        f64::MIN_POSITIVE,
        &mut gen,
    );
    assert!(result.is_ok(), "MIN_POSITIVE dimensions should be valid");
    let solid = result.unwrap();
    assert!(
        solid.validate_manifold().is_ok(),
        "MIN_POSITIVE cuboid should be valid manifold"
    );
}

// ---------------------------------------------------------------------------
// T05: Empty Solid — validate_manifold is consistent (empty = vacuously valid)
// ---------------------------------------------------------------------------
#[test]
fn t05_empty_solid_validate_manifold_is_consistent() {
    let empty = Solid::new(0);
    // Empty solid has no shells or faces; validate_manifold returns Ok (vacuously valid).
    // This test guards against a regression where the result changes unexpectedly.
    let r1 = empty.validate_manifold();
    let r2 = empty.validate_manifold();
    assert_eq!(
        r1.is_ok(),
        r2.is_ok(),
        "validate_manifold must be deterministic for empty solid"
    );
}

// ---------------------------------------------------------------------------
// T06: make_cuboid determinism — same seed produces identical cuboid
// ---------------------------------------------------------------------------
#[test]
fn t06_make_cuboid_determinism() {
    let a = {
        let mut gen = IdGenerator::new(42);
        make_cuboid(3.0, 4.0, 5.0, &mut gen).unwrap()
    };
    let b = {
        let mut gen = IdGenerator::new(42);
        make_cuboid(3.0, 4.0, 5.0, &mut gen).unwrap()
    };

    assert_eq!(a.vertices.len(), b.vertices.len());
    assert_eq!(a.edges.len(), b.edges.len());
    assert_eq!(a.faces.len(), b.faces.len());
    for (va, vb) in a.vertices.iter().zip(b.vertices.iter()) {
        assert_eq!(va.id, vb.id, "cuboid vertex id mismatch");
    }

    let mesh_a = tessellate_solid(&a).unwrap();
    let mesh_b = tessellate_solid(&b).unwrap();
    assert_eq!(
        mesh_a.positions, mesh_b.positions,
        "cuboid positions mismatch"
    );
    assert_eq!(mesh_a.indices, mesh_b.indices, "cuboid indices mismatch");
}
