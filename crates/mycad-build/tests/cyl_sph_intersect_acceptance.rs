// Acceptance tests for #49 Cylinder×Sphere Boolean A3: Intersect
// Geometry: Cylinder r=3, h=20, origin=(0,0,-10), axis=+Z
//           Sphere r=5, center=(0,0,0)
// Intersection circles at z=±sqrt(25-9)=±4, radius=3

use mycad_build::build_bodies_from_features;
use mycad_format::Feature;
use mycad_kernel::brep::topology::IdGenerator;
use mycad_kernel::geometry::curve::Curve;

// --- Helpers ---

fn cyl_sph_features() -> Vec<Feature> {
    vec![
        Feature::CreateCylinder {
            id: "cyl1".into(),
            radius: 3.0,
            height: 20.0,
            origin: [0.0, 0.0, -10.0],
        },
        Feature::CreateSphere {
            id: "sph1".into(),
            radius: 5.0,
            center: [0.0, 0.0, 0.0],
        },
        Feature::Intersect {
            id: "result".into(),
            target: "cyl1".into(),
            tool: "sph1".into(),
        },
    ]
}

fn build_intersect() -> mycad_kernel::brep::topology::Solid {
    let mut g = IdGenerator::new(0);
    let bodies = build_bodies_from_features(&cyl_sph_features(), &mut g)
        .expect("cyl×sph intersect build should succeed");
    bodies.get("result").unwrap().solid.clone()
}

fn check_euler(solid: &mycad_kernel::brep::topology::Solid) -> i64 {
    let v = solid.vertices.len() as i64;
    let e = solid.edges.len() as i64;
    let f = solid.faces.len() as i64;
    let l_inner = solid
        .faces
        .iter()
        .map(|face| face.inner_loops.len() as i64)
        .sum::<i64>();
    v - e + f - l_inner
}

// --- Tests ---

/// T01: Determinism — two builds with same IdGenerator seed produce identical Solids.
#[test]
fn t01_determinism() {
    let mut g1 = IdGenerator::new(0);
    let b1 = build_bodies_from_features(&cyl_sph_features(), &mut g1).unwrap();
    let s1 = &b1.get("result").unwrap().solid;

    let mut g2 = IdGenerator::new(0);
    let b2 = build_bodies_from_features(&cyl_sph_features(), &mut g2).unwrap();
    let s2 = &b2.get("result").unwrap().solid;

    assert_eq!(s1.id, s2.id, "solid id mismatch");
    assert_eq!(s1.vertices.len(), s2.vertices.len(), "vertex count");
    for (i, (va, vb)) in s1.vertices.iter().zip(s2.vertices.iter()).enumerate() {
        assert_eq!(va.id, vb.id, "vertex {i} id");
        assert!(
            (va.point - vb.point).norm() < 1e-12,
            "vertex {i} position mismatch"
        );
    }
    assert_eq!(s1.edges.len(), s2.edges.len(), "edge count");
    for (i, (ea, eb)) in s1.edges.iter().zip(s2.edges.iter()).enumerate() {
        assert_eq!(ea.id, eb.id, "edge {i} id");
    }
    assert_eq!(s1.faces.len(), s2.faces.len(), "face count");
    for (i, (fa, fb)) in s1.faces.iter().zip(s2.faces.iter()).enumerate() {
        assert_eq!(fa.id, fb.id, "face {i} id");
    }
}

/// T02: Manifold — validate_manifold passes on the intersect result.
#[test]
fn t02_manifold() {
    let solid = build_intersect();
    solid
        .validate_manifold()
        .expect("manifold validation failed");
}

/// T03: Two circle edges at z=±4 with radius=3.
#[test]
fn t03_two_circle_edges_at_z_pm4() {
    let solid = build_intersect();
    let circles: Vec<_> = solid
        .edges
        .iter()
        .filter_map(|e| match &e.curve {
            Curve::Circle { center, radius, .. } if (radius - 3.0).abs() < 1e-6 => Some(*center),
            _ => None,
        })
        .collect();

    assert!(
        circles.len() >= 2,
        "expected at least 2 circle edges with radius=3, got {}",
        circles.len()
    );
    let has_z_pos4 = circles.iter().any(|c| (c.z - 4.0).abs() < 1e-6);
    let has_z_neg4 = circles.iter().any(|c| (c.z + 4.0).abs() < 1e-6);
    assert!(has_z_pos4, "expected a circle edge at z≈+4");
    assert!(has_z_neg4, "expected a circle edge at z≈-4");
}

/// T04: Euler-Poincaré V−E+F−L_inner == 2 (tolerant: 1 or 2 allowed for seam edges).
#[test]
fn t04_euler_poincare() {
    let solid = build_intersect();
    let euler = check_euler(&solid);
    assert!(
        euler == 1 || euler == 2,
        "Euler V-E+F-L_inner should be 1 or 2 (tolerant for seam edges), got {euler}"
    );
}

/// T05: Closed shell with 3 faces (1 cyl lateral band + 2 sphere caps).
#[test]
fn t05_closed_shell_three_faces() {
    let solid = build_intersect();
    assert_eq!(
        solid.faces.len(),
        3,
        "expected 3 faces (cyl band + 2 sphere caps)"
    );
    assert_eq!(solid.shells.len(), 1, "expected 1 shell");
}

/// T06: Non-coaxial cyl×sph Intersect propagates UnsupportedSurfaceIntersection error.
#[test]
fn t06_non_coaxial_errors() {
    let features = vec![
        Feature::CreateCylinder {
            id: "cyl1".into(),
            radius: 3.0,
            height: 20.0,
            origin: [0.0, 0.0, -10.0],
        },
        Feature::CreateSphere {
            id: "sph1".into(),
            radius: 5.0,
            center: [1.0, 0.0, 0.0], // offset — non-coaxial
        },
        Feature::Intersect {
            id: "result".into(),
            target: "cyl1".into(),
            tool: "sph1".into(),
        },
    ];
    let mut g = IdGenerator::new(0);
    let result = build_bodies_from_features(&features, &mut g);
    assert!(
        result.is_err(),
        "non-coaxial cyl×sph Intersect should return an error"
    );
}

/// T07: Tangent case (sph_r == cyl_r == 3.0) — must not panic.
#[test]
fn t07_tangent_no_panic() {
    let features = vec![
        Feature::CreateCylinder {
            id: "cyl1".into(),
            radius: 3.0,
            height: 20.0,
            origin: [0.0, 0.0, -10.0],
        },
        Feature::CreateSphere {
            id: "sph1".into(),
            radius: 3.0, // same as cyl radius — tangent
            center: [0.0, 0.0, 0.0],
        },
        Feature::Intersect {
            id: "result".into(),
            target: "cyl1".into(),
            tool: "sph1".into(),
        },
    ];
    let mut g = IdGenerator::new(0);
    // Ok or Err is fine — just must not panic
    let _ = build_bodies_from_features(&features, &mut g);
}
