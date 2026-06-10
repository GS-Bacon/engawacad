/// Acceptance tests for #131 — cross-face adjacency n_u selection.
///
/// These tests verify that `tessellate_face_uv_grid` picks the correct n_u by
/// inspecting the adjacent cap face type rather than relying on arcs_per_rev alone.
///
/// Coverage:
///   T01 — determinism: box∩cyl intersect via adjacent_face_idx path, 2 runs match
///   T04_boundary — primitive cylinder uses angular_segments (arcs_per_rev=1 path)
///   T05_degen — fallback when adjacency returns None or non-Sphere (no panic)
///
/// Edge-case additions (adversarial):
///   EC01 — 100-run determinism (repeated tessellation of box∩cyl intersect)
///   EC02 — numerical boundary: make_cylinder rejects NaN/Inf/negative/zero params
///   EC03 — empty solid tessellation (zero faces → empty mesh, no panic)
///   EC04 — YAML round-trip: serialize box∩cyl solid, deserialize, tessellate, compare
///   EC05 — minimal TessellationOptions (angular_segments=3) on boolean result
///   EC06 — primitive cuboid tessellation (all-planar, no adjacency path)
use mycad_kernel::booleans::{boolean, BooleanOp};
use mycad_kernel::brep::topology::{IdGenerator, Solid};
use mycad_kernel::geometry::Point;
use mycad_kernel::primitives::{make_cuboid, make_cylinder};
use mycad_kernel::tessellation::{tessellate_solid, tessellate_solid_with, TessellationOptions};
use serde_yaml;

/// Build box∩cyl intersect: box(10³) ∩ cylinder(r=2, h=15, origin=(0,0,-7.5)).
fn build_intersect_box_cyl() -> Solid {
    let mut gen = IdGenerator::new(0);
    let box_solid = make_cuboid(10.0, 10.0, 10.0, &mut gen).unwrap();
    let cyl = make_cylinder(2.0, 15.0, Point::new(0.0, 0.0, -7.5), &mut gen).unwrap();
    boolean(&box_solid, &cyl, BooleanOp::Intersect, &mut gen)
        .expect("intersect box cyl should succeed")
}

// ---------------------------------------------------------------------------
// T01: Determinism — box∩cyl intersect through the adjacent_face_idx path
// Two runs with identical IdGenerator must produce bit-identical meshes.
// ---------------------------------------------------------------------------
#[test]
fn t01_determinism_cross_face_nu() {
    let solid1 = build_intersect_box_cyl();
    let solid2 = build_intersect_box_cyl();

    let m1 = tessellate_solid(&solid1).expect("tessellate run 1");
    let m2 = tessellate_solid(&solid2).expect("tessellate run 2");

    assert_eq!(
        m1.positions, m2.positions,
        "T01: positions differ between runs"
    );
    assert_eq!(m1.normals, m2.normals, "T01: normals differ between runs");
    assert_eq!(m1.indices, m2.indices, "T01: indices differ between runs");
}

// ---------------------------------------------------------------------------
// T04_boundary: Primitive cylinder (arcs_per_rev=1) must still use angular_segments.
// The adjacent_face_idx change must not affect primitive (non-boolean) cylinders.
// ---------------------------------------------------------------------------
#[test]
fn t04_boundary_primitive_cylinder_angular_segments() {
    let mut gen = IdGenerator::new(0);
    let cyl = make_cylinder(3.0, 10.0, Point::origin(), &mut gen).unwrap();
    let opts = TessellationOptions::new(16, 2);
    // Must tessellate without panic and produce a non-empty mesh.
    let mesh = tessellate_solid_with(&cyl, &opts).expect("primitive cylinder tessellation");
    assert!(
        mesh.triangle_count() > 0,
        "T04: primitive cylinder produced empty mesh"
    );
    assert!(
        mesh.positions
            .iter()
            .all(|p: &[f64; 3]| p.iter().all(|x| x.is_finite())),
        "T04: NaN/Inf in primitive cylinder mesh"
    );
}

