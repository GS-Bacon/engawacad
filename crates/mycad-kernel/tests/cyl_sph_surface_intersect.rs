// Integration tests for Issue #43: Cylinder×Sphere surface-intersection geometry (T34).
// Tests `intersect_surfaces` directly at the kernel level.
// Fixture: Cylinder r=3, axis=(0,0,1), origin=(0,0,-10)
//          Sphere r=5, center=(0,0,0)
// Expected: 2 loops, centers at z=±4, normal=(0,0,1), t_range=[0, 2π]

use mycad_kernel::geometry::curve::Curve;
use mycad_kernel::geometry::math::LENGTH_TOLERANCE;
use mycad_kernel::geometry::surface::Surface;
use mycad_kernel::geometry::surface_intersect::intersect_surfaces;
use mycad_kernel::geometry::{Point, Vec3};

/// T34: intersect_surfaces(cyl, sph) returns 2 loops sorted by ascending z,
/// each loop is a full circle with normal≈(0,0,1), t_range≈[0, 2π].
#[test]
fn t34_intersect_surfaces_loop_geometry() {
    let cyl = Surface::Cylinder {
        origin: Point::new(0.0, 0.0, -10.0),
        axis: Vec3::z(),
        radius: 3.0,
    };
    let sph = Surface::Sphere {
        center: Point::origin(),
        radius: 5.0,
    };

    let loops = intersect_surfaces(&cyl, &sph).expect("cyl×sph intersect should succeed");

    assert_eq!(loops.len(), 2, "expected 2 intersection loops");

    let two_pi = 2.0 * std::f64::consts::PI;

    for (i, il) in loops.iter().enumerate() {
        let Curve::Circle {
            center,
            normal,
            radius,
        } = &il.curve_3d
        else {
            panic!("loop {i}: expected Curve::Circle");
        };

        // Radius must be 3.0 (cylinder radius)
        assert!(
            (radius - 3.0).abs() < LENGTH_TOLERANCE,
            "loop {i}: radius {radius}, expected 3.0"
        );

        // Normal must be (0, 0, 1)
        assert!(
            (normal.x - 0.0).abs() < LENGTH_TOLERANCE
                && (normal.y - 0.0).abs() < LENGTH_TOLERANCE
                && (normal.z - 1.0).abs() < LENGTH_TOLERANCE,
            "loop {i}: normal {:?}, expected (0, 0, 1)",
            normal
        );

        // t_range must be [0, 2π]
        assert!(
            (il.t_range[0] - 0.0).abs() < LENGTH_TOLERANCE,
            "loop {i}: t_range[0] = {}, expected 0",
            il.t_range[0]
        );
        assert!(
            (il.t_range[1] - two_pi).abs() < LENGTH_TOLERANCE,
            "loop {i}: t_range[1] = {}, expected 2π ≈ {two_pi}",
            il.t_range[1]
        );
    }

    // Centers sorted by ascending z: z ≈ -4 first, z ≈ +4 second
    let z0 = match &loops[0].curve_3d {
        Curve::Circle { center, .. } => center.z,
        _ => unreachable!(),
    };
    let z1 = match &loops[1].curve_3d {
        Curve::Circle { center, .. } => center.z,
        _ => unreachable!(),
    };

    assert!(
        (z0 - (-4.0)).abs() < LENGTH_TOLERANCE,
        "loops[0].center.z = {z0}, expected -4.0"
    );
    assert!(
        (z1 - 4.0).abs() < LENGTH_TOLERANCE,
        "loops[1].center.z = {z1}, expected +4.0"
    );
    assert!(z0 < z1, "loops must be sorted by ascending z: {z0} < {z1}");
}

// ========== Adversarial edge case tests (integration level) ==========

