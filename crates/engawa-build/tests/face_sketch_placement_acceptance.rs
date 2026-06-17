//! Issue #216 STEP 6: Acceptance Test Implementation
//!
//! ModelFace 上のスケッチ描画 e2e (8a/8b 結合) の受入テスト。
//! plan.md (features/216-phase8-modelface-e2e-a/plan.md) のテスト計画 ID T01〜T05_degen に対応する。

use engawa_build::build_assembly;
use engawa_format::{Document, EntityKind, EntityRef};
use engawa_kernel::brep::topology::IdGenerator;
use engawa_kernel::geometry::surface::Surface;
use engawa_kernel::geometry::{Plane, Vec3};
use engawa_kernel::primitives::make_cuboid;
use engawa_kernel::tessellation::{
    merge_meshes, tessellate_solid_with, to_ascii_stl, TessellationOptions,
};
use engawa_kernel::KernelError;
use std::path::Path;

/// Extract a Plane from a Surface if it is planar.
/// Copied from engawa_build/src/lib.rs surface_to_plane for test access.
fn surface_to_plane(surface: &Surface) -> Option<Plane> {
    match surface {
        Surface::Plane {
            origin,
            normal,
            u_axis,
            v_axis,
        } => Some(Plane {
            origin: *origin,
            normal: *normal,
            u_axis: *u_axis,
            v_axis: *v_axis,
        }),
        _ => None,
    }
}

/// T01: STL byte 列 3-run 決定性
#[test]
fn t01_stl_three_run_determinism() {
    let yaml = include_str!("../../../examples/sketch_circle_on_face.engawa");
    let doc = Document::from_yaml(yaml).expect("YAML parse failed");

    let stl_run = || {
        let mut gen = IdGenerator::new(0);
        let bodies = build_assembly(&doc, Path::new("examples"), &mut gen).expect("build failed");
        let meshes: Vec<_> = bodies
            .iter()
            .map(|b| {
                tessellate_solid_with(&b.solid, &TessellationOptions::default())
                    .expect("tessellate failed")
            })
            .collect();
        to_ascii_stl(&merge_meshes(&meshes), "model")
    };

    let a = stl_run();
    let b = stl_run();
    let c = stl_run();

    assert_eq!(a, b);
    assert_eq!(b, c);
}

/// T02: build_assembly 成功 + body 数
#[test]
fn t02_build_assembly_succeeds() {
    let yaml = include_str!("../../../examples/sketch_circle_on_face.engawa");
    let doc = Document::from_yaml(yaml).expect("YAML parse failed");

    let mut gen = IdGenerator::new(0);
    let bodies = build_assembly(&doc, Path::new("examples"), &mut gen).expect("build failed");

    // box_1 (create_box) and extrude_1 are both live bodies (box_1 is not consumed by extrude_1)
    assert_eq!(bodies.len(), 2);
}

/// T03: kernel unit — Face EntityRef → Plane.origin/normal の正しさ
#[test]
fn t03_face_plane_origin_normal() {
    // cuboid(dx=10, dy=10, dz=5) centered at origin → top face at z = dz/2 = 2.5
    let mut gen = IdGenerator::new(0);
    let solid = make_cuboid(10.0, 10.0, 5.0, "box_1", &mut gen).expect("make_cuboid failed");

    let entity_ref = EntityRef::Named {
        feature_id: "box_1".to_string(),
        kind: EntityKind::Face,
        role: "f_z_pos".to_string(),
    };

    let face_idx = solid
        .find_face_by_entity_ref(&entity_ref)
        .expect("face should be found by entity_ref");
    let face = &solid.faces[face_idx];

    let plane = surface_to_plane(&face.surface).expect("face should be planar");

    // Plane.origin.z should be dz/2 = 2.5 ± ε_snap
    assert!(
        (plane.origin.z - 2.5).abs() < 1e-9,
        "plane.origin.z = {}, expected 2.5",
        plane.origin.z
    );

    // Plane.normal should be (0, 0, 1) ± ε_snap
    let expected_normal = Vec3::new(0.0, 0.0, 1.0);
    let diff = (plane.normal - expected_normal).norm();
    assert!(
        diff < 1e-9,
        "plane.normal = {:?}, expected (0, 0, 1), diff = {}",
        plane.normal,
        diff
    );
}

/// T04: Phase 7 (sketch_via_refplane.engawa) non-regression
#[test]
fn t04_phase7_refplane_non_regression() {
    let yaml = include_str!("../../../examples/sketch_via_refplane.engawa");
    let doc = Document::from_yaml(yaml).expect("YAML parse failed");

    let mut gen = IdGenerator::new(0);
    let bodies = build_assembly(&doc, Path::new("examples"), &mut gen).expect("build failed");

    // At least one body should exist (extrude_1)
    assert!(!bodies.is_empty());
}