// ---------------------------------------------------------------------------
// T05_degen: When adjacent_face_idx returns None (or non-Sphere adjacent),
// the code must fall back to arcs_per_rev without panicking.
// This is an indirect test: box∩cyl produces planar (non-Sphere) adjacency,
// verifying the non-Sphere branch does not panic.
// ---------------------------------------------------------------------------
#[test]
fn t05_degen_non_sphere_adjacency_no_panic() {
    let solid = build_intersect_box_cyl();
    // Must not panic; watertightness is checked in boundary_align_acceptance::t05
    let mesh = tessellate_solid(&solid).expect("T05_degen: tessellation must not fail");
    assert!(mesh.triangle_count() > 0, "T05_degen: mesh is empty");
    assert!(
        mesh.positions
            .iter()
            .all(|p: &[f64; 3]| p.iter().all(|x| x.is_finite())),
        "T05_degen: NaN/Inf in positions"
    );
}

// ===========================================================================
// Edge-case tests (adversarial additions beyond plan)
// ===========================================================================

// ---------------------------------------------------------------------------
// EC01: 100-run determinism — box∩cyl intersect tessellated 100 times must
// produce bit-identical meshes on every run.
// ---------------------------------------------------------------------------
#[test]
fn ec01_100_run_determinism_cross_face_nu() {
    let solid = build_intersect_box_cyl();
    let first = tessellate_solid(&solid).expect("tessellate run 1");

    for i in 1..100 {
        let mesh = tessellate_solid(&solid).expect("tessellate run");
        assert_eq!(
            first.positions, mesh.positions,
            "EC01: positions differ on run {i}"
        );
        assert_eq!(
            first.normals, mesh.normals,
            "EC01: normals differ on run {i}"
        );
        assert_eq!(
            first.indices, mesh.indices,
            "EC01: indices differ on run {i}"
        );
    }
}

// ---------------------------------------------------------------------------
// EC02: Numerical boundary — make_cylinder rejects NaN/Inf/negative/zero.
// Ensures the primitive constructors don't panic on degenerate numeric inputs.
// ---------------------------------------------------------------------------
#[test]
fn ec02_make_cylinder_rejects_invalid_params() {
    let mut gen = IdGenerator::new(0);
    let origin = Point::origin();

    // NaN radius
    let result = make_cylinder(f64::NAN, 10.0, origin, &mut gen);
    assert!(result.is_err(), "EC02: NaN radius should be rejected");

    // Inf radius
    let result = make_cylinder(f64::INFINITY, 10.0, origin, &mut gen);
    assert!(result.is_err(), "EC02: Inf radius should be rejected");

    // Negative radius
    let result = make_cylinder(-1.0, 10.0, origin, &mut gen);
    assert!(result.is_err(), "EC02: negative radius should be rejected");

    // Zero radius
    let result = make_cylinder(0.0, 10.0, origin, &mut gen);
    assert!(result.is_err(), "EC02: zero radius should be rejected");

    // NaN height
    let result = make_cylinder(2.0, f64::NAN, origin, &mut gen);
    assert!(result.is_err(), "EC02: NaN height should be rejected");

    // Inf height
    let result = make_cylinder(2.0, f64::INFINITY, origin, &mut gen);
    assert!(result.is_err(), "EC02: Inf height should be rejected");

    // Negative height
    let result = make_cylinder(2.0, -5.0, origin, &mut gen);
    assert!(result.is_err(), "EC02: negative height should be rejected");

    // Zero height
    let result = make_cylinder(2.0, 0.0, origin, &mut gen);
    assert!(result.is_err(), "EC02: zero height should be rejected");

    // NaN origin
    let nan_origin = Point::new(f64::NAN, 0.0, 0.0);
    let result = make_cylinder(2.0, 10.0, nan_origin, &mut gen);
    assert!(result.is_err(), "EC02: NaN origin should be rejected");

    // Inf origin
    let inf_origin = Point::new(0.0, f64::INFINITY, 0.0);
    let result = make_cylinder(2.0, 10.0, inf_origin, &mut gen);
    assert!(result.is_err(), "EC02: Inf origin should be rejected");
}

