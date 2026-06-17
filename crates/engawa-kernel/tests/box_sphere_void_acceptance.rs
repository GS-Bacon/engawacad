/// Acceptance tests for Issue #50:
/// box(10³) − sphere(r=3, origin) Boolean Cut produces an enclosed sphere void,
/// which must tessellate successfully (regression: was TrimmedFaceUnsupported).
use engawa_kernel::booleans::{boolean, BooleanOp};
use engawa_kernel::brep::topology::IdGenerator;
use engawa_kernel::geometry::curve::Curve;
use engawa_kernel::geometry::surface::Surface;
use engawa_kernel::geometry::Point;
use engawa_kernel::primitives::{make_cuboid, make_sphere};
use engawa_kernel::tessellation::tessellate_solid;
use std::collections::HashMap;
use std::f64::consts::PI;

fn make_box_minus_sphere(gen: &mut IdGenerator) -> engawa_kernel::brep::topology::Solid {
    let box_solid = make_cuboid(10.0, 10.0, 10.0, "cuboid", gen).expect("cuboid");
    let sphere = make_sphere(3.0, Point::origin(), gen).expect("sphere");
    boolean(&box_solid, &sphere, BooleanOp::Cut, gen).expect("box - sphere cut")
}

/// Signed mesh volume via divergence theorem.
fn raw_mesh_signed_volume(mesh: &engawa_kernel::tessellation::TriangleMesh) -> f64 {
    let mut vol = 0.0_f64;
    for tri in 0..mesh.triangle_count() {
        let i0 = mesh.indices[tri * 3] as usize;
        let i1 = mesh.indices[tri * 3 + 1] as usize;
        let i2 = mesh.indices[tri * 3 + 2] as usize;
        let p0 = &mesh.positions[i0];
        let p1 = &mesh.positions[i1];
        let p2 = &mesh.positions[i2];
        vol += (p0[0] * (p1[1] * p2[2] - p2[1] * p1[2])
            + p1[0] * (p2[1] * p0[2] - p0[1] * p2[2])
            + p2[0] * (p0[1] * p1[2] - p1[1] * p0[2]))
            / 6.0;
    }
    vol
}

#[test]
fn t01_determinism() {
    let mut gen1 = IdGenerator::new(0);
    let solid1 = make_box_minus_sphere(&mut gen1);
    let mesh1 = tessellate_solid(&solid1).expect("tessellate 1");

    let mut gen2 = IdGenerator::new(0);
    let solid2 = make_box_minus_sphere(&mut gen2);
    let mesh2 = tessellate_solid(&solid2).expect("tessellate 2");

    assert_eq!(mesh1.positions, mesh2.positions, "positions must match");
    assert_eq!(mesh1.normals, mesh2.normals, "normals must match");
    assert_eq!(mesh1.indices, mesh2.indices, "indices must match");

    // Verify seam edge Curve::Circle determinism
    let sphere_face1 = solid1
        .faces
        .iter()
        .find(|f| matches!(f.surface, Surface::Sphere { .. }))
        .expect("sphere face 1");
    let sphere_face2 = solid2
        .faces
        .iter()
        .find(|f| matches!(f.surface, Surface::Sphere { .. }))
        .expect("sphere face 2");

    let get_seam_circle = |solid: &engawa_kernel::brep::topology::Solid,
                           face: &engawa_kernel::brep::topology::Face|
     -> (
        engawa_kernel::geometry::Point,
        engawa_kernel::geometry::Vec3,
        f64,
    ) {
        let ol = &solid.loops[face.outer_loop];
        let he = &solid.half_edges[ol.half_edges[0]];
        let edge = &solid.edges[he.edge];
        match &edge.curve {
            Curve::Circle {
                center,
                normal,
                radius,
            } => (*center, *normal, *radius),
            other => panic!("expected Curve::Circle, got {:?}", other),
        }
    };

    let (c1, n1, r1) = get_seam_circle(&solid1, sphere_face1);
    let (c2, n2, r2) = get_seam_circle(&solid2, sphere_face2);
    assert_eq!(c1, c2, "seam circle center must match");
    assert_eq!(n1, n2, "seam circle normal must match");
    assert!((r1 - r2).abs() < 1e-15, "seam circle radius must match");
}

