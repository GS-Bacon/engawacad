/// Acceptance tests for #111: extrude_cut degenerate B-rep when depth ≈ face distance.
///
/// Fixes verified:
///   Layer 1: EPSILON_GUARD = 1e-6 in extrude.ts (viewer-side clearance)
///   Layer 2: classify.rs coplanar dist <= len_eps (boundary detection)
///   Layer 3: validate_manifold() detects overlapping coplanar faces
use mycad_kernel::booleans::{boolean, BooleanOp};
use mycad_kernel::brep::topology::IdGenerator;
use mycad_kernel::geometry::Vec3;
use mycad_kernel::primitives::make_cuboid;

/// Helper: build a 10×20×30 cuboid centered at origin. Right face at X=+5.
fn target_box(gen: &mut IdGenerator) -> mycad_kernel::brep::topology::Solid {
    make_cuboid(10.0, 20.0, 30.0, gen).expect("target cuboid")
}

/// T01: Boolean Cut where tool face is exactly len_eps (1e-9) from target face.
/// Before fix: dist < len_eps → false → coplanar not detected → overlapping degenerate faces.
/// After fix (<=): correctly classified, result is a valid manifold or clean Err.
#[test]
#[ignore = "STEP 6 で実装後に解除"]
fn t01_cut_tool_face_at_len_eps_from_target_face() {
    let mut gen = IdGenerator::new(1);
    let mut target = target_box(&mut gen);

    // Tool: cuboid positioned so its right face is at X = 5 - 1e-9 (len_eps from target right face).
    // Tool spans X: [-(5-1e-9), 5-1e-9] → width = 2*(5-1e-9), centered at origin.
    // But we want tool's right face at X=5-1e-9 and left face inside target.
    // Use width=4 centered at X=3-1e-9: right=5-1e-9, left=1-1e-9.
    let len_eps = 1e-9_f64;
    let mut tool = make_cuboid(4.0, 10.0, 20.0, &mut gen).expect("tool cuboid");
    // translate: center of tool = (5 - len_eps) - 4/2 = 3 - len_eps
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
/// This is the normal UI-generated case after the EPSILON_GUARD fix.
#[test]
#[ignore = "STEP 6 で実装後に解除"]
fn t01_degen_boundary_cut_with_epsilon_guard_clearance() {
    let mut gen = IdGenerator::new(2);
    let target = target_box(&mut gen);
    // Tool's right face at X = 5 - 1e-6 (= face_dist - EPSILON_GUARD(new))
    let epsilon_guard = 1e-6_f64;
    let mut tool = make_cuboid(4.0, 10.0, 20.0, &mut gen).expect("tool");
    tool.translate(Vec3::new(3.0 - epsilon_guard, 0.0, 0.0));

    let result = boolean(&target, &tool, BooleanOp::Cut, &mut gen);
    assert!(result.is_ok(), "1e-6 clearance cut should succeed: {:?}", result);
    assert!(result.unwrap().validate_manifold().is_ok(), "must be manifold");
}

/// T02: classify boundary — tool face at exactly len_eps from target face.
/// With <= fix, this should be classified as SharedOppositeDirection (coplanar).
/// Net effect: correctly merged, not duplicated.
#[test]
#[ignore = "STEP 6 で実装後に解除"]
fn t02_classify_boundary_coplanar_at_len_eps() {
    let mut gen = IdGenerator::new(3);
    let target = target_box(&mut gen);
    // Tool's right face at X = 5 - 1e-9 = 5 - len_eps.
    let len_eps = 1e-9_f64;
    let mut tool = make_cuboid(4.0, 10.0, 20.0, &mut gen).expect("tool");
    tool.translate(Vec3::new(3.0 - len_eps, 0.0, 0.0));

    let result = boolean(&target, &tool, BooleanOp::Cut, &mut gen);
    // After classify.rs fix, no overlapping faces in result.
    match result {
        Ok(solid) => assert!(solid.validate_manifold().is_ok()),
        Err(_) => {}
    }
}

/// T02_boundary_degen: tool face at len_eps + 1e-15 — just above tolerance, NOT coplanar.
/// The cut should produce a clean result regardless.
#[test]
#[ignore = "STEP 6 で実装後に解除"]
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
/// GLM will implement this once the coplanar check is added to validate_manifold().
#[test]
#[ignore = "STEP 6 で実装後に解除"]
fn t03_validate_manifold_rejects_coplanar_duplicate_faces() {
    // GLM: construct a boolean Cut result that was manually perturbed to contain
    // overlapping coplanar faces, then assert validate_manifold() returns Err.
    // (Or construct such a topology directly and call validate_manifold().)
    todo!("GLM: construct degenerate solid and assert validate_manifold() is_err()")
}

/// T03_boundary_degen: coplanar faces with no 2D overlap → validate_manifold() Ok.
#[test]
#[ignore = "STEP 6 で実装後に解除"]
fn t03_boundary_coplanar_faces_no_overlap_valid() {
    // GLM: coplanar faces on same plane but disjoint in 2D → Ok.
    todo!("GLM: construct solid with non-overlapping coplanar faces → is_ok()")
}

/// T04: Determinism — same inputs produce identical output 2 times.
#[test]
#[ignore = "STEP 6 で実装後に解除"]
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
    assert_eq!(r1, r2, "identical inputs must produce identical topology counts");
}
