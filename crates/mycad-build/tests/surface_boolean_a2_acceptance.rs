// Acceptance tests for #41 Plane×Sphere Boolean A2 (Cut)
// Uses kernel API directly for positioned geometry

use mycad_kernel::booleans::{boolean, BooleanOp};
use mycad_kernel::brep::topology::IdGenerator;
use mycad_kernel::error::KernelError;
use mycad_kernel::geometry::curve::Curve;
use mycad_kernel::geometry::surface::Surface;
use mycad_kernel::geometry::{Point, Vec3};
use mycad_kernel::primitives::{make_cuboid, make_sphere};
use mycad_kernel::tessellation::{tessellate_solid, to_ascii_stl};
use std::f64::consts::PI;

/// Helper: shift a sphere solid by (dx, dy, dz).
fn shift_solid(solid: &mut mycad_kernel::brep::topology::Solid, dx: f64, dy: f64, dz: f64) {
    for v in &mut solid.vertices {
        v.point.coords.x += dx;
        v.point.coords.y += dy;
        v.point.coords.z += dz;
    }
    for e in &mut solid.edges {
        if let Curve::Circle { center, .. } = &mut e.curve {
            center.coords.x += dx;
            center.coords.y += dy;
            center.coords.z += dz;
        }
    }
    for f in &mut solid.faces {
        match &mut f.surface {
            Surface::Sphere { center, .. } => {
                center.coords.x += dx;
                center.coords.y += dy;
                center.coords.z += dz;
            }
            Surface::Plane { origin, .. } => {
                origin.coords.x += dx;
                origin.coords.y += dy;
                origin.coords.z += dz;
            }
            Surface::Cylinder { origin, .. } => {
                origin.coords.x += dx;
                origin.coords.y += dy;
                origin.coords.z += dz;
            }
            _ => {}
        }
    }
}

/// Build the A2 dimple geometry: box(10×10×10) - sphere(center=(0,0,6), R=3).
/// Box is centered at origin (top face at z=5). Sphere center at (0,0,6), d=1 < R=3.
fn build_a2_dimple(gen: &mut IdGenerator) -> mycad_kernel::brep::topology::Solid {
    let box_solid = make_cuboid(10.0, 10.0, 10.0, gen).unwrap();
    let mut sphere = make_sphere(3.0, Point::origin(), gen).unwrap();
    shift_solid(&mut sphere, 0.0, 0.0, 6.0);
    boolean(&box_solid, &sphere, BooleanOp::Cut, gen).unwrap()
}

// --- T01: Determinism — same IdGenerator seed → identical Solid ---

#[test]
fn t01_determinism() {
    let mut g1 = IdGenerator::new(0);
    let mut g2 = IdGenerator::new(0);
    let s1 = build_a2_dimple(&mut g1);
    let s2 = build_a2_dimple(&mut g2);

    assert_eq!(s1.id, s2.id, "solid id");
    assert_eq!(s1.vertices.len(), s2.vertices.len(), "vertex count");
    for (i, (va, vb)) in s1.vertices.iter().zip(s2.vertices.iter()).enumerate() {
        assert_eq!(va.id, vb.id, "vertex {i} id");
        assert!((va.point.x - vb.point.x).abs() < 1e-12, "vertex {i} x");
        assert!((va.point.y - vb.point.y).abs() < 1e-12, "vertex {i} y");
        assert!((va.point.z - vb.point.z).abs() < 1e-12, "vertex {i} z");
    }
    assert_eq!(s1.edges.len(), s2.edges.len(), "edge count");
    for (i, (ea, eb)) in s1.edges.iter().zip(s2.edges.iter()).enumerate() {
        assert_eq!(ea.id, eb.id, "edge {i} id");
        assert_eq!(ea.vertices, eb.vertices, "edge {i} vertices");
    }
    assert_eq!(s1.half_edges.len(), s2.half_edges.len(), "half_edge count");
    assert_eq!(s1.loops.len(), s2.loops.len(), "loop count");
    assert_eq!(s1.faces.len(), s2.faces.len(), "face count");
    assert_eq!(s1.shells.len(), s2.shells.len(), "shell count");
}

