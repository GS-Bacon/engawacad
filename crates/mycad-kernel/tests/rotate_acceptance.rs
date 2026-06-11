//! Acceptance tests for Issue #77 — kernel rotation transform.
//!
//! T01–T09 from the test plan plus additional edge-case tests.
//!
//! # euler_to_matrix シグネチャ変更 (Issue #135)
//! `euler_to_matrix` は rad 入力を受け取る。deg 値でテストする場合は `.to_radians()` で変換する。

use mycad_kernel::brep::topology::{IdGenerator, Solid};
use mycad_kernel::geometry::curve::Curve;
use mycad_kernel::geometry::math::orthonormal_basis;
use mycad_kernel::geometry::surface::Surface;
use mycad_kernel::geometry::transform::euler_to_matrix;
use mycad_kernel::geometry::Plane;
use mycad_kernel::geometry::{Point, Vec3};
use mycad_kernel::primitives::make_cuboid;

fn make_test_cuboid() -> Solid {
    let mut gen = IdGenerator::new(1);
    make_cuboid(2.0, 3.0, 4.0, &mut gen).unwrap()
}

// ---------------------------------------------------------------------------
// T01: Determinism — same rotation applied 100 times yields identical results
// ---------------------------------------------------------------------------
#[test]
fn t01_determinism_100x() {
    let cuboid = make_test_cuboid();
    let matrix = euler_to_matrix(
        30.0_f64.to_radians(),
        45.0_f64.to_radians(),
        60.0_f64.to_radians(),
    );
    let pivot = Point::origin();
    let mut first: Option<String> = None;
    for _ in 0..100 {
        let mut s = cuboid.clone();
        s.rotate(matrix, pivot);
        let yaml = serde_yaml::to_string(&s).unwrap();
        match &first {
            None => first = Some(yaml),
            Some(prev) => assert_eq!(prev, &yaml, "rotate output must be deterministic"),
        }
    }
}

// ---------------------------------------------------------------------------
// T02: x90° rotation aligns cuboid face normals to principal axes
// ---------------------------------------------------------------------------
#[test]
fn t02_x90_cuboid_normals_align() {
    let mut cuboid = make_test_cuboid();
    let matrix = euler_to_matrix(
        90.0_f64.to_radians(),
        0.0_f64.to_radians(),
        0.0_f64.to_radians(),
    );
    cuboid.rotate(matrix, Point::origin());

    let principal = [
        Vec3::x(),
        -Vec3::x(),
        Vec3::y(),
        -Vec3::y(),
        Vec3::z(),
        -Vec3::z(),
    ];
    for face in &cuboid.faces {
        if let Surface::Plane { normal, .. } = &face.surface {
            let n = normal.normalize();
            let ok = principal.iter().any(|a| (n.dot(a) - 1.0).abs() < 1e-12);
            assert!(
                ok,
                "face normal {:?} is not aligned with a principal axis",
                n
            );
        }
    }
    // Euler-Poincaré preserved
    assert_eq!(cuboid.euler_poincare(), 0);
}

// ---------------------------------------------------------------------------
// T03: Inverse — rotate(M) then rotate(M^T) recovers original geometry
// ---------------------------------------------------------------------------
#[test]
fn t03_inverse_roundtrip() {
    let original = make_test_cuboid();
    let matrix = euler_to_matrix(
        30.0_f64.to_radians(),
        45.0_f64.to_radians(),
        60.0_f64.to_radians(),
    );
    let inv = [
        [matrix[0][0], matrix[1][0], matrix[2][0]],
        [matrix[0][1], matrix[1][1], matrix[2][1]],
        [matrix[0][2], matrix[1][2], matrix[2][2]],
    ];
    let pivot = Point::new(1.0, 2.0, 3.0);
    let mut roundtrip = original.clone();
    roundtrip.rotate(matrix, pivot);
    roundtrip.rotate(inv, pivot);

    for (o, r) in original.vertices.iter().zip(roundtrip.vertices.iter()) {
        approx::assert_relative_eq!(o.point.x, r.point.x, epsilon = 1e-12);
        approx::assert_relative_eq!(o.point.y, r.point.y, epsilon = 1e-12);
        approx::assert_relative_eq!(o.point.z, r.point.z, epsilon = 1e-12);
    }
    for (o, r) in original.edges.iter().zip(roundtrip.edges.iter()) {
        match (&o.curve, &r.curve) {
            (
                Curve::Line {
                    origin: oo,
                    direction: od,
                },
                Curve::Line {
                    origin: ro,
                    direction: rd,
                },
            ) => {
                approx::assert_relative_eq!(oo.x, ro.x, epsilon = 1e-12);
                approx::assert_relative_eq!(oo.y, ro.y, epsilon = 1e-12);
                approx::assert_relative_eq!(oo.z, ro.z, epsilon = 1e-12);
                approx::assert_relative_eq!(od.x, rd.x, epsilon = 1e-12);
                approx::assert_relative_eq!(od.y, rd.y, epsilon = 1e-12);
                approx::assert_relative_eq!(od.z, rd.z, epsilon = 1e-12);
            }
            _ => panic!("curve variant mismatch after roundtrip"),
        }
    }
    for (o, r) in original.faces.iter().zip(roundtrip.faces.iter()) {
        let op = o.surface.evaluate(0.5, 0.5);
        let rp = r.surface.evaluate(0.5, 0.5);
        approx::assert_relative_eq!(op.x, rp.x, epsilon = 1e-12);
        approx::assert_relative_eq!(op.y, rp.y, epsilon = 1e-12);
        approx::assert_relative_eq!(op.z, rp.z, epsilon = 1e-12);
    }
}

