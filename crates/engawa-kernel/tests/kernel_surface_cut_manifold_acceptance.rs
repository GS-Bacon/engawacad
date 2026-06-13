//! Acceptance tests for Issue #120: surface cut manifold fix.
//!
//! Tests: T01 (determinism), T02_volume_reduced, T02_boundary_large_offset,
//!        T02_degen_flush.

use engawa_kernel::booleans::{boolean, BooleanOp};
use engawa_kernel::brep::topology::IdGenerator;
use engawa_kernel::geometry::Vec3;
use engawa_kernel::primitives::make_cuboid;
use engawa_kernel::tessellation::tessellate_solid;

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

/// T01_determinism: surface cut with same seed produces identical topology.
#[test]
fn t01_determinism_surface_cut() {
    let build = || {
        let mut gen = IdGenerator::new(42);
        let target = make_cuboid(10.0, 20.0, 30.0, &mut gen).unwrap();
        let mut tool = make_cuboid(2.0, 2.0, 2.0, &mut gen).unwrap();
        tool.translate(Vec3::new(4.5, 0.0, 0.0));
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

/// T02_volume_reduced: surface cut reduces volume.
#[test]
fn t02_volume_reduced_after_surface_cut() {
    let mut gen = IdGenerator::new(42);
    let target = make_cuboid(10.0, 20.0, 30.0, &mut gen).unwrap();
    let mut tool = make_cuboid(2.0, 2.0, 2.0, &mut gen).unwrap();
    tool.translate(Vec3::new(4.5, 0.0, 0.0));

    let result = boolean(&target, &tool, BooleanOp::Cut, &mut gen);
    assert!(result.is_ok(), "surface cut must succeed: {result:?}");
    let cut = result.unwrap();

    assert!(
        cut.validate_manifold().is_ok(),
        "surface cut manifold invalid: {:?}",
        cut.validate_manifold()
    );

    let mesh_t = tessellate_solid(&target).unwrap();
    let mesh_c = tessellate_solid(&cut).unwrap();
    let vol_t = mesh_signed_volume(&mesh_t).abs();
    let vol_c = mesh_signed_volume(&mesh_c).abs();

    assert!(
        vol_c < vol_t,
        "surface cut must reduce volume: before={vol_t}, after={vol_c}"
    );
}

/// T02_boundary_large_offset: x_offset=5.3 (tool largely protrudes) still valid manifold.
#[test]
fn t02_boundary_large_offset_manifold() {
    let mut gen = IdGenerator::new(42);
    let target = make_cuboid(10.0, 20.0, 30.0, &mut gen).unwrap();
    let mut tool = make_cuboid(2.0, 2.0, 2.0, &mut gen).unwrap();
    tool.translate(Vec3::new(5.3, 0.0, 0.0));

    let result = boolean(&target, &tool, BooleanOp::Cut, &mut gen);
    match result {
        Ok(solid) => assert!(
            solid.validate_manifold().is_ok(),
            "large offset manifold invalid: {:?}",
            solid.validate_manifold()
        ),
        Err(_) => {} // clean rejection is acceptable
    }
}

/// T02_degen_flush: x_offset=4.0 (tool right face flush with target face) — no panic.
#[test]
fn t02_degen_flush_no_panic() {
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let mut gen = IdGenerator::new(42);
        let target = make_cuboid(10.0, 20.0, 30.0, &mut gen).unwrap();
        let mut tool = make_cuboid(2.0, 2.0, 2.0, &mut gen).unwrap();
        tool.translate(Vec3::new(4.0, 0.0, 0.0));
        boolean(&target, &tool, BooleanOp::Cut, &mut gen)
    }));
    assert!(result.is_ok(), "flush offset must not panic");
    // Ok or Err are both acceptable outcomes
}

/// E01: proptest が縮小した失敗値 x_offset=4.97809298669049 の固定値回帰テスト。
/// proptest は min-case を永続化しないため、固定値テストで防衛する。
#[test]
fn t02_regression_fixed_x_offset() {
    let mut gen = IdGenerator::new(42);
    let target = make_cuboid(10.0, 20.0, 30.0, &mut gen).unwrap();
    let mut tool = make_cuboid(2.0, 2.0, 2.0, &mut gen).unwrap();
    tool.translate(Vec3::new(4.97809298669049, 0.0, 0.0));
    let result = boolean(&target, &tool, BooleanOp::Cut, &mut gen);
    assert!(
        result.is_ok(),
        "fixed-offset surface cut must succeed: {result:?}"
    );
    let cut = result.unwrap();
    assert!(
        cut.validate_manifold().is_ok(),
        "fixed-offset surface cut manifold invalid: {:?}",
        cut.validate_manifold()
    );
}

/// T03: seed=100 でも決定的であることを確認（seed=42 以外の別 seed）。
#[test]
fn t03_determinism_fixed_seed() {
    let build = |seed: u64| {
        let mut gen = IdGenerator::new(seed);
        let target = make_cuboid(10.0, 20.0, 30.0, &mut gen).unwrap();
        let mut tool = make_cuboid(2.0, 2.0, 2.0, &mut gen).unwrap();
        tool.translate(Vec3::new(4.97809298669049, 0.0, 0.0));
        boolean(&target, &tool, BooleanOp::Cut, &mut gen).unwrap()
    };
    let a = build(100);
    let b = build(100);
    assert_eq!(a.vertices.len(), b.vertices.len(), "vertex count mismatch");
    assert_eq!(a.edges.len(), b.edges.len(), "edge count mismatch");
    assert_eq!(a.faces.len(), b.faces.len(), "face count mismatch");
    for (va, vb) in a.vertices.iter().zip(b.vertices.iter()) {
        assert_eq!(va.id, vb.id, "vertex id mismatch");
    }
}

/// 繰り返し決定性: 同一入力（surface cut）を100回実行して全結果が一致するか。
#[test]
fn t03_surface_cut_100_run_determinism() {
    let mut results: Vec<(usize, usize, usize, Vec<u64>)> = Vec::with_capacity(100);
    for _ in 0..100 {
        let mut gen = IdGenerator::new(42);
        let target = make_cuboid(10.0, 20.0, 30.0, &mut gen).unwrap();
        let mut tool = make_cuboid(2.0, 2.0, 2.0, &mut gen).unwrap();
        tool.translate(Vec3::new(4.5, 0.0, 0.0));
        let solid = boolean(&target, &tool, BooleanOp::Cut, &mut gen).unwrap();
        let ids: Vec<u64> = solid.vertices.iter().map(|v| v.id).collect();
        results.push((
            solid.vertices.len(),
            solid.edges.len(),
            solid.faces.len(),
            ids,
        ));
    }
    let first = &results[0];
    for (i, r) in results.iter().enumerate() {
        assert_eq!(r.0, first.0, "vertex count mismatch at run {i}");
        assert_eq!(r.1, first.1, "edge count mismatch at run {i}");
        assert_eq!(r.2, first.2, "face count mismatch at run {i}");
        assert_eq!(r.3, first.3, "vertex ids mismatch at run {i}");
    }
}