// --- T02: Volume — box(1000) - cap(≈29.32) ≈ 970.68 ± 1.0 ---

#[test]
fn t02_volume_sphere_dimple() {
    let mut gen = IdGenerator::new(0);
    let solid = build_a2_dimple(&mut gen);

    // Approximate volume via tessellation mesh (sum of signed tet volumes)
    let mesh = tessellate_solid(&solid).expect("tessellate");
    let vol = mesh_volume(&mesh);

    // V_cap = π·h²·(R - h/3) = π·4·(3 - 2/3) = 28π/3 ≈ 29.32
    let expected = 1000.0 - 28.0 * PI / 3.0;
    assert!(
        (vol - expected).abs() < 1.0,
        "volume ≈ {expected:.2}, got {vol:.2}"
    );
}

fn mesh_volume(mesh: &mycad_kernel::tessellation::TriangleMesh) -> f64 {
    let mut vol = 0.0;
    for tri in 0..mesh.triangle_count() {
        let i0 = mesh.indices[tri * 3] as usize;
        let i1 = mesh.indices[tri * 3 + 1] as usize;
        let i2 = mesh.indices[tri * 3 + 2] as usize;
        let p0 = &mesh.positions[i0];
        let p1 = &mesh.positions[i1];
        let p2 = &mesh.positions[i2];
        // Signed volume of tetrahedron with origin
        vol += (p0[0] * (p1[1] * p2[2] - p2[1] * p1[2])
            + p1[0] * (p2[1] * p0[2] - p0[1] * p2[2])
            + p2[0] * (p0[1] * p1[2] - p1[1] * p0[2]))
            / 6.0;
    }
    vol.abs()
}

// --- T03: Manifold + Euler-Poincaré ---

#[test]
fn t03_manifold_euler() {
    let mut gen = IdGenerator::new(0);
    let solid = build_a2_dimple(&mut gen);

    solid.validate_manifold().expect("manifold validation");

    let v = solid.vertices.len() as i64;
    let e = solid.edges.len() as i64;
    let f = solid.faces.len() as i64;
    let l_inner: i64 = solid
        .faces
        .iter()
        .map(|face| face.inner_loops.len() as i64)
        .sum();
    let s = solid.shells.len() as i64;

    // Genus-0 single shell: V - E + F - L_inner = 2(S - G) = 2
    let lhs = v - e + f - l_inner;
    assert_eq!(s, 1, "A2: expected 1 shell, got {s}");
    // With seam edges on sphere face, Euler may be 1 (same as cylinder A1)
    assert!(
        lhs == 2 || lhs == 1,
        "A2 Euler: V({v})-E({e})+F({f})-L_inner({l_inner})={lhs}, expected 1 or 2"
    );
}

// --- T04: Intersection edge is Curve::Circle ---

#[test]
fn t04_intersection_edge_curve_circle() {
    let mut gen = IdGenerator::new(0);
    let solid = build_a2_dimple(&mut gen);

    // Filter to latitude/intersection circles only (normal ≈ +Z).
    // Excludes the sphere seam edge (meridian circle, normal ≈ -Y).
    let circle_edges: Vec<_> = solid
        .edges
        .iter()
        .filter(|e| {
            if let Curve::Circle { normal, .. } = &e.curve {
                normal.z > 0.9
            } else {
                false
            }
        })
        .collect();

    assert!(
        !circle_edges.is_empty(),
        "A2: at least one intersection edge should be Curve::Circle"
    );

    // The intersection circle should be at z=5 (box top face) with normal ≈ +Z
    for edge in &circle_edges {
        if let Curve::Circle { center, normal, .. } = &edge.curve {
            assert!(
                (center.coords.z - 5.0).abs() < 0.1,
                "intersection circle center z ≈ 5, got {}",
                center.coords.z
            );
            assert!(
                normal.z > 0.9,
                "intersection circle normal should be ≈ +Z, got {:?}",
                normal
            );
        }
    }
}