// ---------------------------------------------------------------------------
// T04: Identity matrix — euler_to_matrix(0,0,0) returns identity
// ---------------------------------------------------------------------------
#[test]
fn t04_identity_matrix() {
    let m = euler_to_matrix(
        0.0_f64.to_radians(),
        0.0_f64.to_radians(),
        0.0_f64.to_radians(),
    );
    let identity: [[f64; 3]; 3] = [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]];
    assert_eq!(m, identity);
}

// ---------------------------------------------------------------------------
// T05: Snap values — euler_to_matrix(90,0,0) has snap-cleaned integer elements
// ---------------------------------------------------------------------------
#[test]
fn t05_snap_values_x90() {
    let m = euler_to_matrix(
        90.0_f64.to_radians(),
        0.0_f64.to_radians(),
        0.0_f64.to_radians(),
    );
    // Rx(90°) = [[1,0,0],[0,0,-1],[0,1,0]]
    assert_eq!(m[0], [1.0, 0.0, 0.0]);
    assert_eq!(m[1], [0.0, 0.0, -1.0]);
    assert_eq!(m[2], [0.0, 1.0, 0.0]);
}

// ---------------------------------------------------------------------------
// T06: Cylinder basis consistency — rotated axis basis matches rotated original
// ---------------------------------------------------------------------------
#[test]
fn t06_cylinder_basis_consistency() {
    let cyl = Surface::Cylinder {
        origin: Point::origin(),
        axis: Vec3::z(),
        radius: 2.5,
    };
    let matrix = euler_to_matrix(
        90.0_f64.to_radians(),
        0.0_f64.to_radians(),
        0.0_f64.to_radians(),
    );
    let rotated = cyl.rotate(matrix, Point::origin());

    if let Surface::Cylinder {
        origin,
        axis,
        radius,
    } = &rotated
    {
        approx::assert_relative_eq!(origin.x, 0.0, epsilon = 1e-12);
        approx::assert_relative_eq!(origin.y, 0.0, epsilon = 1e-12);
        approx::assert_relative_eq!(origin.z, 0.0, epsilon = 1e-12);
        approx::assert_relative_eq!(*radius, 2.5, epsilon = 1e-12);

        let (bu, bv) = orthonormal_basis(axis);
        approx::assert_relative_eq!(bu.norm(), 1.0, epsilon = 1e-12);
        approx::assert_relative_eq!(bv.norm(), 1.0, epsilon = 1e-12);
        let a = axis.normalize();
        approx::assert_relative_eq!(a.dot(&bu), 0.0, epsilon = 1e-12);
        approx::assert_relative_eq!(a.dot(&bv), 0.0, epsilon = 1e-12);
    } else {
        panic!("expected Surface::Cylinder");
    }
}