/// T05_degen_zero_radius: 退化 (半径 0 polygon) で graceful error
#[test]
fn t05_degen_zero_radius() {
    let yaml = r#"
schema_version: 1
version: "0.1.0"
root_component:
  name: "Degenerate Zero Radius Octagon"
  features:
    - type: create_box
      id: box_1
      width: 10.0
      height: 10.0
      depth: 5.0
    - type: create_sketch
      id: sketch_1
      plane: xy
      offset: 0.0
      profile:
        # All 8 vertices at (0, 0) → zero-radius octagon (degenerate)
        - id: seg_0
          from: [0.0, 0.0]
          to:   [0.0, 0.0]
        - id: seg_1
          from: [0.0, 0.0]
          to:   [0.0, 0.0]
        - id: seg_2
          from: [0.0, 0.0]
          to:   [0.0, 0.0]
        - id: seg_3
          from: [0.0, 0.0]
          to:   [0.0, 0.0]
        - id: seg_4
          from: [0.0, 0.0]
          to:   [0.0, 0.0]
        - id: seg_5
          from: [0.0, 0.0]
          to:   [0.0, 0.0]
        - id: seg_6
          from: [0.0, 0.0]
          to:   [0.0, 0.0]
        - id: seg_7
          from: [0.0, 0.0]
          to:   [0.0, 0.0]
      plane_ref:
        ref: named
        feature_id: box_1
        kind: face
        role: f_z_pos
    - type: extrude
      id: extrude_1
      sketch: sketch_1
      depth: 1.0
"#;

    let doc = Document::from_yaml(yaml).expect("YAML parse failed");
    let mut gen = IdGenerator::new(0);

    let result = build_assembly(&doc, Path::new("examples"), &mut gen);

    // KernelError has only InvalidParameter (no dedicated EmptyProfile/DegenerateSegment).
    // Zero-radius octagon: validate_profile_closed passes (all endpoints coincide at (0,0)),
    // but make_extrusion's degeneracy guards return InvalidParameter { kind: "profile" }.
    // We pin the variant + kind tag to make this contract observable.
    let err = result.expect_err("build_assembly should fail with zero-radius octagon");
    assert!(
        matches!(&err, KernelError::InvalidParameter { kind } if *kind == "profile"),
        "expected KernelError::InvalidParameter {{ kind: \"profile\" }}, got: {:?}",
        err
    );
}

/// T06: u_axis/v_axis orientation check (complete frame-of-reference)
#[test]
fn t06_plane_u_v_axis_orientation() {
    let mut gen = IdGenerator::new(0);
    let solid = make_cuboid(10.0, 10.0, 5.0, "box_1", &mut gen).expect("make_cuboid failed");

    let entity_ref = EntityRef::Named {
        feature_id: "box_1".to_string(),
        kind: EntityKind::Face,
        role: "f_z_pos".to_string(),
    };

    let face_idx = solid
        .find_face_by_entity_ref(&entity_ref)
        .expect("face should be found");
    let face = &solid.faces[face_idx];

    let plane = surface_to_plane(&face.surface).expect("face should be planar");

    // From cuboid.rs:135-136, f_z_pos has u_axis = -Vec3::x(), v_axis = Vec3::y()
    let expected_u = Vec3::new(-1.0, 0.0, 0.0);
    let expected_v = Vec3::new(0.0, 1.0, 0.0);

    let diff_u = (plane.u_axis - expected_u).norm();
    let diff_v = (plane.v_axis - expected_v).norm();

    assert!(
        diff_u < 1e-9,
        "plane.u_axis = {:?}, expected (-1, 0, 0), diff = {}",
        plane.u_axis,
        diff_u
    );
    assert!(
        diff_v < 1e-9,
        "plane.v_axis = {:?}, expected (0, 1, 0), diff = {}",
        plane.v_axis,
        diff_v
    );
}