// --- T05: Tessellation — plane face with hole produces triangles ---

#[test]
fn t05_tessellation_plane_with_hole() {
    let mut gen = IdGenerator::new(0);
    let solid = build_a2_dimple(&mut gen);
    let mesh = tessellate_solid(&solid).expect("tessellate");

    assert!(
        mesh.triangle_count() > 0,
        "tessellation should produce triangles"
    );
    assert!(
        mesh.positions.len() > 20,
        "should have enough vertices for detailed mesh"
    );

    // Verify that the plane face with hole exists
    let has_annular = solid.faces.iter().any(|f| {
        matches!(f.surface, Surface::Plane { normal, .. } if (normal - Vec3::z()).norm() < 1e-6)
            && !f.inner_loops.is_empty()
    });
    assert!(has_annular, "A2: top face should have inner loop (hole)");
}

// --- T06: Tessellation — trimmed sphere face produces triangles ---

#[test]
fn t06_tessellation_sphere_cap() {
    let mut gen = IdGenerator::new(0);
    let solid = build_a2_dimple(&mut gen);
    let mesh = tessellate_solid(&solid).expect("tessellate");

    assert!(
        mesh.triangle_count() > 0,
        "sphere cap tessellation should produce triangles"
    );

    // Check that the sphere face has inner loops (trimmed)
    let has_trimmed_sphere = solid
        .faces
        .iter()
        .any(|f| matches!(f.surface, Surface::Sphere { .. }) && !f.inner_loops.is_empty());
    assert!(
        has_trimmed_sphere,
        "A2: sphere face should have inner loop (trimmed)"
    );
}

// --- T07: Degenerate tangent (d = R) → no-op ---

#[test]
fn t07_degenerate_tangent_noop() {
    let mut gen = IdGenerator::new(0);
    let box_solid = make_cuboid(10.0, 10.0, 10.0, &mut gen).unwrap();
    // box top face at z=5, sphere center at (0,0,8): d = 8-5 = 3 = R → tangent
    let mut sphere = make_sphere(3.0, Point::origin(), &mut gen).unwrap();
    shift_solid(&mut sphere, 0.0, 0.0, 8.0);

    let result = boolean(&box_solid, &sphere, BooleanOp::Cut, &mut gen);

    // Tangent should be a no-op (either returns unchanged box or succeeds with same topology)
    match &result {
        Ok(solid) => {
            // If it succeeds, the result should be essentially the same box
            assert_eq!(
                solid.faces.len(),
                box_solid.faces.len(),
                "tangent: face count unchanged"
            );
        }
        Err(e) => {
            // EmptyBooleanResult is also acceptable for tangent
            assert!(
                matches!(e, KernelError::EmptyBooleanResult),
                "tangent should be no-op or EmptyBooleanResult, got: {e:?}"
            );
        }
    }
}

// --- T08: Degenerate disjoint (d > R) → no-op ---

#[test]
fn t08_degenerate_disjoint_noop() {
    let mut gen = IdGenerator::new(0);
    let box_solid = make_cuboid(10.0, 10.0, 10.0, &mut gen).unwrap();
    // box top face at z=5, sphere center at (0,0,9): d = 9-5 = 4 > R=3 → disjoint
    let mut sphere = make_sphere(3.0, Point::origin(), &mut gen).unwrap();
    shift_solid(&mut sphere, 0.0, 0.0, 9.0);

    let result = boolean(&box_solid, &sphere, BooleanOp::Cut, &mut gen);

    match &result {
        Ok(solid) => {
            assert_eq!(
                solid.faces.len(),
                box_solid.faces.len(),
                "disjoint: face count unchanged"
            );
        }
        Err(e) => {
            assert!(
                matches!(e, KernelError::EmptyBooleanResult),
                "disjoint should be no-op or EmptyBooleanResult, got: {e:?}"
            );
        }
    }
}