/// EC01: No intersection when sphere is smaller than cylinder (r_sq < 0).
#[test]
fn ec01_no_intersection_sphere_smaller() {
    let cyl = Surface::Cylinder {
        origin: Point::origin(),
        axis: Vec3::z(),
        radius: 5.0,
    };
    let sph = Surface::Sphere {
        center: Point::origin(),
        radius: 3.0, // sphere smaller than cylinder → no intersection
    };
    let loops = intersect_surfaces(&cyl, &sph).expect("should return Ok, not error");
    assert!(loops.is_empty(), "expected 0 loops for disjoint cyl/sph");
}

/// EC02: Tangent case — sphere radius == cylinder radius (r_sq ≈ 0).
#[test]
fn ec02_tangent_equal_radii() {
    let cyl = Surface::Cylinder {
        origin: Point::origin(),
        axis: Vec3::z(),
        radius: 3.0,
    };
    let sph = Surface::Sphere {
        center: Point::origin(),
        radius: 3.0, // r_sq = 9 - 9 = 0 → tangent
    };
    let loops = intersect_surfaces(&cyl, &sph).expect("tangent should return Ok");
    assert!(loops.is_empty(), "expected 0 loops for tangent case");
}

/// EC03: 100x determinism for intersect_surfaces — same input, same output every time.
#[test]
fn ec03_100x_determinism() {
    let cyl = Surface::Cylinder {
        origin: Point::new(0.0, 0.0, -10.0),
        axis: Vec3::z(),
        radius: 3.0,
    };
    let sph = Surface::Sphere {
        center: Point::origin(),
        radius: 5.0,
    };

    let first = intersect_surfaces(&cyl, &sph).expect("first call should succeed");

    for i in 1..=99 {
        let cur = intersect_surfaces(&cyl, &sph).unwrap_or_else(|e| panic!("run {i}: {e}"));
        assert_eq!(cur.len(), first.len(), "run {i}: loop count mismatch");
        for (j, (a, b)) in cur.iter().zip(first.iter()).enumerate() {
            assert_eq!(
                a.curve_3d, b.curve_3d,
                "run {i}: loop {j} curve_3d mismatch"
            );
            assert_eq!(a.t_range, b.t_range, "run {i}: loop {j} t_range mismatch");
        }
    }
}

/// EC04: IntersectionLoop serde round-trip — serialize → deserialize → compare.
#[test]
fn ec04_intersection_loop_serde_roundtrip() {
    let cyl = Surface::Cylinder {
        origin: Point::new(0.0, 0.0, -10.0),
        axis: Vec3::z(),
        radius: 3.0,
    };
    let sph = Surface::Sphere {
        center: Point::origin(),
        radius: 5.0,
    };
    let loops = intersect_surfaces(&cyl, &sph).unwrap();

    for il in &loops {
        let yaml = serde_yaml::to_string(il).expect("serialize should succeed");
        let restored: mycad_kernel::geometry::surface_intersect::IntersectionLoop =
            serde_yaml::from_str(&yaml).expect("deserialize should succeed");
        assert_eq!(
            il, &restored,
            "IntersectionLoop must survive serde round-trip"
        );
    }
}

/// EC05: Non-coaxial cylinder×sphere at surface level → UnsupportedSurfaceIntersection.
#[test]
fn ec05_non_coaxial_error() {
    let cyl = Surface::Cylinder {
        origin: Point::origin(),
        axis: Vec3::z(),
        radius: 3.0,
    };
    let sph = Surface::Sphere {
        center: Point::new(0.5, 0.0, 0.0), // offset — not on axis
        radius: 5.0,
    };
    let result = intersect_surfaces(&cyl, &sph);
    assert!(result.is_err(), "non-coaxial should return an error");
}

/// EC06: Cylinder larger than sphere → r_sq < 0 → empty.
#[test]
fn ec06_cylinder_larger_than_sphere() {
    let cyl = Surface::Cylinder {
        origin: Point::origin(),
        axis: Vec3::z(),
        radius: 6.0,
    };
    let sph = Surface::Sphere {
        center: Point::origin(),
        radius: 5.0, // r_sq = 25 - 36 = -11
    };
    let loops = intersect_surfaces(&cyl, &sph).expect("should return Ok");
    assert!(loops.is_empty(), "expected 0 loops when cyl > sph");
}