#[test]
fn t02_regression_tessellate_ok() {
    let mut gen = IdGenerator::new(0);
    let solid = make_box_minus_sphere(&mut gen);
    let result = tessellate_solid(&solid);
    assert!(result.is_ok(), "expected Ok, got {:?}", result.err());
}

#[test]
fn t03_watertight() {
    let mut gen = IdGenerator::new(0);
    let solid = make_box_minus_sphere(&mut gen);
    let mesh = tessellate_solid(&solid).expect("tessellate");

    // Position-based watertight check: quantize vertex positions and verify
    // every directed edge has its geometric reverse present.
    // This handles merged meshes where face boundaries don't share vertex indices.
    let tol = 1e-10;
    let quantize = |p: &[f64; 3]| -> [i64; 3] {
        [
            (p[0] / tol).round() as i64,
            (p[1] / tol).round() as i64,
            (p[2] / tol).round() as i64,
        ]
    };

    let mut edge_count: HashMap<([i64; 3], [i64; 3]), u32> = HashMap::new();
    for tri in 0..mesh.triangle_count() {
        let base = tri * 3;
        let i0 = mesh.indices[base] as usize;
        let i1 = mesh.indices[base + 1] as usize;
        let i2 = mesh.indices[base + 2] as usize;
        let p0 = quantize(&mesh.positions[i0]);
        let p1 = quantize(&mesh.positions[i1]);
        let p2 = quantize(&mesh.positions[i2]);
        // Skip degenerate edges
        if p0 != p1 {
            *edge_count.entry((p0, p1)).or_insert(0) += 1;
        }
        if p1 != p2 {
            *edge_count.entry((p1, p2)).or_insert(0) += 1;
        }
        if p2 != p0 {
            *edge_count.entry((p2, p0)).or_insert(0) += 1;
        }
    }

    for ((a, b), count) in &edge_count {
        assert_eq!(
            *count, 1,
            "directed edge {:?}->{:?} used {} times (expected 1)",
            a, b, count
        );
        let rev_count = edge_count.get(&(*b, *a)).copied().unwrap_or(0);
        assert_eq!(rev_count, 1, "reverse edge missing for {:?}->{:?}", a, b);
    }
}

#[test]
fn t04_void_normals_inward() {
    let mut gen = IdGenerator::new(0);
    let solid = make_box_minus_sphere(&mut gen);
    let mesh = tessellate_solid(&solid).expect("tessellate");

    // Sphere center is at origin. For void (cavity) faces, normals point inward
    // toward the center, so normal · (center − centroid) > 0 for each triangle.
    // We identify sphere-face triangles by checking if all 3 vertices are
    // within the sphere radius + tolerance.
    let sphere_r = 3.0;
    let tolerance = 0.5;

    let mut sphere_tri_count = 0usize;
    for tri in 0..mesh.triangle_count() {
        let base = tri * 3;
        let i0 = mesh.indices[base] as usize;
        let i1 = mesh.indices[base + 1] as usize;
        let i2 = mesh.indices[base + 2] as usize;

        let p0 = &mesh.positions[i0];
        let p1 = &mesh.positions[i1];
        let p2 = &mesh.positions[i2];

        let d0 = (p0[0].powi(2) + p0[1].powi(2) + p0[2].powi(2)).sqrt();
        let d1 = (p1[0].powi(2) + p1[1].powi(2) + p1[2].powi(2)).sqrt();
        let d2 = (p2[0].powi(2) + p2[1].powi(2) + p2[2].powi(2)).sqrt();

        if d0 < sphere_r + tolerance && d1 < sphere_r + tolerance && d2 < sphere_r + tolerance {
            sphere_tri_count += 1;
            // centroid
            let cx = (p0[0] + p1[0] + p2[0]) / 3.0;
            let cy = (p0[1] + p1[1] + p2[1]) / 3.0;
            let cz = (p0[2] + p1[2] + p2[2]) / 3.0;

            // Use the face normal (average of vertex normals for this triangle)
            let n0 = &mesh.normals[i0];
            let n1 = &mesh.normals[i1];
            let n2 = &mesh.normals[i2];
            let nx = (n0[0] + n1[0] + n2[0]) / 3.0;
            let ny = (n0[1] + n1[1] + n2[1]) / 3.0;
            let nz = (n0[2] + n1[2] + n2[2]) / 3.0;

            // center (origin) - centroid
            let to_center_x = -cx;
            let to_center_y = -cy;
            let to_center_z = -cz;

            let dot = nx * to_center_x + ny * to_center_y + nz * to_center_z;
            assert!(
                dot > 0.0,
                "void sphere triangle #{}: normal should point inward (toward center), dot={}",
                tri,
                dot
            );
        }
    }
    assert!(sphere_tri_count > 0, "expected some sphere triangles");
}