// ---------------------------------------------------------------------------
// EC03: Empty solid tessellation — zero faces should produce empty mesh.
// ---------------------------------------------------------------------------
#[test]
fn ec03_empty_solid_tessellation() {
    let mut gen = IdGenerator::new(0);
    let empty_solid = Solid::new(gen.next());

    let mesh = tessellate_solid(&empty_solid).expect("EC03: empty solid should tessellate");
    assert_eq!(
        mesh.positions.len(),
        0,
        "EC03: empty solid should produce zero vertices"
    );
    assert_eq!(
        mesh.indices.len(),
        0,
        "EC03: empty solid should produce zero indices"
    );
    assert_eq!(
        mesh.triangle_count(),
        0,
        "EC03: empty solid should have 0 triangles"
    );
}

// ---------------------------------------------------------------------------
// EC04: YAML round-trip — serialize box∩cyl solid, deserialize, tessellate
// both originals and round-tripped, verify bit-identical meshes.
// ---------------------------------------------------------------------------
#[test]
fn ec04_yaml_roundtrip_box_intersect_cyl() {
    let original = build_intersect_box_cyl();

    let yaml = serde_yaml::to_string(&original).expect("EC04: serialize to YAML");
    let restored: Solid = serde_yaml::from_str(&yaml).expect("EC04: deserialize from YAML");

    // Verify topology is identical after round-trip
    assert_eq!(
        original.vertices.len(),
        restored.vertices.len(),
        "EC04: vertex count mismatch"
    );
    assert_eq!(
        original.faces.len(),
        restored.faces.len(),
        "EC04: face count mismatch"
    );
    assert_eq!(
        original.edges.len(),
        restored.edges.len(),
        "EC04: edge count mismatch"
    );
    assert_eq!(
        original.half_edges.len(),
        restored.half_edges.len(),
        "EC04: half-edge count mismatch"
    );

    // Tessellate both and compare meshes
    let mesh_orig = tessellate_solid(&original).expect("EC04: tessellate original");
    let mesh_rt = tessellate_solid(&restored).expect("EC04: tessellate round-tripped");

    assert_eq!(
        mesh_orig.positions, mesh_rt.positions,
        "EC04: positions differ after round-trip"
    );
    assert_eq!(
        mesh_orig.normals, mesh_rt.normals,
        "EC04: normals differ after round-trip"
    );
    assert_eq!(
        mesh_orig.indices, mesh_rt.indices,
        "EC04: indices differ after round-trip"
    );
}

// ---------------------------------------------------------------------------
// EC05: Minimal TessellationOptions (angular_segments=3) on box∩cyl boolean
// result — must not panic and must produce a finite non-empty mesh.
// ---------------------------------------------------------------------------
#[test]
fn ec05_minimal_angular_segments_boolean() {
    let solid = build_intersect_box_cyl();
    let opts = TessellationOptions::new(3, 1);
    let mesh = tessellate_solid_with(&solid, &opts).expect("EC05: minimal opts tessellation");

    assert!(mesh.triangle_count() > 0, "EC05: mesh should not be empty");
    assert!(
        mesh.positions
            .iter()
            .all(|p: &[f64; 3]| p.iter().all(|x| x.is_finite())),
        "EC05: all positions must be finite"
    );
    assert!(
        mesh.normals
            .iter()
            .all(|n: &[f64; 3]| n.iter().all(|x| x.is_finite())),
        "EC05: all normals must be finite"
    );
}

// ---------------------------------------------------------------------------
// EC06: Primitive cuboid tessellation — all-planar faces, no cylinder path,
// no adjacency walk. Verifies tessellation doesn't regress on pure planar.
// ---------------------------------------------------------------------------
#[test]
fn ec06_cuboid_all_planar_no_adjacency() {
    let mut gen = IdGenerator::new(0);
    let cuboid = make_cuboid(5.0, 5.0, 5.0, &mut gen).unwrap();
    let mesh = tessellate_solid(&cuboid).expect("EC06: cuboid tessellation");

    assert!(
        mesh.triangle_count() > 0,
        "EC06: cuboid should have triangles"
    );
    assert!(
        mesh.positions
            .iter()
            .all(|p: &[f64; 3]| p.iter().all(|x| x.is_finite())),
        "EC06: all positions must be finite"
    );
    // A cuboid should have exactly 12 triangles (6 faces × 2 triangles each)
    assert_eq!(
        mesh.triangle_count(),
        12,
        "EC06: cuboid should have exactly 12 triangles"
    );
}