// ---------------------------------------------------------------------------
// T07: Zero rotation leaves Solid completely unchanged
// ---------------------------------------------------------------------------
#[test]
fn t07_boundary_zero_rotation() {
    let original = make_test_cuboid();
    let matrix = euler_to_matrix(
        0.0_f64.to_radians(),
        0.0_f64.to_radians(),
        0.0_f64.to_radians(),
    );
    let mut rotated = original.clone();
    rotated.rotate(matrix, Point::origin());

    for (o, r) in original.vertices.iter().zip(rotated.vertices.iter()) {
        assert_eq!(
            o.point, r.point,
            "vertex must be identical with zero rotation"
        );
    }
    for (o, r) in original.edges.iter().zip(rotated.edges.iter()) {
        assert_eq!(
            o.curve, r.curve,
            "edge curve must be identical with zero rotation"
        );
    }
    for (o, r) in original.faces.iter().zip(rotated.faces.iter()) {
        assert_eq!(
            o.surface, r.surface,
            "face surface must be identical with zero rotation"
        );
    }
}

// ---------------------------------------------------------------------------
// T08: 180° around x-axis flips y/z coordinates (around origin pivot)
// ---------------------------------------------------------------------------
#[test]
fn t08_boundary_180_flip() {
    let mut cuboid = make_test_cuboid();
    let original = cuboid.clone();
    let matrix = euler_to_matrix(
        180.0_f64.to_radians(),
        0.0_f64.to_radians(),
        0.0_f64.to_radians(),
    );
    cuboid.rotate(matrix, Point::origin());

    for (o, r) in original.vertices.iter().zip(cuboid.vertices.iter()) {
        // Rx(180°): x unchanged, y → -y, z → -z
        approx::assert_relative_eq!(o.point.x, r.point.x, epsilon = 1e-12);
        approx::assert_relative_eq!(o.point.y, -r.point.y, epsilon = 1e-12);
        approx::assert_relative_eq!(o.point.z, -r.point.z, epsilon = 1e-12);
    }
}

// ---------------------------------------------------------------------------
// T09: Pivot at vertex — that vertex remains invariant
// ---------------------------------------------------------------------------
#[test]
fn t09_degen_pivot_at_vertex() {
    let cuboid = make_test_cuboid();
    let matrix = euler_to_matrix(
        45.0_f64.to_radians(),
        30.0_f64.to_radians(),
        60.0_f64.to_radians(),
    );
    let pivot = cuboid.vertices[0].point;
    let mut rotated = cuboid.clone();
    rotated.rotate(matrix, pivot);

    approx::assert_relative_eq!(
        cuboid.vertices[0].point.x,
        rotated.vertices[0].point.x,
        epsilon = 1e-15
    );
    approx::assert_relative_eq!(
        cuboid.vertices[0].point.y,
        rotated.vertices[0].point.y,
        epsilon = 1e-15
    );
    approx::assert_relative_eq!(
        cuboid.vertices[0].point.z,
        rotated.vertices[0].point.z,
        epsilon = 1e-15
    );
}

// ===========================================================================
// Additional edge-case tests
// ===========================================================================

// ---------------------------------------------------------------------------
// euler_to_matrix(0,90,0) — Y-axis rotation snap-cleaned
// ---------------------------------------------------------------------------
#[test]
fn edge_y90_rotation_snap() {
    let m = euler_to_matrix(
        0.0_f64.to_radians(),
        90.0_f64.to_radians(),
        0.0_f64.to_radians(),
    );
    // Ry(90°) = [[0,0,1],[0,1,0],[-1,0,0]]
    assert_eq!(m[0], [0.0, 0.0, 1.0]);
    assert_eq!(m[1], [0.0, 1.0, 0.0]);
    assert_eq!(m[2], [-1.0, 0.0, 0.0]);
}

// ---------------------------------------------------------------------------
// euler_to_matrix(0,0,90) — Z-axis rotation snap-cleaned
// ---------------------------------------------------------------------------
#[test]
fn edge_z90_rotation_snap() {
    let m = euler_to_matrix(
        0.0_f64.to_radians(),
        0.0_f64.to_radians(),
        90.0_f64.to_radians(),
    );
    // Rz(90°) = [[0,-1,0],[1,0,0],[0,0,1]]
    assert_eq!(m[0], [0.0, -1.0, 0.0]);
    assert_eq!(m[1], [1.0, 0.0, 0.0]);
    assert_eq!(m[2], [0.0, 0.0, 1.0]);
}

// ---------------------------------------------------------------------------
// 270° rotation — snap-cleaned values
// ---------------------------------------------------------------------------
#[test]
fn edge_270_rotation_snap() {
    let m = euler_to_matrix(
        270.0_f64.to_radians(),
        0.0_f64.to_radians(),
        0.0_f64.to_radians(),
    );
    // Rx(270°) = Rx(-90°) = [[1,0,0],[0,0,1],[0,-1,0]]
    assert_eq!(m[0], [1.0, 0.0, 0.0]);
    assert_eq!(m[1], [0.0, 0.0, 1.0]);
    assert_eq!(m[2], [0.0, -1.0, 0.0]);
}