#[test]
fn t05_signed_volume() {
    let mut gen = IdGenerator::new(0);
    let solid = make_box_minus_sphere(&mut gen);
    let mesh = tessellate_solid(&solid).expect("tessellate");
    let vol = raw_mesh_signed_volume(&mesh);

    let expected = 1000.0 - (4.0 / 3.0) * PI * 3.0_f64.powi(3);
    // Faceted void carves slightly less than true sphere, so vol > expected
    assert!(
        vol > expected,
        "volume {} should exceed expected {}",
        vol,
        expected
    );
    let diff = (vol - expected).abs();
    assert!(
        diff < 8.0,
        "volume diff {} too large (expected < 8.0)",
        diff
    );
}

#[test]
fn t06_topology() {
    let mut gen = IdGenerator::new(0);
    let solid = make_box_minus_sphere(&mut gen);

    assert_eq!(solid.faces.len(), 7, "6 box faces + 1 sphere void face");

    // Find the sphere void face
    let sphere_face = solid
        .faces
        .iter()
        .find(|f| matches!(f.surface, Surface::Sphere { .. }))
        .expect("sphere void face not found");

    assert!(
        !sphere_face.same_sense,
        "void sphere face must have same_sense = false"
    );

    // Seam edge must be Curve::Circle with normal ≈ -Y and span ≈ π
    let outer_loop = &solid.loops[sphere_face.outer_loop];
    let he0 = &solid.half_edges[outer_loop.half_edges[0]];
    let edge = &solid.edges[he0.edge];
    match &edge.curve {
        Curve::Circle { normal, .. } => {
            let neg_y = engawa_kernel::geometry::Vec3::new(0.0, -1.0, 0.0);
            assert!(
                (*normal - neg_y).norm() < 1e-9,
                "seam normal should be -Y, got {:?}",
                normal
            );
            let span = (edge.t_range[1] - edge.t_range[0]).abs();
            assert!(
                (span - PI).abs() < 1e-9,
                "seam t_range span should be π, got {span}"
            );
        }
        other => panic!("expected Curve::Circle, got {:?}", other),
    }
}

// ---------------------------------------------------------------------------
// Edge-case / adversarial tests (EC-02 .. EC-06)
// ---------------------------------------------------------------------------

/// EC02: Void sphere shell satisfies Euler-Poincaré V-E+F = 2.
/// The cut result has 2 closed shells (box outer + sphere void).
#[test]
fn ec02_euler_poincare_void_shell() {
    let mut gen = IdGenerator::new(0);
    let solid = make_box_minus_sphere(&mut gen);

    // Expect exactly 2 shells, both closed
    assert_eq!(
        solid.shells.len(),
        2,
        "expected 2 shells (box + sphere void)"
    );
    for (i, shell) in solid.shells.iter().enumerate() {
        assert!(shell.closed, "shell {i} must be closed");
    }

    let sphere_face_idx = solid
        .faces
        .iter()
        .position(|f| matches!(f.surface, Surface::Sphere { .. }))
        .expect("sphere face");

    let sphere_shell = solid
        .shells
        .iter()
        .find(|s| s.faces.contains(&sphere_face_idx))
        .expect("sphere shell");

    // Collect unique vertices and edges across all faces in the sphere shell
    let mut vertex_indices: Vec<usize> = Vec::new();
    let mut edge_indices: Vec<usize> = Vec::new();
    let mut face_count = 0usize;

    for &fi in &sphere_shell.faces {
        face_count += 1;
        let face = &solid.faces[fi];
        let ol = &solid.loops[face.outer_loop];
        for &he_idx in &ol.half_edges {
            let he = &solid.half_edges[he_idx];
            if !vertex_indices.contains(&he.start_vertex) {
                vertex_indices.push(he.start_vertex);
            }
            if !edge_indices.contains(&he.edge) {
                edge_indices.push(he.edge);
            }
        }
    }

    let v = vertex_indices.len() as i64;
    let e = edge_indices.len() as i64;
    let f = face_count as i64;
    assert_eq!(
        v - e + f,
        2,
        "Euler-Poincaré V-E+F must equal 2 (got V={v}, E={e}, F={f})"
    );
}