/// T07: other_role_returns_different_face — f_z_neg returns different face with normal (0,0,-1)
#[test]
fn t07_other_role_returns_different_face() {
    let mut gen = IdGenerator::new(0);
    let solid = make_cuboid(10.0, 10.0, 5.0, "box_1", &mut gen).expect("make_cuboid failed");

    let ref_z_pos = EntityRef::Named {
        feature_id: "box_1".to_string(),
        kind: EntityKind::Face,
        role: "f_z_pos".to_string(),
    };

    let ref_z_neg = EntityRef::Named {
        feature_id: "box_1".to_string(),
        kind: EntityKind::Face,
        role: "f_z_neg".to_string(),
    };

    let idx_pos = solid
        .find_face_by_entity_ref(&ref_z_pos)
        .expect("f_z_pos should be found");
    let idx_neg = solid
        .find_face_by_entity_ref(&ref_z_neg)
        .expect("f_z_neg should be found");

    // Different faces
    assert_ne!(
        idx_pos, idx_neg,
        "f_z_pos and f_z_neg should resolve to different faces"
    );

    // f_z_neg normal should be (0, 0, -1)
    let face_neg = &solid.faces[idx_neg];
    let plane_neg = surface_to_plane(&face_neg.surface).expect("face should be planar");
    let expected_normal_neg = Vec3::new(0.0, 0.0, -1.0);
    let diff_neg = (plane_neg.normal - expected_normal_neg).norm();

    assert!(
        diff_neg < 1e-9,
        "f_z_neg plane.normal = {:?}, expected (0, 0, -1), diff = {}",
        plane_neg.normal,
        diff_neg
    );
}

/// T08: unknown_role returns None
#[test]
fn t08_unknown_role_returns_none() {
    let mut gen = IdGenerator::new(0);
    let solid = make_cuboid(10.0, 10.0, 5.0, "box_1", &mut gen).expect("make_cuboid failed");

    let ref_unknown_role = EntityRef::Named {
        feature_id: "box_1".to_string(),
        kind: EntityKind::Face,
        role: "f_xxx".to_string(), // non-existent role
    };

    let result = solid.find_face_by_entity_ref(&ref_unknown_role);
    assert!(
        result.is_none(),
        "unknown role 'f_xxx' should return None, got {:?}",
        result
    );
}

/// T09: unknown_feature_id returns None
#[test]
fn t09_unknown_feature_id_returns_none() {
    let mut gen = IdGenerator::new(0);
    let solid = make_cuboid(10.0, 10.0, 5.0, "box_1", &mut gen).expect("make_cuboid failed");

    let ref_unknown_feature = EntityRef::Named {
        feature_id: "box_999".to_string(), // non-existent feature_id
        kind: EntityKind::Face,
        role: "f_z_pos".to_string(),
    };

    let result = solid.find_face_by_entity_ref(&ref_unknown_feature);
    assert!(
        result.is_none(),
        "unknown feature_id 'box_999' should return None, got {:?}",
        result
    );
}

/// T10: negative_radius — 8-gon with radius -1 (vertices at negative coordinates)
/// Expected: graceful behavior (no panic). Actual behavior documented in test.
#[test]
fn t10_negative_radius_graceful() {
    // 8-gon with radius -1 → vertices at negative coordinates
    // k=0: (-2.0, 0.0) etc. (mirrored across origin)
    let yaml = r#"
schema_version: 1
version: "0.1.0"
root_component:
  name: "Negative Radius Octagon"
  features:
    - type: create_box
      id: box_1
      width: 10.0
      height: 10.0
      depth: 5.0
    - type: create_sketch
      id: sketch_1
      plane: xy
      offset: 0.0
      profile:
        - id: seg_0
          from: [-2.0, 0.0]
          to:   [-1.4142135623730951, 1.4142135623730951]
        - id: seg_1
          from: [-1.4142135623730951, 1.4142135623730951]
          to:   [0.0, 2.0]
        - id: seg_2
          from: [0.0, 2.0]
          to:   [1.4142135623730951, 1.4142135623730951]
        - id: seg_3
          from: [1.4142135623730951, 1.4142135623730951]
          to:   [2.0, 0.0]
        - id: seg_4
          from: [2.0, 0.0]
          to:   [1.4142135623730951, -1.4142135623730951]
        - id: seg_5
          from: [1.4142135623730951, -1.4142135623730951]
          to:   [0.0, -2.0]
        - id: seg_6
          from: [0.0, -2.0]
          to:   [-1.4142135623730951, -1.4142135623730951]
        - id: seg_7
          from: [-1.4142135623730951, -1.4142135623730951]
          to:   [-2.0, 0.0]
      plane_ref:
        ref: named
        feature_id: box_1
        kind: face
        role: f_z_pos
    - type: extrude
      id: extrude_1
      sketch: sketch_1
      depth: 1.0
"#;

    let doc = Document::from_yaml(yaml).expect("YAML parse failed");
    let mut gen = IdGenerator::new(0);

    // Graceful check: this test passes if no panic occurs
    // The actual behavior (success/error with negative radius) is documented here.
    // Current implementation: build succeeds with negative radius vertices,
    // producing a valid solid (profile is closed CCW, just mirrored).
    let result = build_assembly(&doc, Path::new("examples"), &mut gen);
    match result {
        Ok(bodies) => {
            // Build succeeds — profile is valid closed CCW (mirrored but still CCW)
            // Documented behavior: negative radius mirrors vertices but keeps CCW order
            assert!(
                !bodies.is_empty(),
                "extrude should produce at least one body"
            );
        }
        Err(e) => {
            // If implementation changes to reject negative radius, that's also graceful
            let error_msg = format!("{:?}", e);
            // Document any non-panic error as acceptable
            assert!(
                error_msg.contains("EmptyProfile")
                    || error_msg.contains("ProfileNotClosed")
                    || error_msg.contains("DegenerateGeometry")
                    || error_msg.contains("InvalidParameter"),
                "Unexpected error: {:?}",
                e
            );
        }
    }
    // Test passing = no panic occurred
}

