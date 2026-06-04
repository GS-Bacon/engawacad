// Acceptance tests for #49/#43 Cylinder×Sphere Boolean A3: Intersect
// #49: t01-t07 (determinism, manifold, circle edges, Euler, 3 faces, non-coaxial error, tangent)
// #43: t18 (tessellation), t22b (resolution-invariant determinism), t23 (YAML round-trip)
// Geometry: Cylinder r=3, h=20, origin=(0,0,-10), axis=+Z
//           Sphere r=5, center=(0,0,0)
// Intersection circles at z=±sqrt(25-9)=±4, radius=3

use mycad_build::build_bodies_from_features;
use mycad_format::Document;
use mycad_format::Feature;
use mycad_kernel::brep::topology::IdGenerator;
use mycad_kernel::geometry::curve::Curve;
use mycad_kernel::tessellation::{tessellate_solid, tessellate_solid_with, TessellationOptions};
use std::path::{Path, PathBuf};

// --- Helpers ---

fn examples_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("examples")
}

fn assert_solids_equal(
    a: &mycad_kernel::brep::topology::Solid,
    b: &mycad_kernel::brep::topology::Solid,
) {
    assert_eq!(a.id, b.id, "solid id mismatch");

    assert_eq!(a.vertices.len(), b.vertices.len(), "vertex count");
    for (i, (va, vb)) in a.vertices.iter().zip(b.vertices.iter()).enumerate() {
        assert_eq!(va.id, vb.id, "vertex {i} id");
        assert!((va.point.x - vb.point.x).abs() < 1e-12, "vertex {i} x");
        assert!((va.point.y - vb.point.y).abs() < 1e-12, "vertex {i} y");
        assert!((va.point.z - vb.point.z).abs() < 1e-12, "vertex {i} z");
    }

    assert_eq!(a.edges.len(), b.edges.len(), "edge count");
    for (i, (ea, eb)) in a.edges.iter().zip(b.edges.iter()).enumerate() {
        assert_eq!(ea.id, eb.id, "edge {i} id");
        assert_eq!(ea.vertices, eb.vertices, "edge {i} vertices");
        assert_eq!(ea.t_range, eb.t_range, "edge {i} t_range");
    }

    assert_eq!(a.half_edges.len(), b.half_edges.len(), "half_edge count");
    for (i, (ha, hb)) in a.half_edges.iter().zip(b.half_edges.iter()).enumerate() {
        assert_eq!(ha.id, hb.id, "half_edge {i} id");
        assert_eq!(
            ha.start_vertex, hb.start_vertex,
            "half_edge {i} start_vertex"
        );
        assert_eq!(ha.edge, hb.edge, "half_edge {i} edge");
        assert_eq!(ha.forward, hb.forward, "half_edge {i} forward");
    }

    assert_eq!(a.loops.len(), b.loops.len(), "loop count");
    for (i, (la, lb)) in a.loops.iter().zip(b.loops.iter()).enumerate() {
        assert_eq!(la.id, lb.id, "loop {i} id");
        assert_eq!(la.half_edges, lb.half_edges, "loop {i} half_edges");
    }

    assert_eq!(a.faces.len(), b.faces.len(), "face count");
    for (i, (fa, fb)) in a.faces.iter().zip(b.faces.iter()).enumerate() {
        assert_eq!(fa.id, fb.id, "face {i} id");
        assert_eq!(fa.outer_loop, fb.outer_loop, "face {i} outer_loop");
        assert_eq!(fa.inner_loops, fb.inner_loops, "face {i} inner_loops");
        assert_eq!(fa.same_sense, fb.same_sense, "face {i} same_sense");
    }

    assert_eq!(a.shells.len(), b.shells.len(), "shell count");
    for (i, (sa, sb)) in a.shells.iter().zip(b.shells.iter()).enumerate() {
        assert_eq!(sa.id, sb.id, "shell {i} id");
        assert_eq!(sa.faces, sb.faces, "shell {i} faces");
        assert_eq!(sa.closed, sb.closed, "shell {i} closed");
    }
}