/// EC03: 100-run determinism — same IdGenerator seed always produces the
/// identical tessellated mesh (positions, normals, indices).
#[test]
fn ec03_100x_determinism() {
    let mut gen0 = IdGenerator::new(0);
    let solid0 = make_box_minus_sphere(&mut gen0);
    let mesh0 = tessellate_solid(&solid0).expect("tessellate baseline");

    for i in 1..=99 {
        let mut gen = IdGenerator::new(0);
        let solid = make_box_minus_sphere(&mut gen);
        let mesh = tessellate_solid(&solid).unwrap_or_else(|e| panic!("run {i}: {e}"));

        assert_eq!(
            mesh0.positions, mesh.positions,
            "run {i}: positions mismatch"
        );
        assert_eq!(mesh0.normals, mesh.normals, "run {i}: normals mismatch");
        assert_eq!(mesh0.indices, mesh.indices, "run {i}: indices mismatch");
    }
}

/// EC04: make_sphere rejects special float values (NaN, Inf, -0.0) without
/// panicking, and accepts f64::MIN_POSITIVE.
#[test]
fn ec04_make_sphere_numerical_boundaries() {
    // NaN radius → error
    assert!(
        make_sphere(f64::NAN, Point::origin(), &mut IdGenerator::new(0)).is_err(),
        "NaN radius should error"
    );
    // +Inf radius → error
    assert!(
        make_sphere(f64::INFINITY, Point::origin(), &mut IdGenerator::new(0)).is_err(),
        "+Inf radius should error"
    );
    // -Inf radius → error
    assert!(
        make_sphere(f64::NEG_INFINITY, Point::origin(), &mut IdGenerator::new(0)).is_err(),
        "-Inf radius should error"
    );
    // -0.0 radius → error (IEEE 754: -0.0 ≤ 0.0)
    assert!(
        make_sphere(-0.0, Point::origin(), &mut IdGenerator::new(0)).is_err(),
        "-0.0 radius should error"
    );
    // f64::MIN_POSITIVE radius → OK (finite, strictly > 0)
    assert!(
        make_sphere(f64::MIN_POSITIVE, Point::origin(), &mut IdGenerator::new(0)).is_ok(),
        "MIN_POSITIVE radius should succeed"
    );
    // NaN center → error
    let nan_center = Point::new(f64::NAN, 0.0, 0.0);
    assert!(
        make_sphere(1.0, nan_center, &mut IdGenerator::new(0)).is_err(),
        "NaN center should error"
    );
    // Inf center → error
    let inf_center = Point::new(0.0, f64::INFINITY, 0.0);
    assert!(
        make_sphere(1.0, inf_center, &mut IdGenerator::new(0)).is_err(),
        "Inf center should error"
    );
}

/// EC05: make_cuboid rejects special float values without panicking.
#[test]
fn ec05_make_cuboid_numerical_boundaries() {
    // NaN dimension → error
    assert!(
        make_cuboid(f64::NAN, 1.0, 1.0, "test", &mut IdGenerator::new(0)).is_err(),
        "NaN width should error"
    );
    // +Inf dimension → error
    assert!(
        make_cuboid(f64::INFINITY, 1.0, 1.0, "test", &mut IdGenerator::new(0)).is_err(),
        "+Inf width should error"
    );
    // -0.0 dimension → error
    assert!(
        make_cuboid(-0.0, 1.0, 1.0, "test", &mut IdGenerator::new(0)).is_err(),
        "-0.0 width should error"
    );
    // Negative dimension → error
    assert!(
        make_cuboid(-1.0, 1.0, 1.0, "test", &mut IdGenerator::new(0)).is_err(),
        "negative width should error"
    );
    // Zero dimension → error
    assert!(
        make_cuboid(1.0, 0.0, 1.0, "test", &mut IdGenerator::new(0)).is_err(),
        "zero height should error"
    );
}

/// EC06: Boolean cut result passes validate_manifold.
#[test]
fn ec06_manifold_validation() {
    let mut gen = IdGenerator::new(0);
    let solid = make_box_minus_sphere(&mut gen);
    solid
        .validate_manifold()
        .expect("cut result should be a valid manifold");
}
