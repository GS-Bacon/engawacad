//! ModelFace Sketch を ExtrudeCut で押出カットする回帰テスト。
//!
//! # スコープ
//!
//! ## Contract
//!
//! - `bodies.len() == 1` (extrude_cut が target を正しく consume)
//! - `feature_id == "extrude_cut_1"` (boolean 結果が正しい id で register される)
//! - 選択面の法線と逆方向へ実カットされ、指定 depth に穴底が生成される
//! - `validate_manifold()` と `euler_poincare() == 0`
//! - 決定性 (同じ入力 + IdGenerator(0) seed → vertex/edge/face ID と座標が bit-equal)
//!
//! # 関連ファイル
//!
//! - plan.md: features/218-phase8-modelface-sketch-extrudecut/plan.md
//! - follow-up: https://github.com/GS-Bacon/engawacad/issues/220
//! - 参考 (同じ boolean Cut パスの先行 fixture): crates/engawa-build/tests/face_entity_ref_planeref.rs::t04_plane_ref_entity_extrude_cut (#215)

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
    let yaml = include_str!("../../../examples/sketch_extrudecut_hole.engawa");
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

/// T02_plumbing: active main contract from the file header — `bodies.len() == 1`,
/// `feature_id == "extrude_cut_1"`, `validate_manifold()` (same gate as #215 t04).
/// The following topology test additionally verifies the Euler–Poincaré invariant.
#[test]
fn t02_plumbing_manifold_contract() {
    let yaml = include_str!("../../../examples/sketch_extrudecut_hole.engawa");
    let doc = Document::from_yaml(yaml).expect("YAML parse failed");

    let mut gen = IdGenerator::new(0);
    let bodies = build_assembly(&doc, Path::new("examples"), &mut gen).expect("build failed");

    assert_eq!(
        bodies.len(),
        1,
        "extrude_cut_1 should consume box_1 (got {} bodies)",
        bodies.len()
    );
    let result = &bodies[0];
    assert_eq!(result.feature_id, "extrude_cut_1");

    result.solid.validate_manifold().expect(
        "ExtrudeCut + Face EntityRef result must satisfy validate_manifold (same gate as #215 t04)",
    );

    // Target-vs-tool discriminator (B-6 cross-cut F01): the cuboid spans z ∈ [-5, 5],
    // the tool alone would span z ∈ [5, 10]. Asserting the result's z range == [-5, 5]
    // proves the returned body is the boolean Cut output (which still contains the
    // original target volume), not the standalone tool extrusion.
    let (z_min, z_max) = result
        .solid
        .vertices
        .iter()
        .fold((f64::INFINITY, f64::NEG_INFINITY), |(lo, hi), v| {
            (lo.min(v.point.z), hi.max(v.point.z))
        });
    assert!(
        (z_min - (-5.0)).abs() < 1e-9,
        "result z_min = {}, expected -5 (target cuboid bottom); a returned-tool regression would give z_min = 5",
        z_min
    );
    assert!(
        (z_max - 5.0).abs() < 1e-9,
        "result z_max = {}, expected 5 (target cuboid top); a returned-tool regression would give z_max = 10",
        z_max
    );

    // Face basis sanity (Codex r5 F01): the f_z_pos top face has origin (0,0,5),
    // normal +Z, u_axis = -X, v_axis = +Y (see cuboid.rs:130-140). A profile point
    // (u, v) therefore maps to world (-u, v, 5). The fixture's asymmetric rectangle
    // [(0,0)-(2,2)] in plane uv must contribute the four corner vertices
    // (0,0,5), (-2,0,5), (-2,2,5), (0,2,5) to the result solid. If a u_axis/v_axis
    // swap or sign flip occurred, those points would land elsewhere and this assert
    // would fail loudly.
    let expected_corners = [
        (0.0, 0.0, 5.0),
        (-2.0, 0.0, 5.0),
        (-2.0, 2.0, 5.0),
        (0.0, 2.0, 5.0),
    ];
    for (ex, ey, ez) in expected_corners {
        let found = result.solid.vertices.iter().any(|v| {
            (v.point.x - ex).abs() < 1e-9
                && (v.point.y - ey).abs() < 1e-9
                && (v.point.z - ez).abs() < 1e-9
        });
        assert!(
            found,
            "expected cut profile vertex at ({ex}, {ey}, {ez}) is missing — a u/v swap or sign flip in f_z_pos basis resolution would cause this"
        );
    }
}