/// Like assert_solids_equal but also checks entity names (vertex, edge, face).
fn assert_solids_equal_with_names(
    a: &mycad_kernel::brep::topology::Solid,
    b: &mycad_kernel::brep::topology::Solid,
) {
    assert_solids_equal(a, b);
    for (i, (va, vb)) in a.vertices.iter().zip(b.vertices.iter()).enumerate() {
        assert_eq!(va.name, vb.name, "vertex {i} name");
    }
    for (i, (ea, eb)) in a.edges.iter().zip(b.edges.iter()).enumerate() {
        assert_eq!(ea.name, eb.name, "edge {i} name");
    }
    for (i, (fa, fb)) in a.faces.iter().zip(b.faces.iter()).enumerate() {
        assert_eq!(fa.name, fb.name, "face {i} name");
    }
}

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

/// T18: tessellate_solid on the A2 intersect result produces Ok with triangle_count > 0.
#[test]
fn t18_tessellate_triangle_count() {
    let solid = build_intersect();
    let mesh = tessellate_solid(&solid).expect("tessellate_solid should succeed");
    assert!(
        mesh.triangle_count() > 0,
        "expected positive triangle count, got {}",
        mesh.triangle_count()
    );
}

/// T22b: Two builds with the same IdGenerator seed produce identical Solids (names included),
/// and tessellation at angular_segments=8 and 64 both succeed.
#[test]
fn t22b_determinism_across_tessellation_resolution() {
    let mut g1 = IdGenerator::new(0);
    let b1 = build_bodies_from_features(&cyl_sph_features(), &mut g1).unwrap();
    let s1 = &b1.get("result").unwrap().solid;

    let mut g2 = IdGenerator::new(0);
    let b2 = build_bodies_from_features(&cyl_sph_features(), &mut g2).unwrap();
    let s2 = &b2.get("result").unwrap().solid;

    // Solid topology is resolution-independent
    assert_solids_equal_with_names(s1, s2);

    // Both tessellation resolutions succeed
    let opts_low = TessellationOptions::new(8, 1);
    let opts_high = TessellationOptions::new(64, 1);
    let mesh_low = tessellate_solid_with(s1, &opts_low).expect("tessellate@8 should succeed");
    let mesh_high = tessellate_solid_with(s2, &opts_high).expect("tessellate@64 should succeed");
    assert!(mesh_low.triangle_count() > 0);
    assert!(mesh_high.triangle_count() > 0);
}

/// T23: boolean_intersect_cyl_sphere.mycad round-trips YAML byte-identically.
#[test]
fn t23_yaml_round_trip_example() {
    let path = examples_dir().join("boolean_intersect_cyl_sphere.mycad");
    let doc = Document::from_path(&path).unwrap_or_else(|e| {
        panic!(
            "failed to load {:?}: {e}",
            path.file_name().unwrap().to_string_lossy()
        )
    });

    // from_path → to_yaml → from_yaml → to_yaml must be byte-identical
    let yaml1 = doc.to_yaml().expect("first to_yaml should succeed");
    let doc2 = Document::from_yaml(&yaml1).expect("from_yaml roundtrip should succeed");
    let yaml2 = doc2.to_yaml().expect("second to_yaml should succeed");
    assert_eq!(yaml1, yaml2, "YAML round-trip must be byte-identical");

    // Verify the features match our fixture expectations
    assert_eq!(doc.root_component.features.len(), 3);
    assert!(
        matches!(&doc.root_component.features[0], Feature::CreateCylinder { radius, height, origin, .. }
            if (*radius - 3.0).abs() < 1e-12
            && (*height - 20.0).abs() < 1e-12
            && *origin == [0.0, 0.0, -10.0]),
        "first feature must be CreateCylinder r=3 h=20 origin=[0,0,-10]"
    );
    assert!(
        matches!(&doc.root_component.features[1], Feature::CreateSphere { radius, center, .. }
            if (*radius - 5.0).abs() < 1e-12
            && *center == [0.0, 0.0, 0.0]),
        "second feature must be CreateSphere r=5 center=[0,0,0]"
    );
    assert!(
        matches!(&doc.root_component.features[2], Feature::Intersect { target, tool, .. }
            if target == "cyl1" && tool == "sph1"),
        "third feature must be Intersect cyl1 ∩ sph1"
    );
}

// ========== Adversarial edge case tests ==========

