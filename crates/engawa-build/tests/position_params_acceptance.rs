// Acceptance tests for Issue #48: position parameters (origin / center).

use engawa_build::build_bodies_from_features;
use engawa_format::Document;
use engawa_format::Feature;
use engawa_kernel::brep::topology::IdGenerator;
use engawa_kernel::geometry::surface::Surface;
use engawa_kernel::geometry::Point;
use std::path::{Path, PathBuf};

// --- Helper: field-by-field Solid comparison for determinism tests ---

fn assert_solids_equal(
    a: &engawa_kernel::brep::topology::Solid,
    b: &engawa_kernel::brep::topology::Solid,
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
    a: &engawa_kernel::brep::topology::Solid,
    b: &engawa_kernel::brep::topology::Solid,
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

/// Compare topology structure (IDs, connectivity) and names, but NOT coordinates.
/// Used for verifying that position offset preserves role names.
fn assert_topology_and_names_equal(
    a: &engawa_kernel::brep::topology::Solid,
    b: &engawa_kernel::brep::topology::Solid,
) {
    assert_eq!(a.id, b.id, "solid id mismatch");

    assert_eq!(a.vertices.len(), b.vertices.len(), "vertex count");
    for (i, (va, vb)) in a.vertices.iter().zip(b.vertices.iter()).enumerate() {
        assert_eq!(va.id, vb.id, "vertex {i} id");
        assert_eq!(va.name, vb.name, "vertex {i} name");
    }

    assert_eq!(a.edges.len(), b.edges.len(), "edge count");
    for (i, (ea, eb)) in a.edges.iter().zip(b.edges.iter()).enumerate() {
        assert_eq!(ea.id, eb.id, "edge {i} id");
        assert_eq!(ea.name, eb.name, "edge {i} name");
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
        assert_eq!(fa.name, fb.name, "face {i} name");
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

fn examples_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("examples")
}

fn build_one(features: &[Feature]) -> engawa_kernel::brep::topology::Solid {
    let mut gen = IdGenerator::new(0);
    let bodies = build_bodies_from_features(features, &Vec::new(), &mut gen).unwrap();
    bodies.all()[0].solid.clone()
}

// --- T01: Determinism — offset cylinder built twice, all topology identical ---

#[test]
fn t01_determinism() {
    let path = examples_dir().join("cylinder_offset.engawa");
    let doc = Document::from_path(&path).unwrap();
    let mut g1 = IdGenerator::new(0);
    let mut g2 = IdGenerator::new(0);
    let b1 = build_bodies_from_features(
        &doc.root_component.features,
        &doc.root_component.ref_planes,
        &mut g1,
    )
    .unwrap();
    let b2 = build_bodies_from_features(
        &doc.root_component.features,
        &doc.root_component.ref_planes,
        &mut g2,
    )
    .unwrap();
    assert_solids_equal_with_names(&b1.all()[0].solid, &b2.all()[0].solid);
}

// --- T02: origin reflected in cylinder geometry ---

#[test]
fn t02_origin_reflected_in_cylinder() {
    let features = vec![Feature::CreateCylinder {
        id: "c".into(),
        radius: 5.0,
        height: 20.0,
        origin: [0.0, 0.0, -10.0],
        suppressed: false,
    }];
    let mut gen = IdGenerator::new(0);
    let bodies = build_bodies_from_features(&features, &Vec::new(), &mut gen).unwrap();
    let solid = &bodies.all()[0].solid;

    // Verify Surface::Cylinder.origin == Point::new(0, 0, -10)
    let cyl_face = solid
        .faces
        .iter()
        .find(|f| matches!(f.surface, Surface::Cylinder { .. }))
        .expect("no cylindrical face found");
    if let Surface::Cylinder { origin, .. } = cyl_face.surface {
        let expected = Point::new(0.0, 0.0, -10.0);
        assert!(
            (origin - expected).norm() < 1e-9,
            "cylinder origin: {origin:?} != {expected:?}"
        );
    }

    // Verify bottom vertices z == -10
    let bottom_vs: Vec<_> = solid
        .vertices
        .iter()
        .filter(|v| (v.point.z + 10.0).abs() < 1e-9)
        .collect();
    assert!(!bottom_vs.is_empty(), "no bottom vertices at z=-10 found");

    // Verify top vertices z == +10
    let top_vs: Vec<_> = solid
        .vertices
        .iter()
        .filter(|v| (v.point.z - 10.0).abs() < 1e-9)
        .collect();
    assert!(!top_vs.is_empty(), "no top vertices at z=10 found");
}

// --- T03: backward compatible — origin omitted == origin=[0,0,0] ---

#[test]
fn t03_backward_compatible_cylinder() {
    // origin=[0,0,0] should not appear in serialized YAML
    let f = Feature::CreateCylinder {
        id: "c".into(),
        radius: 3.0,
        height: 5.0,
        origin: [0.0, 0.0, 0.0],
        suppressed: false,
    };
    let yaml = serde_yaml::to_string(&f).unwrap();
    assert!(
        !yaml.contains("origin"),
        "origin:[0,0,0] should not appear in YAML: {yaml}"
    );

    // round-trip existing cylinder.engawa unchanged
    let path = examples_dir().join("cylinder.engawa");
    let doc = Document::from_path(&path).unwrap();
    let yaml1 = doc.to_yaml().unwrap();
    let doc2 = Document::from_yaml(&yaml1).unwrap();
    assert_eq!(
        yaml1,
        doc2.to_yaml().unwrap(),
        "cylinder.engawa roundtrip mismatch"
    );

    // Same for sphere.engawa
    let path = examples_dir().join("sphere.engawa");
    let doc = Document::from_path(&path).unwrap();
    let yaml1 = doc.to_yaml().unwrap();
    let doc2 = Document::from_yaml(&yaml1).unwrap();
    assert_eq!(
        yaml1,
        doc2.to_yaml().unwrap(),
        "sphere.engawa roundtrip mismatch"
    );
}

// --- T04: center reflected in sphere geometry ---

#[test]
fn t04_center_reflected_in_sphere() {
    let features = vec![Feature::CreateSphere {
        id: "s".into(),
        radius: 5.0,
        center: [2.0, 0.0, 0.0],
        suppressed: false,
    }];
    let mut gen = IdGenerator::new(0);
    let bodies = build_bodies_from_features(&features, &Vec::new(), &mut gen).unwrap();
    let solid = &bodies.all()[0].solid;

    // Verify Surface::Sphere.center == Point::new(2, 0, 0)
    let sphere_face = solid
        .faces
        .iter()
        .find(|f| matches!(f.surface, Surface::Sphere { .. }))
        .expect("no spherical face found");
    if let Surface::Sphere { center, .. } = sphere_face.surface {
        let expected = Point::new(2.0, 0.0, 0.0);
        assert!(
            (center - expected).norm() < 1e-9,
            "sphere center: {center:?} != {expected:?}"
        );
    }

    // North pole: center + (0, 0, radius) = (2, 0, 5)
    let north = solid
        .vertices
        .iter()
        .max_by(|a, b| a.point.z.partial_cmp(&b.point.z).unwrap())
        .unwrap();
    let expected_north = Point::new(2.0, 0.0, 5.0);
    assert!(
        (north.point - expected_north).norm() < 1e-9,
        "north pole: {:?} != {:?}",
        north.point,
        expected_north
    );

    // South pole: center + (0, 0, -radius) = (2, 0, -5)
    let south = solid
        .vertices
        .iter()
        .min_by(|a, b| a.point.z.partial_cmp(&b.point.z).unwrap())
        .unwrap();
    let expected_south = Point::new(2.0, 0.0, -5.0);
    assert!(
        (south.point - expected_south).norm() < 1e-9,
        "south pole: {:?} != {:?}",
        south.point,
        expected_south
    );
}

// --- T05: derived names invariant — offset does not change role names ---

#[test]
fn t05_derived_names_invariant() {
    // Cylinder: origin vs offset
    let f_origin = vec![Feature::CreateCylinder {
        id: "c".into(),
        radius: 3.0,
        height: 5.0,
        origin: [0.0, 0.0, 0.0],
        suppressed: false,
    }];
    let f_offset = vec![Feature::CreateCylinder {
        id: "c".into(),
        radius: 3.0,
        height: 5.0,
        origin: [1.0, 2.0, -3.0],
        suppressed: false,
    }];
    let s1 = build_one(&f_origin);
    let s2 = build_one(&f_offset);
    assert_topology_and_names_equal(&s1, &s2);

    // Sphere: center vs offset
    let f_origin_s = vec![Feature::CreateSphere {
        id: "s".into(),
        radius: 5.0,
        center: [0.0, 0.0, 0.0],
        suppressed: false,
    }];
    let f_offset_s = vec![Feature::CreateSphere {
        id: "s".into(),
        radius: 5.0,
        center: [10.0, -20.0, 30.0],
        suppressed: false,
    }];
    let s3 = build_one(&f_origin_s);
    let s4 = build_one(&f_offset_s);
    assert_topology_and_names_equal(&s3, &s4);
}

// --- T06: YAML roundtrip for offset examples ---

#[test]
fn t06_yaml_roundtrip_offset_examples() {
    for name in &["cylinder_offset.engawa", "sphere_offset.engawa"] {
        let path = examples_dir().join(name);
        let doc = Document::from_path(&path).unwrap();
        let yaml1 = doc.to_yaml().unwrap();
        let doc2 = Document::from_yaml(&yaml1).unwrap();
        assert_eq!(
            yaml1,
            doc2.to_yaml().unwrap(),
            "roundtrip mismatch for {name}"
        );
    }
}

// ==========================================================================
// Edge-case / boundary / adversarial tests (adversarial persona)
// ==========================================================================

// --- EC01: Large finite coordinates build successfully ---

#[test]
fn ec01_large_finite_origin_cylinder() {
    let features = vec![Feature::CreateCylinder {
        id: "c".into(),
        radius: 1.0,
        height: 2.0,
        origin: [1e15, -1e15, 0.0],
        suppressed: false,
    }];
    let mut gen = IdGenerator::new(0);
    let bodies = build_bodies_from_features(&features, &Vec::new(), &mut gen).unwrap();
    let solid = &bodies.all()[0].solid;

    // Surface::Cylinder.origin should reflect the large coordinates
    let cyl_face = solid
        .faces
        .iter()
        .find(|f| matches!(f.surface, Surface::Cylinder { .. }))
        .unwrap();
    if let Surface::Cylinder { origin, .. } = cyl_face.surface {
        assert!(
            (origin - Point::new(1e15, -1e15, 0.0)).norm() < 1.0,
            "large cylinder origin mismatch: {origin:?}"
        );
    }
}

#[test]
fn ec02_large_finite_center_sphere() {
    let features = vec![Feature::CreateSphere {
        id: "s".into(),
        radius: 1.0,
        center: [-1e15, 1e15, 0.0],
        suppressed: false,
    }];
    let mut gen = IdGenerator::new(0);
    let bodies = build_bodies_from_features(&features, &Vec::new(), &mut gen).unwrap();
    let solid = &bodies.all()[0].solid;

    let sphere_face = solid
        .faces
        .iter()
        .find(|f| matches!(f.surface, Surface::Sphere { .. }))
        .unwrap();
    if let Surface::Sphere { center, .. } = sphere_face.surface {
        assert!(
            (center - Point::new(-1e15, 1e15, 0.0)).norm() < 1.0,
            "large sphere center mismatch: {center:?}"
        );
    }
}

// --- EC03: Negative coordinates produce correct surface origin ---

#[test]
fn ec03_negative_origin_cylinder() {
    let features = vec![Feature::CreateCylinder {
        id: "c".into(),
        radius: 5.0,
        height: 10.0,
        origin: [-100.0, -50.0, -200.0],
        suppressed: false,
    }];
    let mut gen = IdGenerator::new(0);
    let bodies = build_bodies_from_features(&features, &Vec::new(), &mut gen).unwrap();
    let solid = &bodies.all()[0].solid;

    let cyl_face = solid
        .faces
        .iter()
        .find(|f| matches!(f.surface, Surface::Cylinder { .. }))
        .unwrap();
    if let Surface::Cylinder { origin, .. } = cyl_face.surface {
        let expected = Point::new(-100.0, -50.0, -200.0);
        assert!(
            (origin - expected).norm() < 1e-9,
            "negative origin: {origin:?} != {expected:?}"
        );
    }

    // Bottom cap plane origin should also match
    let bottom_face = solid
        .faces
        .iter()
        .find(|f| matches!(f.surface, Surface::Plane { .. }))
        .unwrap();
    if let Surface::Plane { origin, .. } = bottom_face.surface {
        assert!(
            (origin - Point::new(-100.0, -50.0, -200.0)).norm() < 1e-9,
            "bottom cap origin: {origin:?}"
        );
    }
}

// --- EC04: Zero-coords mixed (center not at origin, center field appears in YAML) ---

#[test]
fn ec04_mixed_zero_center_appears_in_yaml() {
    let f = Feature::CreateSphere {
        id: "s".into(),
        radius: 5.0,
        center: [0.0, 0.0, 5.0],
        suppressed: false,
    };
    let yaml = serde_yaml::to_string(&f).unwrap();
    assert!(
        yaml.contains("center"),
        "center:[0,0,5] must appear in YAML: {yaml}"
    );

    let f_cyl = Feature::CreateCylinder {
        id: "c".into(),
        radius: 3.0,
        height: 5.0,
        origin: [0.0, 0.0, 1.0],
        suppressed: false,
    };
    let yaml_cyl = serde_yaml::to_string(&f_cyl).unwrap();
    assert!(
        yaml_cyl.contains("origin"),
        "origin:[0,0,1] must appear in YAML: {yaml_cyl}"
    );
}

// --- EC05: Determinism — 100 runs produce identical results ---

#[test]
fn ec05_determinism_100_runs_cylinder_offset() {
    let features = vec![Feature::CreateCylinder {
        id: "c".into(),
        radius: 5.0,
        height: 20.0,
        origin: [3.0, -7.0, 11.0],
        suppressed: false,
    }];

    let mut first: Option<engawa_kernel::brep::topology::Solid> = None;
    for _run in 0..100 {
        let mut gen = IdGenerator::new(0);
        let bodies = build_bodies_from_features(&features, &Vec::new(), &mut gen).unwrap();
        let solid = bodies.all()[0].solid.clone();
        match &first {
            None => first = Some(solid),
            Some(ref expected) => {
                assert_solids_equal_with_names(expected, &solid);
            }
        }
    }
}

#[test]
fn ec06_determinism_100_runs_sphere_offset() {
    let features = vec![Feature::CreateSphere {
        id: "s".into(),
        radius: 5.0,
        center: [4.0, -3.0, 2.0],
        suppressed: false,
    }];

    let mut first: Option<engawa_kernel::brep::topology::Solid> = None;
    for _run in 0..100 {
        let mut gen = IdGenerator::new(0);
        let bodies = build_bodies_from_features(&features, &Vec::new(), &mut gen).unwrap();
        let solid = bodies.all()[0].solid.clone();
        match &first {
            None => first = Some(solid),
            Some(ref expected) => {
                assert_solids_equal_with_names(expected, &solid);
            }
        }
    }
}

// --- EC07: Roundtrip — construct → YAML serialize → deserialize → reconstruct ---

#[test]
fn ec07_roundtrip_cylinder_offset() {
    let f = Feature::CreateCylinder {
        id: "cyl_1".into(),
        radius: 5.0,
        height: 20.0,
        origin: [0.0, 0.0, -10.0],
        suppressed: false,
    };
    let yaml = serde_yaml::to_string(&f).unwrap();
    let back: Feature = serde_yaml::from_str(&yaml).unwrap();
    match back {
        Feature::CreateCylinder {
            id,
            radius,
            height,
            origin,
            ..
        } => {
            assert_eq!(id, "cyl_1");
            assert!((radius - 5.0).abs() < 1e-12);
            assert!((height - 20.0).abs() < 1e-12);
            assert_eq!(origin, [0.0, 0.0, -10.0]);
        }
        other => panic!("expected CreateCylinder, got {other:?}"),
    }
}

#[test]
fn ec08_roundtrip_sphere_offset() {
    let f = Feature::CreateSphere {
        id: "sphere_1".into(),
        radius: 5.0,
        center: [2.0, 0.0, 0.0],
        suppressed: false,
    };
    let yaml = serde_yaml::to_string(&f).unwrap();
    let back: Feature = serde_yaml::from_str(&yaml).unwrap();
    match back {
        Feature::CreateSphere {
            id, radius, center, ..
        } => {
            assert_eq!(id, "sphere_1");
            assert!((radius - 5.0).abs() < 1e-12);
            assert_eq!(center, [2.0, 0.0, 0.0]);
        }
        other => panic!("expected CreateSphere, got {other:?}"),
    }
}
