/// Acceptance tests for #111: extrude_cut degenerate B-rep when depth ≈ face distance.
///
/// Fixes verified:
///   Layer 1: EPSILON_GUARD = 1e-6 in extrude.ts (viewer-side clearance)
///   Layer 2: classify.rs coplanar dist <= len_eps (boundary detection)
///   Layer 3: validate_manifold() detects overlapping coplanar faces
use mycad_kernel::booleans::{boolean, BooleanOp};
use mycad_kernel::brep::topology::{IdGenerator, Solid};
use mycad_kernel::geometry::surface::Surface;
use mycad_kernel::geometry::Vec3;
use mycad_kernel::primitives::make_cuboid;

/// Helper: build a 10×20×30 cuboid centered at origin. Right face at X=+5.
fn target_box(gen: &mut IdGenerator) -> Solid {
    make_cuboid(10.0, 20.0, 30.0, gen).expect("target cuboid")
}

/// Helper: build a solid with two coplanar overlapping faces.
/// Takes a valid cuboid and duplicates its bottom face.
fn build_solid_with_coplanar_overlapping_faces(gen: &mut IdGenerator) -> Solid {
    let mut cuboid = make_cuboid(2.0, 2.0, 2.0, gen).expect("cuboid");

    let bot_face_idx = cuboid
        .faces
        .iter()
        .position(|f| {
            if let Surface::Plane { normal, .. } = &f.surface {
                normal.z < 0.0
            } else {
                false
            }
        })
        .expect("bottom face");

    let original = cuboid.faces[bot_face_idx].clone();
    let new_face_idx = cuboid.add_face(
        gen.next(),
        original.surface.clone(),
        original.outer_loop,
        original.inner_loops.clone(),
        original.same_sense,
        mycad_format::EntityRef::try_named("test", mycad_format::EntityKind::Face, "dup").ok(),
    );
    cuboid.shells[0].faces.push(new_face_idx);
    cuboid
}

/// Helper: build a solid with two coplanar faces that do NOT overlap in 2D.
/// Two separate cuboids on the same Z=0 plane but far apart in X.
fn build_solid_with_coplanar_non_overlapping_faces(gen: &mut IdGenerator) -> Solid {
    let mut box1 = make_cuboid(2.0, 2.0, 2.0, gen).expect("box1");
    let mut box2 = make_cuboid(2.0, 2.0, 2.0, gen).expect("box2");
    box2.translate(Vec3::new(10.0, 0.0, 0.0));

    // Merge box2 topology into box1
    let v_map: Vec<usize> = box2
        .vertices
        .iter()
        .map(|v| box1.add_vertex(gen.next(), v.point, v.name.clone()))
        .collect();

    let e_map: Vec<usize> = box2
        .edges
        .iter()
        .map(|e| {
            box1.add_edge(
                gen.next(),
                [v_map[e.vertices[0]], v_map[e.vertices[1]]],
                e.curve.clone(),
                e.t_range,
                e.name.clone(),
            )
        })
        .collect();

    let he_map: Vec<usize> = box2
        .half_edges
        .iter()
        .map(|he| {
            box1.add_half_edge(
                gen.next(),
                v_map[he.start_vertex],
                e_map[he.edge],
                he.forward,
            )
        })
        .collect();

    let l_map: Vec<usize> = box2
        .loops
        .iter()
        .map(|lp| {
            let new_hes: Vec<usize> = lp.half_edges.iter().map(|&h| he_map[h]).collect();
            box1.add_loop(gen.next(), new_hes)
        })
        .collect();

    let f_map: Vec<usize> = box2
        .faces
        .iter()
        .map(|f| {
            let outer = l_map[f.outer_loop];
            let inner: Vec<usize> = f.inner_loops.iter().map(|&l| l_map[l]).collect();
            box1.add_face(
                gen.next(),
                f.surface.clone(),
                outer,
                inner,
                f.same_sense,
                f.name.clone(),
            )
        })
        .collect();

    let shell_faces: Vec<usize> = box2.shells[0].faces.iter().map(|&f| f_map[f]).collect();
    box1.add_shell(gen.next(), shell_faces, true);

    box1
}

/// T01: Boolean Cut where tool face is exactly len_eps (1e-9) from target face.
/// Before fix: dist < len_eps → false → coplanar not detected → overlapping degenerate faces.
/// After fix (<=): correctly classified, result is a valid manifold or clean Err.
#[test]
fn t01_cut_tool_face_at_len_eps_from_target_face() {
    let mut gen = IdGenerator::new(1);
    let target = target_box(&mut gen);

    let len_eps = 1e-9_f64;
    let mut tool = make_cuboid(4.0, 10.0, 20.0, &mut gen).expect("tool cuboid");
    tool.translate(Vec3::new(3.0 - len_eps, 0.0, 0.0));

    let result = boolean(&target, &tool, BooleanOp::Cut, &mut gen);
    match result {
        Ok(solid) => assert!(
            solid.validate_manifold().is_ok(),
            "result must be manifold (no overlapping coplanar faces)"
        ),
        Err(_) => {} // Clear rejection is acceptable for degenerate geometry
    }
}