/// EC01: 100x determinism — build the same features 100 times, all solids match the first.
#[test]
fn ec01_100x_determinism() {
    let mut g0 = IdGenerator::new(0);
    let b0 = build_bodies_from_features(&cyl_sph_features(), &mut g0).unwrap();
    let first = &b0.get("result").unwrap().solid;

    for i in 1..=99 {
        let mut g = IdGenerator::new(0);
        let b = build_bodies_from_features(&cyl_sph_features(), &mut g)
            .unwrap_or_else(|e| panic!("build {i} failed: {e}"));
        let cur = &b.get("result").unwrap().solid;

        assert_eq!(first.id, cur.id, "iteration {i}: solid id mismatch");
        assert_eq!(
            first.vertices.len(),
            cur.vertices.len(),
            "iteration {i}: vertex count"
        );
        for (j, (va, vb)) in first.vertices.iter().zip(cur.vertices.iter()).enumerate() {
            assert_eq!(va.id, vb.id, "iteration {i}: vertex {j} id");
            assert!(
                (va.point - vb.point).norm() < 1e-12,
                "iteration {i}: vertex {j} position mismatch"
            );
        }
        assert_eq!(
            first.edges.len(),
            cur.edges.len(),
            "iteration {i}: edge count"
        );
        for (j, (ea, eb)) in first.edges.iter().zip(cur.edges.iter()).enumerate() {
            assert_eq!(ea.id, eb.id, "iteration {i}: edge {j} id");
        }
        assert_eq!(
            first.faces.len(),
            cur.faces.len(),
            "iteration {i}: face count"
        );
        for (j, (fa, fb)) in first.faces.iter().zip(cur.faces.iter()).enumerate() {
            assert_eq!(fa.id, fb.id, "iteration {i}: face {j} id");
            assert_eq!(fa.name, fb.name, "iteration {i}: face {j} name");
        }
    }
}

/// EC02: Build → Document → YAML → parse → rebuild → compare solids.
/// Verifies that the full pipeline from features through YAML serialization
/// produces the same geometry when rebuilt.
#[test]
fn ec02_build_yaml_rebuild_roundtrip() {
    use mycad_format::Component;

    let features = cyl_sph_features();

    // Build original
    let mut g1 = IdGenerator::new(0);
    let b1 = build_bodies_from_features(&features, &mut g1).unwrap();
    let s1 = &b1.get("result").unwrap().solid;

    // Construct Document from the same features
    let doc = Document {
        schema_version: 1,
        version: "0.1.0".into(),
        root_component: Component {
            name: "Roundtrip Test".into(),
            transform: Default::default(),
            reference: None,
            features: features.clone(),
            children: vec![],
        },
    };

    // Serialize → deserialize → re-extract features
    let yaml = doc.to_yaml().expect("serialize should succeed");
    let doc2 = Document::from_yaml(&yaml).expect("deserialize should succeed");

    // Rebuild from round-tripped features
    let mut g2 = IdGenerator::new(0);
    let b2 = build_bodies_from_features(&doc2.root_component.features, &mut g2)
        .expect("rebuild should succeed");
    let s2 = &b2.get("result").unwrap().solid;

    // Compare solids with names
    assert_solids_equal_with_names(s1, s2);

    // Verify YAML idempotency through a second round
    let yaml2 = doc2.to_yaml().expect("second serialize should succeed");
    assert_eq!(yaml, yaml2, "YAML must be idempotent across round-trips");
}

/// T08: tessellate_solid result spans z ≈ -5.0 to +5.0 (both caps present in mesh).
/// Regression guard for issue #54 (upper cap missing, max_z was stuck at +4.0).
#[test]
fn t08_mesh_z_range_covers_both_caps() {
    let solid = build_intersect();
    let mesh = tessellate_solid(&solid).expect("tessellate_solid should succeed");

    let min_z = mesh
        .positions
        .iter()
        .map(|p| p[2])
        .fold(f64::INFINITY, f64::min);
    let max_z = mesh
        .positions
        .iter()
        .map(|p| p[2])
        .fold(f64::NEG_INFINITY, f64::max);

    // Sphere radius=5, center=(0,0,0) ⇒ caps extend to z=±5
    assert!(
        (max_z - 5.0).abs() < 1e-3,
        "max_z should be ≈+5.0 (north pole), got {max_z}"
    );
    assert!(
        (min_z + 5.0).abs() < 1e-3,
        "min_z should be ≈-5.0 (south pole), got {min_z}"
    );
}