/// T02_topology: the blind cavity remains a valid manifold.
#[test]
fn t02_topology_manifold() {
    let yaml = include_str!("../../../examples/sketch_extrudecut_hole.engawa");
    let doc = Document::from_yaml(yaml).expect("YAML parse failed");

    let mut gen = IdGenerator::new(0);
    let bodies = build_assembly(&doc, Path::new("examples"), &mut gen).expect("build failed");
    let result = &bodies[0];

    result.solid.validate_manifold().expect("manifold cavity");
    assert_eq!(result.solid.euler_poincare(), 0);
}

/// T03: depth=5 cuts inward from z=5 and creates a hole bottom at z=0.
#[test]
fn t03_hole_bottom_vertex_at_inner_depth() {
    let yaml = include_str!("../../../examples/sketch_extrudecut_hole.engawa");
    let doc = Document::from_yaml(yaml).expect("YAML parse failed");

    let mut gen = IdGenerator::new(0);
    let bodies = build_assembly(&doc, Path::new("examples"), &mut gen).expect("build failed");

    let result = &bodies[0];

    let (z_min, z_max) = result
        .solid
        .vertices
        .iter()
        .fold((f64::INFINITY, f64::NEG_INFINITY), |(lo, hi), v| {
            (lo.min(v.point.z), hi.max(v.point.z))
        });

    assert!(
        (z_min - (-5.0)).abs() < 1e-9,
        "z_min = {}, expected target bottom -5",
        z_min
    );
    assert!(
        (z_max - 5.0).abs() < 1e-9,
        "z_max = {}, expected target top 5",
        z_max
    );
    assert!(
        result.solid.vertices.iter().any(|v| {
            (v.point.x + 2.0).abs() < 1e-9 && v.point.y.abs() < 1e-9 && v.point.z.abs() < 1e-9
        }),
        "expected a profile corner on the hole bottom at z=0"
    );
    result.solid.validate_manifold().expect("manifold cavity");
    assert_eq!(result.solid.euler_poincare(), 0);
}

/// T04: cuboid resize regression — 20x20x8 で同 EntityRef 解決の上 hole が drilled される
///
#[test]
fn t04_resize_cuboid_same_entityref_hole_still_drilled() {
    // Same feature list but cuboid is 20x20x8 (top face at z = dz/2 = 4.0)
    let yaml = r#"
schema_version: 1
version: "0.1.0"
root_component:
  name: "Octagonal Hole with Resized Cuboid"
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
    - type: extrude_cut
      id: extrude_cut_1
      sketch: sketch_1
      depth: 2.0
      target: box_1
"#;

    let doc = Document::from_yaml(yaml).expect("YAML parse failed");
    let mut gen = IdGenerator::new(0);

    let bodies = build_assembly(&doc, Path::new("examples"), &mut gen).expect("build failed");

    assert_eq!(bodies.len(), 1, "extrude_cut_1 should consume box_1");

    let result = &bodies[0];

    // Cuboid(dx=20, dy=20, dz=8) centered at origin has z range [-4.0, 4.0].
    let (z_min, z_max) = result
        .solid
        .vertices
        .iter()
        .fold((f64::INFINITY, f64::NEG_INFINITY), |(lo, hi), v| {
            (lo.min(v.point.z), hi.max(v.point.z))
        });

    assert!(
        (z_min - (-4.0)).abs() < 1e-9,
        "z_min = {}, expected resized cuboid bottom -4.0",
        z_min
    );
    assert!(
        (z_max - 4.0).abs() < 1e-9,
        "z_max = {}, expected resized cuboid top 4.0",
        z_max
    );
    assert!(
        result
            .solid
            .vertices
            .iter()
            .any(|v| (v.point.z - 2.0).abs() < 1e-9),
        "expected hole-bottom vertices at z=2.0"
    );
    result.solid.validate_manifold().expect("manifold cavity");
    assert_eq!(result.solid.euler_poincare(), 0);
}