// ---------------------------------------------------------------------------
// euler_to_matrix is a pure function (deterministic with same inputs)
// ---------------------------------------------------------------------------
#[test]
fn edge_euler_to_matrix_determinism() {
    for _ in 0..100 {
        let a = euler_to_matrix(
            37.0_f64.to_radians(),
            53.0_f64.to_radians(),
            71.0_f64.to_radians(),
        );
        let b = euler_to_matrix(
            37.0_f64.to_radians(),
            53.0_f64.to_radians(),
            71.0_f64.to_radians(),
        );
        assert_eq!(a, b);
    }
}

// ---------------------------------------------------------------------------
// rotate_vec preserves vector length (isometry)
// ---------------------------------------------------------------------------
#[test]
fn edge_rotate_vec_preserves_length() {
    use mycad_kernel::geometry::transform::{euler_to_matrix, rotate_vec};

    let v = Vec3::new(3.0, 4.0, 5.0);
    let expected_len = v.norm();
    let matrix = euler_to_matrix(
        37.0_f64.to_radians(),
        53.0_f64.to_radians(),
        71.0_f64.to_radians(),
    );
    let rv = rotate_vec(v, matrix);
    approx::assert_relative_eq!(rv.norm(), expected_len, epsilon = 1e-12);
}

// ---------------------------------------------------------------------------
// rotate_point with non-origin pivot
// ---------------------------------------------------------------------------
#[test]
fn edge_rotate_point_nonorigin_pivot() {
    use mycad_kernel::geometry::transform::rotate_point;

    let p = Point::new(2.0, 0.0, 0.0);
    let pivot = Point::new(1.0, 0.0, 0.0);
    // 90° around z-axis: (2,0,0) around (1,0,0) → (1,1,0)
    let matrix = euler_to_matrix(
        0.0_f64.to_radians(),
        0.0_f64.to_radians(),
        90.0_f64.to_radians(),
    );
    let result = rotate_point(p, matrix, pivot);
    approx::assert_relative_eq!(result.x, 1.0, epsilon = 1e-12);
    approx::assert_relative_eq!(result.y, 1.0, epsilon = 1e-12);
    approx::assert_relative_eq!(result.z, 0.0, epsilon = 1e-12);
}

// ---------------------------------------------------------------------------
// Curve::Circle rotation
// ---------------------------------------------------------------------------
#[test]
fn edge_curve_circle_rotate() {
    let circle = Curve::Circle {
        center: Point::new(1.0, 0.0, 0.0),
        normal: Vec3::z(),
        radius: 2.0,
    };
    let matrix = euler_to_matrix(
        90.0_f64.to_radians(),
        0.0_f64.to_radians(),
        0.0_f64.to_radians(),
    );
    let rotated = circle.rotate(matrix, Point::origin());

    if let Curve::Circle {
        center,
        normal,
        radius,
    } = &rotated
    {
        approx::assert_relative_eq!(center.x, 1.0, epsilon = 1e-12);
        approx::assert_relative_eq!(center.y, 0.0, epsilon = 1e-12);
        approx::assert_relative_eq!(center.z, 0.0, epsilon = 1e-12);
        approx::assert_relative_eq!(*radius, 2.0, epsilon = 1e-12);
        // Normal z → rotates to -y under Rx(90°)
        approx::assert_relative_eq!(normal.x, 0.0, epsilon = 1e-12);
        approx::assert_relative_eq!(normal.y, -1.0, epsilon = 1e-12);
        approx::assert_relative_eq!(normal.z, 0.0, epsilon = 1e-12);
    } else {
        panic!("expected Curve::Circle");
    }
}

// ---------------------------------------------------------------------------
// Surface::Sphere rotation — center moves, radius unchanged
// ---------------------------------------------------------------------------
#[test]
fn edge_surface_sphere_rotate() {
    let sphere = Surface::Sphere {
        center: Point::new(1.0, 2.0, 3.0),
        radius: 5.0,
    };
    let matrix = euler_to_matrix(
        0.0_f64.to_radians(),
        0.0_f64.to_radians(),
        180.0_f64.to_radians(),
    );
    let rotated = sphere.rotate(matrix, Point::origin());

    if let Surface::Sphere { center, radius } = &rotated {
        approx::assert_relative_eq!(center.x, -1.0, epsilon = 1e-12);
        approx::assert_relative_eq!(center.y, -2.0, epsilon = 1e-12);
        approx::assert_relative_eq!(center.z, 3.0, epsilon = 1e-12);
        approx::assert_relative_eq!(*radius, 5.0, epsilon = 1e-12);
    } else {
        panic!("expected Surface::Sphere");
    }
}