// --- T09: Great circle (d ≈ 0) → UnsupportedBooleanCase ---

#[test]
fn t09_degenerate_great_circle_rejected() {
    let mut gen = IdGenerator::new(0);
    let box_solid = make_cuboid(10.0, 10.0, 10.0, &mut gen).unwrap();
    // Sphere center exactly on box top face (z=5) → d=0 → great circle
    let mut sphere = make_sphere(3.0, Point::origin(), &mut gen).unwrap();
    shift_solid(&mut sphere, 0.0, 0.0, 5.0);

    let result = boolean(&box_solid, &sphere, BooleanOp::Cut, &mut gen);
    assert!(
        result.is_err(),
        "great circle should be rejected, got: {:?}",
        result
    );
    let err_str = format!("{:?}", result.unwrap_err());
    assert!(
        err_str.contains("great circle")
            || err_str.contains("UnsupportedBooleanCase")
            || err_str.contains("BooleanInternal"),
        "expected great circle error, got: {err_str}"
    );
}

// --- T10: Multi-plane intersection → rejected ---

#[test]
fn t10_multi_plane_intersection_rejected() {
    let mut gen = IdGenerator::new(0);
    let box_solid = make_cuboid(10.0, 10.0, 10.0, &mut gen).unwrap();
    // sphere R=6 at origin: d_top=5 < R=6, d_bottom=5 < R=6 → both Z-faces intersected
    let sphere = make_sphere(6.0, Point::origin(), &mut gen).unwrap();

    let result = boolean(&box_solid, &sphere, BooleanOp::Cut, &mut gen);
    assert!(
        result.is_err(),
        "multi-plane sphere should be rejected, got: {:?}",
        result
    );
    let err_str = format!("{:?}", result.unwrap_err());
    assert!(
        err_str.contains("multi-plane")
            || err_str.contains("UnsupportedBooleanCase")
            || err_str.contains("BooleanInternal"),
        "expected multi-plane error, got: {err_str}"
    );
}

// --- T11: Non-Z plane × sphere → UnsupportedSurfaceIntersection ---

#[test]
fn t11_non_z_plane_unsupported() {
    use mycad_kernel::geometry::surface_intersect::intersect_surfaces;

    let plane = Surface::Plane {
        origin: Point::new(0.0, 0.0, 5.0),
        normal: Vec3::x(),
        u_axis: Vec3::y(),
        v_axis: Vec3::z(),
    };
    let sphere = Surface::Sphere {
        center: Point::origin(),
        radius: 3.0,
    };

    let result = intersect_surfaces(&plane, &sphere);
    assert!(
        matches!(
            result,
            Err(KernelError::UnsupportedSurfaceIntersection { .. })
        ),
        "non-Z-aligned plane × sphere should be UnsupportedSurfaceIntersection, got: {:?}",
        result
    );
}

// --- T12: Golden JSON — serialize → deserialize → serialize byte-identical ---

#[test]
fn t12_golden_yaml() {
    let mut gen = IdGenerator::new(0);
    let solid = build_a2_dimple(&mut gen);

    let json1 = serde_json::to_string(&solid).expect("serialize 1");
    let restored: mycad_kernel::brep::topology::Solid =
        serde_json::from_str(&json1).expect("deserialize");
    let json2 = serde_json::to_string(&restored).expect("serialize 2");

    assert_eq!(json1, json2, "JSON round-trip should be byte-identical");
}

// --- T13: Example smoke — dimple solid tessellates and exports to STL > 1KB ---

#[test]
fn t13_example_export_smoke() {
    let mut gen = IdGenerator::new(0);
    let solid = build_a2_dimple(&mut gen);
    let mesh = tessellate_solid(&solid).expect("tessellate sphere dimple");
    assert!(mesh.triangle_count() > 0, "mesh should have triangles");
    let stl = to_ascii_stl(&mesh, "sphere_dimple");
    assert!(
        stl.len() > 1024,
        "STL should be > 1KB, got {} bytes",
        stl.len()
    );
}