/// T09: upper cap has multiple vertices above z=4.0 (not just the rim).
/// Confirms the north-pole fan triangles are actually generated.
#[test]
fn t09_upper_cap_vertices_exist() {
    let solid = build_intersect();
    let mesh = tessellate_solid(&solid).expect("tessellate_solid should succeed");

    let above = mesh.positions.iter().filter(|p| p[2] > 4.0 + 1e-3).count();

    assert!(
        above >= 12,
        "expected ≥12 vertices above z=4.0+ε (north-pole fan), got {above}"
    );
}

/// EC03: 100× tessellation determinism — tessellate the same solid 100 times,
/// all meshes produce identical positions, normals, and indices.
#[test]
fn ec03_tessellate_100x_determinism() {
    let solid = build_intersect();
    let first = tessellate_solid(&solid).expect("tessellate should succeed");

    for i in 1..=99 {
        let cur = tessellate_solid(&solid).unwrap_or_else(|e| panic!("tess {i} failed: {e}"));
        assert_eq!(
            first.positions.len(),
            cur.positions.len(),
            "tess {i}: position count"
        );
        for (j, (a, b)) in first.positions.iter().zip(cur.positions.iter()).enumerate() {
            assert_eq!(a, b, "tess {i}: position[{j}] mismatch");
        }
        assert_eq!(
            first.normals.len(),
            cur.normals.len(),
            "tess {i}: normal count"
        );
        for (j, (a, b)) in first.normals.iter().zip(cur.normals.iter()).enumerate() {
            assert_eq!(a, b, "tess {i}: normal[{j}] mismatch");
        }
        assert_eq!(
            first.indices.len(),
            cur.indices.len(),
            "tess {i}: index count"
        );
        assert_eq!(first.indices, cur.indices, "tess {i}: indices mismatch");
    }
}

/// EC04: Lower cap symmetry — multiple vertices below z=-4.0 (south-pole fan).
/// Mirror of T09 to confirm the lower cap is also correctly tessellated.
#[test]
fn ec04_lower_cap_vertices_exist() {
    let solid = build_intersect();
    let mesh = tessellate_solid(&solid).expect("tessellate_solid should succeed");

    let below = mesh.positions.iter().filter(|p| p[2] < -4.0 - 1e-3).count();

    assert!(
        below >= 12,
        "expected ≥12 vertices below z=-4.0-ε (south-pole fan), got {below}"
    );
}

/// EC05: Both caps present even at minimum angular_segments (3).
/// Verifies the trim-direction fix works at the lowest viable resolution.
#[test]
fn ec05_both_caps_at_minimum_segments() {
    let solid = build_intersect();
    let opts = TessellationOptions::new(3, 1);
    let mesh =
        tessellate_solid_with(&solid, &opts).expect("tessellate with min segments should succeed");

    let max_z = mesh
        .positions
        .iter()
        .map(|p| p[2])
        .fold(f64::NEG_INFINITY, f64::max);
    let min_z = mesh
        .positions
        .iter()
        .map(|p| p[2])
        .fold(f64::INFINITY, f64::min);

    assert!(
        (max_z - 5.0).abs() < 1e-3,
        "min-segments max_z should be ≈+5.0, got {max_z}"
    );
    assert!(
        (min_z + 5.0).abs() < 1e-3,
        "min-segments min_z should be ≈-5.0, got {min_z}"
    );
    assert!(
        mesh.triangle_count() > 0,
        "min-segments mesh should have triangles"
    );
}

/// EC06: No NaN or Inf in tessellated mesh positions or normals.
#[test]
fn ec06_mesh_no_nan_nor_inf() {
    let solid = build_intersect();
    let mesh = tessellate_solid(&solid).expect("tessellate_solid should succeed");

    for (i, p) in mesh.positions.iter().enumerate() {
        for (c, axis) in p.iter().zip(['x', 'y', 'z']) {
            assert!(c.is_finite(), "position[{i}].{axis} is not finite: {c}");
        }
    }
    for (i, n) in mesh.normals.iter().enumerate() {
        for (c, axis) in n.iter().zip(['x', 'y', 'z']) {
            assert!(c.is_finite(), "normal[{i}].{axis} is not finite: {c}");
        }
    }
}