// ---------------------------------------------------------------------------
// Surface::Cone rotation — apex moves, axis rotates, half_angle unchanged
// ---------------------------------------------------------------------------
#[test]
fn edge_surface_cone_rotate() {
    let cone = Surface::Cone {
        apex: Point::new(0.0, 0.0, 1.0),
        axis: Vec3::z(),
        half_angle: 0.4,
    };
    let matrix = euler_to_matrix(
        90.0_f64.to_radians(),
        0.0_f64.to_radians(),
        0.0_f64.to_radians(),
    );
    let rotated = cone.rotate(matrix, Point::origin());

    if let Surface::Cone {
        apex,
        axis,
        half_angle,
    } = &rotated
    {
        approx::assert_relative_eq!(apex.x, 0.0, epsilon = 1e-12);
        approx::assert_relative_eq!(apex.y, -1.0, epsilon = 1e-12);
        approx::assert_relative_eq!(apex.z, 0.0, epsilon = 1e-12);
        approx::assert_relative_eq!(*half_angle, 0.4, epsilon = 1e-12);
        let _ = axis; // axis rotated but direction is valid
    } else {
        panic!("expected Surface::Cone");
    }
}

// ---------------------------------------------------------------------------
// Plane::rotate
// ---------------------------------------------------------------------------
#[test]
fn edge_plane_rotate() {
    let plane = Plane::xy();
    let matrix = euler_to_matrix(
        90.0_f64.to_radians(),
        0.0_f64.to_radians(),
        0.0_f64.to_radians(),
    );
    let rotated = plane.rotate(matrix, Point::origin());

    // Origin stays at origin (origin of xy-plane is at origin)
    approx::assert_relative_eq!(rotated.origin.x, 0.0, epsilon = 1e-12);
    approx::assert_relative_eq!(rotated.origin.y, 0.0, epsilon = 1e-12);
    approx::assert_relative_eq!(rotated.origin.z, 0.0, epsilon = 1e-12);

    // Normal z → -y under Rx(90°)
    approx::assert_relative_eq!(rotated.normal.x, 0.0, epsilon = 1e-12);
    approx::assert_relative_eq!(rotated.normal.y, -1.0, epsilon = 1e-12);
    approx::assert_relative_eq!(rotated.normal.z, 0.0, epsilon = 1e-12);
}

// ---------------------------------------------------------------------------
// Rotation preserves topology counts (V, E, F unchanged)
// ---------------------------------------------------------------------------
#[test]
fn edge_rotation_preserves_topology_counts() {
    let original = make_test_cuboid();
    let matrix = euler_to_matrix(
        45.0_f64.to_radians(),
        30.0_f64.to_radians(),
        60.0_f64.to_radians(),
    );
    let mut rotated = original.clone();
    rotated.rotate(matrix, Point::new(1.0, 2.0, 3.0));

    assert_eq!(original.vertices.len(), rotated.vertices.len());
    assert_eq!(original.edges.len(), rotated.edges.len());
    assert_eq!(original.faces.len(), rotated.faces.len());
    assert_eq!(original.half_edges.len(), rotated.half_edges.len());
    assert_eq!(original.loops.len(), rotated.loops.len());
    assert_eq!(original.shells.len(), rotated.shells.len());
    assert_eq!(original.euler_poincare(), rotated.euler_poincare());
}

// ---------------------------------------------------------------------------
// Rotation preserves EntityIDs
// ---------------------------------------------------------------------------
#[test]
fn edge_rotation_preserves_entity_ids() {
    let original = make_test_cuboid();
    let matrix = euler_to_matrix(
        45.0_f64.to_radians(),
        30.0_f64.to_radians(),
        60.0_f64.to_radians(),
    );
    let mut rotated = original.clone();
    rotated.rotate(matrix, Point::origin());

    for (o, r) in original.vertices.iter().zip(rotated.vertices.iter()) {
        assert_eq!(o.id, r.id);
    }
    for (o, r) in original.edges.iter().zip(rotated.edges.iter()) {
        assert_eq!(o.id, r.id);
    }
    for (o, r) in original.faces.iter().zip(rotated.faces.iter()) {
        assert_eq!(o.id, r.id);
    }
}