/// T05_degen_zero_depth: depth=0 で InvalidParameter { kind: "depth" }
#[test]
fn t05_degen_zero_depth() {
    // Same as sketch_extrudecut_hole.engawa but with depth: 0.0
    let yaml = r#"
schema_version: 1
version: "0.1.0"
root_component:
  name: "Zero Depth ExtrudeCut"
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
    - type: extrude_cut
      id: extrude_cut_1
      sketch: sketch_1
      depth: 0.0
      target: box_1
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

/// T06_existing_t04_non_regression: 既存 t04_plane_ref_entity_extrude_cut と同等の build を acceptance 内で重複実行
///
/// 既存 face_entity_ref_planeref.rs::t04_plane_ref_entity_extrude_cut (#215) と同じ features 列を
/// この acceptance test 内でも明示的に build し、既存 assertion (faces.len() >= 6 + manifold) が pass する
#[test]
fn t06_existing_t04_non_regression() {
    // 既存 face_entity_ref_planeref.rs::t04 と同じ features
    let yaml = r#"
schema_version: 1
version: "0.1.0"
root_component:
  name: "ExtrudeCut EntityRef Non-Regression"
  features:
    - type: create_box
      id: box_target
      width: 10.0
      height: 10.0
      depth: 10.0
    - type: create_sketch
      id: sketch_cut
      plane: xy
      offset: 0.0
      profile:
        - id: seg_a
          from: [0.0, 0.0]
          to:   [2.0, 0.0]
        - id: seg_b
          from: [2.0, 0.0]
          to:   [2.0, 2.0]
        - id: seg_c
          from: [2.0, 2.0]
          to:   [0.0, 2.0]
        - id: seg_d
          from: [0.0, 2.0]
          to:   [0.0, 0.0]
      plane_ref:
        ref: named
        feature_id: box_target
        kind: face
        role: f_z_pos
    - type: extrude_cut
      id: cut_1
      sketch: sketch_cut
      depth: 5.0
      target: box_target
"#;

    let doc = Document::from_yaml(yaml).expect("YAML parse failed");
    let mut gen = IdGenerator::new(0);

    let result = build_assembly(&doc, Path::new("examples"), &mut gen);
    assert!(result.is_ok(), "build_assembly should succeed");

    let bodies = result.unwrap();
    assert_eq!(bodies.len(), 1, "cut result = 1 body");

    let result_solid = &bodies[0].solid;
    assert!(
        result_solid.faces.len() >= 6,
        "cut result must have at least 6 faces, got {}",
        result_solid.faces.len()
    );
    result_solid
        .validate_manifold()
        .expect("ExtrudeCut 後の Solid は manifold");
}

/// T07_phase3_extrude_existing_smoke: 既存 extruded_rect.engawa を build → Ok (Phase 3 path non-regression)
#[test]
fn t07_phase3_extrude_existing_smoke() {
    let yaml = include_str!("../../../examples/extruded_rect.engawa");
    let doc = Document::from_yaml(yaml).expect("YAML parse failed");
    let mut gen = IdGenerator::new(0);

    let result = build_assembly(&doc, Path::new("examples"), &mut gen);
    assert!(
        result.is_ok(),
        "extruded_rect.engawa build should succeed (Phase 3 non-regression)"
    );
}

/// T08_negative_depth_dispatcher_guard: depth=-1.0 → InvalidParameter { kind: "depth" }
#[test]
fn t08_negative_depth_dispatcher_guard() {
    let yaml = r#"
schema_version: 1
version: "0.1.0"
root_component:
  name: "Negative Depth ExtrudeCut"
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
    - type: extrude_cut
      id: extrude_cut_1
      sketch: sketch_1
      depth: -1.0
      target: box_1
"#;

    let doc = Document::from_yaml(yaml).expect("YAML parse failed");
    let mut gen = IdGenerator::new(0);

    let result = build_assembly(&doc, Path::new("examples"), &mut gen);
    let err = result.expect_err("build_assembly should fail with negative depth");
    assert!(
        matches!(&err, KernelError::InvalidParameter { kind } if *kind == "depth"),
        "expected KernelError::InvalidParameter {{ kind: \"depth\" }}, got: {:?}",
        err
    );
}

/// T09_unknown_role_graceful: 存在しない role "f_xxx" → FaceEntityRefNotFound { .. }
#[test]
fn t09_unknown_role_graceful() {
    let yaml = r#"
schema_version: 1
version: "0.1.0"
root_component:
  name: "Unknown Role ExtrudeCut"
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
    - type: extrude_cut
      id: extrude_cut_1
      sketch: sketch_1
      depth: 2.0
      target: box_1
"#;

    let doc = Document::from_yaml(yaml).expect("YAML parse failed");
    let mut gen = IdGenerator::new(0);

    let result = build_assembly(&doc, Path::new("examples"), &mut gen);
    let err = result.expect_err("build_assembly should fail with unknown role");
    assert!(
        matches!(&err, KernelError::FaceEntityRefNotFound { .. }),
        "expected KernelError::FaceEntityRefNotFound, got: {:?}",
        err
    );
}

/// T10_very_small_depth: depth=1e-6 (極小値) → build 成功 / bodies.len() == 1
///
#[test]
fn t10_very_small_depth() {
    let yaml = r#"
schema_version: 1
version: "0.1.0"
root_component:
  name: "Very Small Depth ExtrudeCut"
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
    - type: extrude_cut
      id: extrude_cut_1
      sketch: sketch_1
      depth: 0.000001
      target: box_1
"#;

    let doc = Document::from_yaml(yaml).expect("YAML parse failed");
    let mut gen = IdGenerator::new(0);

    let result = build_assembly(&doc, Path::new("examples"), &mut gen);
    assert!(
        result.is_ok(),
        "build_assembly should succeed with very small depth"
    );
    let bodies = result.unwrap();
    assert_eq!(bodies.len(), 1, "bodies.len() == 1");
    bodies[0]
        .solid
        .validate_manifold()
        .expect("small-depth cut remains manifold");
    assert_eq!(bodies[0].solid.euler_poincare(), 0);
}

/// T11_two_run_topology_determinism: full B-rep determinism. Serializes the entire
/// resulting Solid (vertices + half_edges + edges + loops + faces + shells + names)
/// via serde_json and compares the two runs as byte-equal strings. This catches any
/// non-deterministic change to topology connectivity, half-edge twin assignments,
/// shell composition, or EntityRef naming — not just vertex coords or top-level IDs
/// (Codex r5 F02).
#[test]
fn t11_two_run_topology_determinism() {
    let yaml = include_str!("../../../examples/sketch_extrudecut_hole.engawa");
    let doc = Document::from_yaml(yaml).expect("YAML parse failed");

    let build_snapshot = || {
        let mut gen = IdGenerator::new(0);
        let bodies = build_assembly(&doc, Path::new("examples"), &mut gen).expect("build failed");
        let result = &bodies[0];
        // Serialize feature_id + the entire Solid: includes vertices (with names),
        // half_edges, edges (with names), loops, faces (with names + outer/inner loops),
        // shells, and the solid id itself.
        serde_json::to_string(&(&result.feature_id, &result.solid))
            .expect("Solid must be serializable")
    };

    let snap1 = build_snapshot();
    let snap2 = build_snapshot();

    assert_eq!(
        snap1, snap2,
        "full Solid serialization (vertices/half_edges/edges/loops/faces/shells/names) must be byte-equal across two runs"
    );
}