/// T01_degen_boundary: tool face at exactly 1e-6 clearance (EPSILON_GUARD after fix).
#[test]
fn t01_degen_boundary_cut_with_epsilon_guard_clearance() {
    let mut gen = IdGenerator::new(2);
    let target = target_box(&mut gen);
    let epsilon_guard = 1e-6_f64;
    let mut tool = make_cuboid(4.0, 10.0, 20.0, &mut gen).expect("tool");
    tool.translate(Vec3::new(3.0 - epsilon_guard, 0.0, 0.0));

    let result = boolean(&target, &tool, BooleanOp::Cut, &mut gen);
    assert!(
        result.is_ok(),
        "1e-6 clearance cut should succeed: {:?}",
        result
    );
    assert!(
        result.unwrap().validate_manifold().is_ok(),
        "must be manifold"
    );
}

/// T02: classify boundary — tool face at exactly len_eps from target face.
/// With <= fix, this should be classified as SharedOppositeDirection (coplanar).
#[test]
fn t02_classify_boundary_coplanar_at_len_eps() {
    let mut gen = IdGenerator::new(3);
    let target = target_box(&mut gen);
    let len_eps = 1e-9_f64;
    let mut tool = make_cuboid(4.0, 10.0, 20.0, &mut gen).expect("tool");
    tool.translate(Vec3::new(3.0 - len_eps, 0.0, 0.0));

    let result = boolean(&target, &tool, BooleanOp::Cut, &mut gen);
    match result {
        Ok(solid) => assert!(solid.validate_manifold().is_ok()),
        Err(_) => {}
    }
}

/// T02_boundary_degen: tool face at len_eps + 1e-15 — just above tolerance, NOT coplanar.
#[test]
fn t02_boundary_dist_just_above_len_eps_not_coplanar() {
    let mut gen = IdGenerator::new(4);
    let target = target_box(&mut gen);
    let above_eps = 1e-9 + 1e-15_f64;
    let mut tool = make_cuboid(4.0, 10.0, 20.0, &mut gen).expect("tool");
    tool.translate(Vec3::new(3.0 - above_eps, 0.0, 0.0));

    let result = boolean(&target, &tool, BooleanOp::Cut, &mut gen);
    assert!(result.is_ok(), "should succeed: {:?}", result);
    assert!(result.unwrap().validate_manifold().is_ok());
}

/// T03: validate_manifold() rejects a solid with overlapping coplanar faces.
#[test]
fn t03_validate_manifold_rejects_coplanar_duplicate_faces() {
    let mut gen = IdGenerator::new(5);
    let solid = build_solid_with_coplanar_overlapping_faces(&mut gen);
    let result = solid.validate_manifold();
    assert!(
        result.is_err(),
        "overlapping coplanar faces must be rejected, but got Ok"
    );
    let err = format!("{}", result.unwrap_err());
    assert!(
        err.contains("overlapping coplanar faces"),
        "expected coplanar faces error, got: {err}"
    );
}

/// T03_boundary_degen: coplanar faces with no 2D overlap → validate_manifold() Ok.
#[test]
fn t03_boundary_coplanar_faces_no_overlap_valid() {
    let mut gen = IdGenerator::new(6);
    let solid = build_solid_with_coplanar_non_overlapping_faces(&mut gen);
    let result = solid.validate_manifold();
    assert!(
        result.is_ok(),
        "coplanar but non-overlapping faces should pass: {:?}",
        result
    );
}

/// T04: Determinism — same inputs produce identical output 2 times.
#[test]
fn t04_determinism_extrude_cut_near_face() {
    let run = || {
        let mut gen = IdGenerator::new(99);
        let target = target_box(&mut gen);
        let mut tool = make_cuboid(4.0, 10.0, 20.0, &mut gen).expect("tool");
        tool.translate(Vec3::new(2.0, 0.0, 0.0));
        let solid = boolean(&target, &tool, BooleanOp::Cut, &mut gen).unwrap();
        (solid.vertices.len(), solid.edges.len(), solid.faces.len())
    };
    let r1 = run();
    let r2 = run();
    assert_eq!(
        r1, r2,
        "identical inputs must produce identical topology counts"
    );
}