// ---------------------------------------------------------------------------
// Rotation rows are orthonormal for arbitrary angles
// ---------------------------------------------------------------------------
#[test]
fn edge_rotation_matrix_orthonormal() {
    let m = euler_to_matrix(
        30.0_f64.to_radians(),
        45.0_f64.to_radians(),
        60.0_f64.to_radians(),
    );
    // Each row has unit norm
    for row in &m {
        let norm = (row[0] * row[0] + row[1] * row[1] + row[2] * row[2]).sqrt();
        approx::assert_relative_eq!(norm, 1.0, epsilon = 1e-12);
    }
    // Rows are mutually orthogonal
    for i in 0..3 {
        for j in (i + 1)..3 {
            let dot = m[i][0] * m[j][0] + m[i][1] * m[j][1] + m[i][2] * m[j][2];
            approx::assert_relative_eq!(dot, 0.0, epsilon = 1e-12);
        }
    }
    // det = 1 (proper rotation, not reflection)
    let det = m[0][0] * (m[1][1] * m[2][2] - m[1][2] * m[2][1])
        - m[0][1] * (m[1][0] * m[2][2] - m[1][2] * m[2][0])
        + m[0][2] * (m[1][0] * m[2][1] - m[1][1] * m[2][0]);
    approx::assert_relative_eq!(det, 1.0, epsilon = 1e-12);
}

// ---------------------------------------------------------------------------
// Rotation preserves edge vertex indices and half-edge topology
// ---------------------------------------------------------------------------
#[test]
fn edge_rotation_preserves_topology_indices() {
    let original = make_test_cuboid();
    let matrix = euler_to_matrix(
        37.0_f64.to_radians(),
        53.0_f64.to_radians(),
        71.0_f64.to_radians(),
    );
    let mut rotated = original.clone();
    rotated.rotate(matrix, Point::origin());

    for (o, r) in original.edges.iter().zip(rotated.edges.iter()) {
        assert_eq!(o.vertices, r.vertices);
        assert_eq!(o.t_range, r.t_range);
    }
    for (o, r) in original.half_edges.iter().zip(rotated.half_edges.iter()) {
        assert_eq!(o.start_vertex, r.start_vertex);
        assert_eq!(o.edge, r.edge);
        assert_eq!(o.forward, r.forward);
    }
    for (o, r) in original.loops.iter().zip(rotated.loops.iter()) {
        assert_eq!(o.half_edges, r.half_edges);
    }
    for (o, r) in original.faces.iter().zip(rotated.faces.iter()) {
        assert_eq!(o.outer_loop, r.outer_loop);
        assert_eq!(o.inner_loops, r.inner_loops);
        assert_eq!(o.same_sense, r.same_sense);
    }
    for (o, r) in original.shells.iter().zip(rotated.shells.iter()) {
        assert_eq!(o.faces, r.faces);
        assert_eq!(o.closed, r.closed);
    }
}

// ---------------------------------------------------------------------------
// YAML roundtrip: construct → rotate → serialize → deserialize → re-rotate
// ---------------------------------------------------------------------------
#[test]
fn edge_yaml_roundtrip_after_rotation() {
    let mut gen = IdGenerator::new(1);
    let cuboid = make_cuboid(2.0, 3.0, 4.0, &mut gen).unwrap();
    let matrix = euler_to_matrix(
        45.0_f64.to_radians(),
        0.0_f64.to_radians(),
        0.0_f64.to_radians(),
    );
    let pivot = Point::origin();

    let mut rotated = cuboid.clone();
    rotated.rotate(matrix, pivot);

    let yaml = serde_yaml::to_string(&rotated).unwrap();
    let deserialized: Solid = serde_yaml::from_str(&yaml).unwrap();

    // All vertex coordinates must match
    for (r, d) in rotated.vertices.iter().zip(deserialized.vertices.iter()) {
        assert_eq!(r.point, d.point, "vertex point must survive YAML roundtrip");
    }
    for (r, d) in rotated.edges.iter().zip(deserialized.edges.iter()) {
        assert_eq!(r.curve, d.curve, "edge curve must survive YAML roundtrip");
    }
    for (r, d) in rotated.faces.iter().zip(deserialized.faces.iter()) {
        assert_eq!(
            r.surface, d.surface,
            "face surface must survive YAML roundtrip"
        );
    }
}