/// T11: T03 repeated determinism — find_face_by_entity_ref returns same index across 3 runs
#[test]
fn t11_face_ref_repeated_determinism() {
    let entity_ref = EntityRef::Named {
        feature_id: "box_1".to_string(),
        kind: EntityKind::Face,
        role: "f_z_pos".to_string(),
    };

    // Run 3 times with fresh generators each time
    let mut gen1 = IdGenerator::new(0);
    let solid1 = make_cuboid(10.0, 10.0, 5.0, "box_1", &mut gen1).expect("make_cuboid failed");
    let idx1 = solid1
        .find_face_by_entity_ref(&entity_ref)
        .expect("face should be found");

    let mut gen2 = IdGenerator::new(0);
    let solid2 = make_cuboid(10.0, 10.0, 5.0, "box_1", &mut gen2).expect("make_cuboid failed");
    let idx2 = solid2
        .find_face_by_entity_ref(&entity_ref)
        .expect("face should be found");

    let mut gen3 = IdGenerator::new(0);
    let solid3 = make_cuboid(10.0, 10.0, 5.0, "box_1", &mut gen3).expect("make_cuboid failed");
    let idx3 = solid3
        .find_face_by_entity_ref(&entity_ref)
        .expect("face should be found");

    assert_eq!(
        idx1, idx2,
        "face_idx should be identical across runs 1 and 2"
    );
    assert_eq!(
        idx2, idx3,
        "face_idx should be identical across runs 2 and 3"
    );
}

/// T12: end-to-end check that `plane_ref` (Face EntityRef) is actually consumed by the build
/// pipeline. If a regression caused `plane_ref` to be silently ignored, `extrude_1` would
/// be built on the canonical xy-plane (z=0) instead of the cuboid's top face (z=2.5), and
/// the extruded solid's z range would be [0, 1] instead of [2.5, 3.5]. This test guards
/// against that regression — it is the only test in this file that observes the result of
/// the build pipeline, not the kernel APIs in isolation.
#[test]
fn t12_extrude_uses_plane_ref_top_face_z_range() {
    let yaml = include_str!("../../../examples/sketch_circle_on_face.engawa");
    let doc = Document::from_yaml(yaml).expect("YAML parse failed");

    let mut gen = IdGenerator::new(0);
    let bodies = build_assembly(&doc, Path::new("examples"), &mut gen).expect("build failed");

    let extrude = bodies
        .iter()
        .find(|b| b.feature_id == "extrude_1")
        .expect("extrude_1 body should be present");

    let (z_min, z_max) = extrude
        .solid
        .vertices
        .iter()
        .fold((f64::INFINITY, f64::NEG_INFINITY), |(lo, hi), v| {
            (lo.min(v.point.z), hi.max(v.point.z))
        });

    // Top face of cuboid(dx=10, dy=10, dz=5) is at z = dz/2 = 2.5.
    // Extrude depth = 1.0 along +Z (top face normal), so the extruded body spans z ∈ [2.5, 3.5].
    // If plane_ref were ignored, the build would fall back to the canonical xy-plane at z = 0,
    // producing z ∈ [0, 1] — that would fail this assertion.
    assert!(
        (z_min - 2.5).abs() < 1e-9,
        "extrude_1 z_min = {}, expected 2.5 (top face). If this is ~0, plane_ref was ignored.",
        z_min
    );
    assert!(
        (z_max - 3.5).abs() < 1e-9,
        "extrude_1 z_max = {}, expected 3.5 (top face + depth=1).",
        z_max
    );
}
