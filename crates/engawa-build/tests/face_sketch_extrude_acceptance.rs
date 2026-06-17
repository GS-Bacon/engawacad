//! Issue #217 STEP 6: Acceptance Test Implementation
//!
//! ModelFace Sketch を Extrude で押出 (e2e + 決定性) の受入テスト。
//! plan.md (features/217-phase8-modelface-sketch-extrude/plan.md) の T01〜T05_degen_zero_depth に対応する。

use engawa_build::build_assembly;
use engawa_format::Document;
use engawa_kernel::brep::topology::IdGenerator;
use engawa_kernel::tessellation::{
    merge_meshes, tessellate_solid_with, to_ascii_stl, TessellationOptions,
};
use engawa_kernel::KernelError;
use std::path::Path;

/// T01: STL byte 列 3-run 決定性
#[test]
fn t01_stl_three_run_determinism() {
    let yaml = include_str!("../../../examples/sketch_extrude_pillar.engawa");
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

/// T02: build_assembly 成功 + bodies.len == 2 + extrude_1 z range [2.5, 4.5] + euler_poincare == 0
#[test]
fn t02_build_assembly_e2e_z_range_and_euler() {
    let yaml = include_str!("../../../examples/sketch_extrude_pillar.engawa");
    let doc = Document::from_yaml(yaml).expect("YAML parse failed");

    let mut gen = IdGenerator::new(0);
    let bodies = build_assembly(&doc, Path::new("examples"), &mut gen).expect("build failed");

    // box_1 (create_box) and extrude_1 are both live bodies
    assert_eq!(bodies.len(), 2);

    let extrude = bodies
        .iter()
        .find(|b| b.feature_id == "extrude_1")
        .expect("extrude_1 body should be present");

    // Top face of cuboid(dx=10, dy=10, dz=5) is at z = dz/2 = 2.5.
    // Extrude depth = 2.0 along +Z, so the extruded body spans z ∈ [2.5, 4.5].
    let (z_min, z_max) = extrude
        .solid
        .vertices
        .iter()
        .fold((f64::INFINITY, f64::NEG_INFINITY), |(lo, hi), v| {
            (lo.min(v.point.z), hi.max(v.point.z))
        });

    assert!(
        (z_min - 2.5).abs() < 1e-9,
        "extrude_1 z_min = {}, expected 2.5 (top face)",
        z_min
    );
    assert!(
        (z_max - 4.5).abs() < 1e-9,
        "extrude_1 z_max = {}, expected 4.5 (top face + depth=2)",
        z_max
    );

    // Euler-Poincaré: V - E + F = 0 for a solid with a cavity (topologically a torus-like shell)
    // For a simple extruded octagon pillar, euler_poincare should be 0
    assert_eq!(extrude.solid.euler_poincare(), 0);

    // B-rep manifold integrity (B-6 r2 F01): half-edge twin/next, loop closure, shell
    // composition. Euler count alone could pass with a broken topology that happens to
    // match the same V-E+F-2S sum.
    extrude
        .solid
        .validate_manifold()
        .expect("extrude_1 must be a valid manifold solid");
}

/// T03: 押出方向が +Z (Face 法線) — extrude_1 全頂点 z >= 2.5 - 1e-9
#[test]
fn t03_extrude_direction_along_face_normal_positive_z() {
    let yaml = include_str!("../../../examples/sketch_extrude_pillar.engawa");
    let doc = Document::from_yaml(yaml).expect("YAML parse failed");

    let mut gen = IdGenerator::new(0);
    let bodies = build_assembly(&doc, Path::new("examples"), &mut gen).expect("build failed");

    let extrude = bodies
        .iter()
        .find(|b| b.feature_id == "extrude_1")
        .expect("extrude_1 body should be present");

    // All vertices should be at or above the top face (z=2.5), meaning extrusion is +Z direction
    let top_z = 2.5;
    let all_above = extrude
        .solid
        .vertices
        .iter()
        .all(|v| v.point.z >= top_z - 1e-9);
    assert!(all_above, "extrude_1 should be extruded in +Z direction");
}

/// T04: cuboid resize regression — 20x20x8 で同 EntityRef 解決、z range [4.0, 6.0]
#[test]
fn t04_resize_cuboid_same_entityref_still_resolves() {
    // Same feature list but cuboid is 20x20x8 (top face at z = dz/2 = 4.0)
    let yaml = r#"
schema_version: 1
version: "0.1.0"
root_component:
  name: "Octagon Pillar with Resized Cuboid"
  features:
    - type: create_box
      id: box_1
      width: 20.0
      height: 20.0
      depth: 8.0
    - type: create_sketch
      id: sketch_1
      plane: xy
      offset: 0.0
      profile:
        - id: seg_0
          from: [2.0, 0.0]
          to:   [1.4142135623730951, 1.4142135623730951]
        - id: seg_1
          from: [1.4142135623730951, 1.4142135623730951]
          to:   [0.0, 2.0]
        - id: seg_2
          from: [0.0, 2.0]
          to:   [-1.4142135623730951, 1.4142135623730951]
        - id: seg_3
          from: [-1.4142135623730951, 1.4142135623730951]
          to:   [-2.0, 0.0]
        - id: seg_4
          from: [-2.0, 0.0]
          to:   [-1.4142135623730951, -1.4142135623730951]
        - id: seg_5
          from: [-1.4142135623730951, -1.4142135623730951]
          to:   [0.0, -2.0]
        - id: seg_6
          from: [0.0, -2.0]
          to:   [1.4142135623730951, -1.4142135623730951]
        - id: seg_7
          from: [1.4142135623730951, -1.4142135623730951]
          to:   [2.0, 0.0]
      plane_ref:
        ref: named
        feature_id: box_1
        kind: face
        role: f_z_pos
    - type: extrude
      id: extrude_1
      sketch: sketch_1
      depth: 2.0
"#;

    let doc = Document::from_yaml(yaml).expect("YAML parse failed");
    let mut gen = IdGenerator::new(0);

    let bodies = build_assembly(&doc, Path::new("examples"), &mut gen).expect("build failed");

    let extrude = bodies
        .iter()
        .find(|b| b.feature_id == "extrude_1")
        .expect("extrude_1 body should be present");

    // Top face of cuboid(dx=20, dy=20, dz=8) is at z = dz/2 = 4.0.
    // Extrude depth = 2.0 along +Z, so z ∈ [4.0, 6.0].
    let (z_min, z_max) = extrude
        .solid
        .vertices
        .iter()
        .fold((f64::INFINITY, f64::NEG_INFINITY), |(lo, hi), v| {
            (lo.min(v.point.z), hi.max(v.point.z))
        });

    assert!(
        (z_min - 4.0).abs() < 1e-9,
        "extrude_1 z_min = {}, expected 4.0 (resized cuboid top face)",
        z_min
    );
    assert!(
        (z_max - 6.0).abs() < 1e-9,
        "extrude_1 z_max = {}, expected 6.0 (resized top face + depth=2)",
        z_max
    );
}

/// T05_degen_zero_depth: depth=0 で InvalidParameter { kind: "depth" }
#[test]
fn t05_degen_zero_depth() {
    // Same as sketch_extrude_pillar.engawa but with depth: 0.0
    let yaml = r#"
schema_version: 1
version: "0.1.0"
root_component:
  name: "Zero Depth Extrusion"
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
          from: [2.0, 0.0]
          to:   [1.4142135623730951, 1.4142135623730951]
        - id: seg_1
          from: [1.4142135623730951, 1.4142135623730951]
          to:   [0.0, 2.0]
        - id: seg_2
          from: [0.0, 2.0]
          to:   [-1.4142135623730951, 1.4142135623730951]
        - id: seg_3
          from: [-1.4142135623730951, 1.4142135623730951]
          to:   [-2.0, 0.0]
        - id: seg_4
          from: [-2.0, 0.0]
          to:   [-1.4142135623730951, -1.4142135623730951]
        - id: seg_5
          from: [-1.4142135623730951, -1.4142135623730951]
          to:   [0.0, -2.0]
        - id: seg_6
          from: [0.0, -2.0]
          to:   [1.4142135623730951, -1.4142135623730951]
        - id: seg_7
          from: [1.4142135623730951, -1.4142135623730951]
          to:   [2.0, 0.0]
      plane_ref:
        ref: named
        feature_id: box_1
        kind: face
        role: f_z_pos
    - type: extrude
      id: extrude_1
      sketch: sketch_1
      depth: 0.0
"#;

    let doc = Document::from_yaml(yaml).expect("YAML parse failed");
    let mut gen = IdGenerator::new(0);

    let result = build_assembly(&doc, Path::new("examples"), &mut gen);

    let err = result.expect_err("build_assembly should fail with zero depth");
    assert!(
        matches!(&err, KernelError::InvalidParameter { kind } if *kind == "depth"),
        "expected KernelError::InvalidParameter {{ kind: \"depth\" }}, got: {:?}",
        err
    );
}

/// T06: Phase 3 extruded_rect.engawa non-regression test
#[test]
fn t06_phase3_extruded_rect_non_regression() {
    let yaml = include_str!("../../../examples/extruded_rect.engawa");
    let doc = Document::from_yaml(yaml).expect("YAML parse failed");
    let mut gen = IdGenerator::new(0);

    let result = build_assembly(&doc, Path::new("examples"), &mut gen);
    assert!(
        result.is_ok(),
        "Phase 3 extruded_rect.engawa should build successfully"
    );
}

/// T07: pillar_top_face_resolved_via_entity_ref — 上面 (z=4.5) に 8 頂点以上存在
#[test]
fn t07_pillar_top_face_resolved_via_entity_ref() {
    let yaml = include_str!("../../../examples/sketch_extrude_pillar.engawa");
    let doc = Document::from_yaml(yaml).expect("YAML parse failed");

    let mut gen = IdGenerator::new(0);
    let bodies = build_assembly(&doc, Path::new("examples"), &mut gen).expect("build failed");

    let extrude = bodies
        .iter()
        .find(|b| b.feature_id == "extrude_1")
        .expect("extrude_1 body should be present");

    let top_z = 4.5;
    let top_count = extrude
        .solid
        .vertices
        .iter()
        .filter(|v| (v.point.z - top_z).abs() < 1e-9)
        .count();

    assert!(
        top_count >= 8,
        "top face should have at least 8 vertices (octagon cap), got {}",
        top_count
    );
}

/// T08: negative_depth_signed_contract — depth=-1.0 で成功し、押出方向が逆転 (#110)
#[test]
fn t08_negative_depth_signed_contract() {
    // Negative depth is valid per Issue #110 (signed depth contract).
    // Depth = -1.0 extrudes in the opposite direction (-Z) from the face normal.
    let yaml = r#"
schema_version: 1
version: "0.1.0"
root_component:
  name: "Negative Depth Extrusion"
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
          from: [2.0, 0.0]
          to:   [1.4142135623730951, 1.4142135623730951]
        - id: seg_1
          from: [1.4142135623730951, 1.4142135623730951]
          to:   [0.0, 2.0]
        - id: seg_2
          from: [0.0, 2.0]
          to:   [-1.4142135623730951, 1.4142135623730951]
        - id: seg_3
          from: [-1.4142135623730951, 1.4142135623730951]
          to:   [-2.0, 0.0]
        - id: seg_4
          from: [-2.0, 0.0]
          to:   [-1.4142135623730951, -1.4142135623730951]
        - id: seg_5
          from: [-1.4142135623730951, -1.4142135623730951]
          to:   [0.0, -2.0]
        - id: seg_6
          from: [0.0, -2.0]
          to:   [1.4142135623730951, -1.4142135623730951]
        - id: seg_7
          from: [1.4142135623730951, -1.4142135623730951]
          to:   [2.0, 0.0]
      plane_ref:
        ref: named
        feature_id: box_1
        kind: face
        role: f_z_pos
    - type: extrude
      id: extrude_1
      sketch: sketch_1
      depth: -1.0
"#;

    let doc = Document::from_yaml(yaml).expect("YAML parse failed");
    let mut gen = IdGenerator::new(0);

    let bodies = build_assembly(&doc, Path::new("examples"), &mut gen)
        .expect("build_assembly should succeed with negative depth per #110");

    let extrude = bodies
        .iter()
        .find(|b| b.feature_id == "extrude_1")
        .expect("extrude_1 body should be present");

    // Top face of cuboid is at z = 2.5.
    // Negative depth (-1.0) extrudes in -Z direction, so z ∈ [1.5, 2.5].
    let (z_min, z_max) = extrude
        .solid
        .vertices
        .iter()
        .fold((f64::INFINITY, f64::NEG_INFINITY), |(lo, hi), v| {
            (lo.min(v.point.z), hi.max(v.point.z))
        });

    assert!(
        (z_min - 1.5).abs() < 1e-9,
        "extrude_1 z_min = {}, expected 1.5 (top face + depth=-1.0)",
        z_min
    );
    assert!(
        (z_max - 2.5).abs() < 1e-9,
        "extrude_1 z_max = {}, expected 2.5 (top face)",
        z_max
    );
}

/// T09: face_role_unknown_graceful — 存在しない role で FaceEntityRefNotFound
#[test]
fn t09_face_role_unknown_graceful() {
    let yaml = r#"
schema_version: 1
version: "0.1.0"
root_component:
  name: "Unknown Face Role"
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
          from: [2.0, 0.0]
          to:   [1.4142135623730951, 1.4142135623730951]
        - id: seg_1
          from: [1.4142135623730951, 1.4142135623730951]
          to:   [0.0, 2.0]
        - id: seg_2
          from: [0.0, 2.0]
          to:   [-1.4142135623730951, 1.4142135623730951]
        - id: seg_3
          from: [-1.4142135623730951, 1.4142135623730951]
          to:   [-2.0, 0.0]
        - id: seg_4
          from: [-2.0, 0.0]
          to:   [-1.4142135623730951, -1.4142135623730951]
        - id: seg_5
          from: [-1.4142135623730951, -1.4142135623730951]
          to:   [0.0, -2.0]
        - id: seg_6
          from: [0.0, -2.0]
          to:   [1.4142135623730951, -1.4142135623730951]
        - id: seg_7
          from: [1.4142135623730951, -1.4142135623730951]
          to:   [2.0, 0.0]
      plane_ref:
        ref: named
        feature_id: box_1
        kind: face
        role: f_xxx
    - type: extrude
      id: extrude_1
      sketch: sketch_1
      depth: 2.0
"#;

    let doc = Document::from_yaml(yaml).expect("YAML parse failed");
    let mut gen = IdGenerator::new(0);

    let result = build_assembly(&doc, Path::new("examples"), &mut gen);
    let err = result.expect_err("build_assembly should fail with unknown face role");
    assert!(
        matches!(&err, KernelError::FaceEntityRefNotFound { .. }),
        "expected KernelError::FaceEntityRefNotFound, got: {:?}",
        err
    );
}

/// T10: very_small_depth — depth=1e-6 で成功、z range [2.5, 2.500001]
#[test]
fn t10_very_small_depth() {
    let yaml = r#"
schema_version: 1
version: "0.1.0"
root_component:
  name: "Very Small Depth Extrusion"
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
          from: [2.0, 0.0]
          to:   [1.4142135623730951, 1.4142135623730951]
        - id: seg_1
          from: [1.4142135623730951, 1.4142135623730951]
          to:   [0.0, 2.0]
        - id: seg_2
          from: [0.0, 2.0]
          to:   [-1.4142135623730951, 1.4142135623730951]
        - id: seg_3
          from: [-1.4142135623730951, 1.4142135623730951]
          to:   [-2.0, 0.0]
        - id: seg_4
          from: [-2.0, 0.0]
          to:   [-1.4142135623730951, -1.4142135623730951]
        - id: seg_5
          from: [-1.4142135623730951, -1.4142135623730951]
          to:   [0.0, -2.0]
        - id: seg_6
          from: [0.0, -2.0]
          to:   [1.4142135623730951, -1.4142135623730951]
        - id: seg_7
          from: [1.4142135623730951, -1.4142135623730951]
          to:   [2.0, 0.0]
      plane_ref:
        ref: named
        feature_id: box_1
        kind: face
        role: f_z_pos
    - type: extrude
      id: extrude_1
      sketch: sketch_1
      depth: 0.000001
"#;

    let doc = Document::from_yaml(yaml).expect("YAML parse failed");
    let mut gen = IdGenerator::new(0);

    let bodies = build_assembly(&doc, Path::new("examples"), &mut gen).expect("build failed");

    let extrude = bodies
        .iter()
        .find(|b| b.feature_id == "extrude_1")
        .expect("extrude_1 body should be present");

    let z_max = extrude
        .solid
        .vertices
        .iter()
        .fold(f64::NEG_INFINITY, |hi, v| hi.max(v.point.z));

    let expected_z_max = 2.5 + 0.000001;
    assert!(
        (z_max - expected_z_max).abs() < 1e-9,
        "z_max = {}, expected {} (top face + depth=1e-6)",
        z_max,
        expected_z_max
    );
}

/// T11: two_run_full_solid_snapshot — 2 回実行で extrude_1 の B-rep snapshot 全体が一致。
/// B-6 r2 F02 対応: 頂点座標だけでなく solid.id / vertex/edge/face/half_edge/loop/shell の
/// 順序・接続・name まで含めて決定性を pin する (serde_json snapshot)。
#[test]
fn t11_two_run_solid_vertex_bytes_match() {
    let yaml = include_str!("../../../examples/sketch_extrude_pillar.engawa");
    let doc = Document::from_yaml(yaml).expect("YAML parse failed");

    let run = || {
        let mut gen = IdGenerator::new(0);
        let bodies = build_assembly(&doc, Path::new("examples"), &mut gen).expect("build failed");

        let extrude = bodies
            .iter()
            .find(|b| b.feature_id == "extrude_1")
            .expect("extrude_1 body should be present");

        serde_json::to_string(&(&extrude.feature_id, &extrude.solid))
            .expect("Solid must be serializable")
    };

    let verts_run1 = run();
    let verts_run2 = run();

    assert_eq!(
        verts_run1, verts_run2,
        "full Solid snapshot (vertices/half_edges/edges/loops/faces/shells/names + IDs) must match bit-for-bit across runs"
    );
}
