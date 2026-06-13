/// Acceptance tests for Issue #53 — tessellation vertex overdensity on hole faces.
///
/// Bug: boolean_cut_cylinder_hole top face generated ~2081 vertices (58× expected ~36)
/// due to double-discretization: 64 sub-arc edges × 32 samples/edge = 2048 points.
/// Fix: arc-proportional sampling in collect_loop_points.
use engawa_build::build_bodies_from_features;
use engawa_format::Document;
use engawa_kernel::brep::topology::IdGenerator;
use engawa_kernel::tessellation::tessellate_solid;
use engawa_kernel::LENGTH_TOLERANCE;

fn build_and_tessellate_hole() -> (Vec<[f64; 3]>, Vec<u32>) {
    let yaml = include_str!("../../../examples/boolean_cut_cylinder_hole.mycad");
    let doc: Document = serde_yaml::from_str(yaml).expect("YAML parse failed");
    let mut gen = IdGenerator::new(0);
    let bodies =
        build_bodies_from_features(&doc.root_component.features, &mut gen).expect("build failed");
    let live: Vec<_> = bodies.live().collect();
    assert!(!live.is_empty(), "no live bodies produced");
    let mesh = tessellate_solid(&live[0].solid).expect("tessellation failed");
    (mesh.positions, mesh.indices)
}

/// T01: Determinism — two runs produce identical positions and indices.
#[test]
fn t01_determinism() {
    let (pos1, idx1) = build_and_tessellate_hole();
    let (pos2, idx2) = build_and_tessellate_hole();
    assert_eq!(pos1, pos2, "positions differ between runs");
    assert_eq!(idx1, idx2, "indices differ between runs");
}

/// T02: Total vertex count is well under the buggy threshold (~2202).
#[test]
fn t02_total_vertex_count_under_threshold() {
    let (positions, _) = build_and_tessellate_hole();
    assert!(
        positions.len() < 400,
        "total vertices = {}, expected < 400",
        positions.len()
    );
}

/// T03: Top face (z ≈ 5.0) vertex count ≤ 200.
/// Donut face outer(4) + inner rim(64) + cylinder wall top rim(~65, matches 64 arc segments
/// from boolean result per #129 Fix C) + margin. Was ≤ 130 before #129 fix; increased because
/// n_u now derives from arc count (64) instead of default angular_segments (32) for boolean
/// cylinder faces, ensuring watertight boundary alignment.
#[test]
fn t03_top_face_vertex_count_under_threshold() {
    let (positions, _) = build_and_tessellate_hole();
    let top_count = positions
        .iter()
        .filter(|p| (p[2] - 5.0).abs() <= LENGTH_TOLERANCE)
        .count();
    assert!(
        top_count <= 200,
        "top face vertex count = {}, expected ≤ 200",
        top_count
    );
}

/// T04: No zero-area triangles on top face.
#[test]
fn t04_no_zero_area_triangles_on_top_face() {
    let (positions, indices) = build_and_tessellate_hole();
    let area_eps = LENGTH_TOLERANCE * LENGTH_TOLERANCE;

    let top_verts: std::collections::HashSet<usize> = positions
        .iter()
        .enumerate()
        .filter(|(_, p)| (p[2] - 5.0).abs() <= LENGTH_TOLERANCE)
        .map(|(i, _)| i)
        .collect();

    for tri in indices.chunks(3) {
        let (i0, i1, i2) = (tri[0] as usize, tri[1] as usize, tri[2] as usize);
        if !top_verts.contains(&i0) || !top_verts.contains(&i1) || !top_verts.contains(&i2) {
            continue;
        }
        let p0 = positions[i0];
        let p1 = positions[i1];
        let p2 = positions[i2];
        let u = [p1[0] - p0[0], p1[1] - p0[1], p1[2] - p0[2]];
        let v = [p2[0] - p0[0], p2[1] - p0[1], p2[2] - p0[2]];
        let cross_norm_sq = (u[1] * v[2] - u[2] * v[1]).powi(2)
            + (u[2] * v[0] - u[0] * v[2]).powi(2)
            + (u[0] * v[1] - u[1] * v[0]).powi(2);
        assert!(
            cross_norm_sq > area_eps * area_eps,
            "zero-area triangle on top face: verts [{}, {}, {}]",
            i0,
            i1,
            i2
        );
    }
}

/// T01b: 100-run determinism — all runs produce identical positions and indices.
#[test]
fn t01b_100_run_determinism() {
    let (pos1, idx1) = build_and_tessellate_hole();
    for i in 1..100 {
        let (pos, idx) = build_and_tessellate_hole();
        assert_eq!(pos1, pos, "positions differ on run {i}");
        assert_eq!(idx1, idx, "indices differ on run {i}");
    }
}

/// T06: All mesh coordinates are finite (no NaN/Inf).
#[test]
fn t06_all_coords_finite() {
    let (positions, _) = build_and_tessellate_hole();
    for (i, p) in positions.iter().enumerate() {
        for (j, &x) in p.iter().enumerate() {
            assert!(x.is_finite(), "position[{i}][{j}] = {x} is not finite");
        }
    }
}

/// T07: YAML roundtrip — build solid → serialize → deserialize → tessellate → identical.
#[test]
fn t07_yaml_roundtrip() {
    let yaml = include_str!("../../../examples/boolean_cut_cylinder_hole.mycad");
    let doc: Document = serde_yaml::from_str(yaml).expect("YAML parse failed");
    let mut gen = IdGenerator::new(0);
    let bodies =
        build_bodies_from_features(&doc.root_component.features, &mut gen).expect("build failed");
    let live: Vec<_> = bodies.live().collect();
    let solid1 = &live[0].solid;

    // Serialize → deserialize
    let solid_yaml = serde_yaml::to_string(solid1).expect("Solid serialize failed");
    let solid2: engawa_kernel::brep::topology::Solid =
        serde_yaml::from_str(&solid_yaml).expect("Solid deserialize failed");

    // Tessellate both and compare
    let mesh1 = tessellate_solid(solid1).expect("tessellation 1 failed");
    let mesh2 = tessellate_solid(&solid2).expect("tessellation 2 failed");

    assert_eq!(
        mesh1.positions, mesh2.positions,
        "positions differ after YAML roundtrip"
    );
    assert_eq!(
        mesh1.indices, mesh2.indices,
        "indices differ after YAML roundtrip"
    );
}
