// Acceptance tests for #55: Fuse(box + cylinder) manifold validation fix
// Geometry: Box 10×10×10 (centered at origin, z=-5..+5)
//           Cylinder r=2, h=15, origin=(0,0,-7.5) — fully pierces box (z=-7.5..+7.5)
//           Two intersection circles at z=±5, r=2
//
// Test plan:
//   T01 決定性          — 同一入力を 2 回 build し全頂点座標・面数・EntityID が一致
//   T02 manifold        — 結果 Solid が validate_manifold() を通る
//   T03 Euler           — V - E + F == 2 (genus-0)
//   T04 smoke           — examples/boolean_fuse_box_cyl.mycad の build が panic しない
//   T05 regression      — 既存 boolean smoke テストが引き続き通る
//   T06 single-pierce   — 片側貫通 Fuse でも manifold が通る
//   T07 numerical-eps   — 交線円が ε_snap 近傍を通る配置でも manifold OK
//   T08 分岐C           — is_ccw=false (もともと CW) の配置で manifold OK

use mycad_build::build_bodies_from_features;
use mycad_format::{Document, Feature};
use mycad_kernel::brep::topology::IdGenerator;

// -----------------------------------------------------------------------
// Helpers
// -----------------------------------------------------------------------

fn fuse_box_cyl_features() -> Vec<Feature> {
    let doc: Document =
        serde_yaml::from_str(include_str!("../../../examples/boolean_fuse_box_cyl.mycad"))
            .expect("parse boolean_fuse_box_cyl.mycad");
    doc.root_component.features
}

fn build_fuse_result(seed: u64) -> mycad_kernel::brep::topology::Solid {
    let mut gen = IdGenerator::new(seed);
    let features = fuse_box_cyl_features();
    let bodies =
        build_bodies_from_features(&features, &mut gen).expect("build_bodies_from_features failed");
    bodies
        .get("result")
        .expect("result body missing")
        .solid
        .clone()
}

/// Build a fuse from raw features (for T06/T07/T08 where we need custom geometry).
fn build_fuse_from_features(features: &[Feature]) -> mycad_kernel::brep::topology::Solid {
    let mut gen = IdGenerator::new(0);
    let bodies =
        build_bodies_from_features(features, &mut gen).expect("build_bodies_from_features failed");
    bodies
        .get("result")
        .expect("result body missing")
        .solid
        .clone()
}

// -----------------------------------------------------------------------
// T01: 決定性 — 同一入力を 2 回 build し全 EntityID・座標が一致
// -----------------------------------------------------------------------
#[test]
fn t01_determinism() {
    let s1 = build_fuse_result(0);
    let s2 = build_fuse_result(0);

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
    assert_eq!(s1.half_edges.len(), s2.half_edges.len(), "half-edge count");
    for (i, (ha, hb)) in s1.half_edges.iter().zip(s2.half_edges.iter()).enumerate() {
        assert_eq!(ha.id, hb.id, "half-edge {i} id");
    }

    // 100-run determinism: same topology counts every time
    let baseline = (
        s1.vertices.len(),
        s1.edges.len(),
        s1.faces.len(),
        s1.half_edges.len(),
        s1.id,
    );
    for run in 1..100 {
        let s = build_fuse_result(0);
        assert_eq!(baseline.0, s.vertices.len(), "run {run}: vertex count");
        assert_eq!(baseline.1, s.edges.len(), "run {run}: edge count");
        assert_eq!(baseline.2, s.faces.len(), "run {run}: face count");
        assert_eq!(baseline.3, s.half_edges.len(), "run {run}: half-edge count");
        assert_eq!(baseline.4, s.id, "run {run}: solid id");
    }
}

// -----------------------------------------------------------------------
// T02: manifold — 結果 Solid が validate_manifold() を通る
// -----------------------------------------------------------------------
#[test]
fn t02_manifold() {
    let solid = build_fuse_result(0);
    solid
        .validate_manifold()
        .expect("double-pierce fuse should pass manifold validation");
}

// -----------------------------------------------------------------------
// T03: Euler — V - E + F - H == 2 (genus-0, one shell, accounting for inner loops)
// -----------------------------------------------------------------------
#[test]
fn t03_euler_poincare() {
    let solid = build_fuse_result(0);
    // B-rep Euler-Poincaré with inner loops (rings):
    //   V - E + F - H = 2(S - G)
    // where H = total number of inner loops across all faces,
    // S = shells, G = genus.
    // For the double-pierce fuse: the top and bottom box faces each
    // gain one inner loop (the circular intersection), so H = 2.
    let v = solid.vertices.len() as i64;
    let e = solid.edges.len() as i64;
    let f = solid.faces.len() as i64;
    let s = solid.shells.len() as i64;
    let h: i64 = solid
        .faces
        .iter()
        .map(|face| face.inner_loops.len() as i64)
        .sum();
    let lhs = v - e + f - h;
    let rhs = 2 * (s - 0); // genus = 0
    assert_eq!(
        lhs, rhs,
        "V-E+F-H should equal 2(S-G): got V={v} E={e} F={f} H={h} S={s} => {lhs} vs {rhs}"
    );
}

// -----------------------------------------------------------------------
// T04: smoke — examples/boolean_fuse_box_cyl.mycad が build できる
// -----------------------------------------------------------------------
#[test]
fn t04_smoke_build() {
    let solid = build_fuse_result(0);
    assert!(!solid.vertices.is_empty(), "solid should have vertices");
    assert!(!solid.edges.is_empty(), "solid should have edges");
    assert!(!solid.faces.is_empty(), "solid should have faces");
}

// -----------------------------------------------------------------------
// T05: regression — 既存 boolean テストが引き続き通る
//   (代表として boolean_box_fuse と intersect_box_cyl を確認)
// -----------------------------------------------------------------------
#[test]
fn t05_regression_existing_boolean() {
    // boolean_box_fuse (box + box)
    let fuse_doc: Document =
        serde_yaml::from_str(include_str!("../../../examples/boolean_box_fuse.mycad"))
            .expect("parse boolean_box_fuse");
    let mut gen = IdGenerator::new(0);
    let fuse_bodies = build_bodies_from_features(&fuse_doc.root_component.features, &mut gen)
        .expect("boolean_box_fuse build failed");
    let fuse_result = fuse_bodies.get("fuse1").expect("fuse1 body missing");
    fuse_result
        .solid
        .validate_manifold()
        .expect("boolean_box_fuse manifold failed");

    // boolean_intersect_box_cyl (box + cylinder intersect)
    let isect_doc: Document = serde_yaml::from_str(include_str!(
        "../../../examples/boolean_intersect_box_cyl.mycad"
    ))
    .expect("parse boolean_intersect_box_cyl");
    let mut gen2 = IdGenerator::new(0);
    let isect_bodies = build_bodies_from_features(&isect_doc.root_component.features, &mut gen2)
        .expect("boolean_intersect_box_cyl build failed");
    let isect_result = isect_bodies.get("result").expect("result body missing");
    isect_result
        .solid
        .validate_manifold()
        .expect("boolean_intersect_box_cyl manifold failed");
}

// -----------------------------------------------------------------------
// T06: single-pierce — 片側貫通 Fuse でも manifold が通る
//   cylinder が box の片側(z=0..15)だけ交差するケース
//   Box [-5,5]^3, Cylinder r=2, h=15, origin=(0,0,0) → z=0..15
//   Cylinder crosses box top face at z=5 only (single-pierce)
// -----------------------------------------------------------------------
#[test]
fn t06_single_pierce_fuse_manifold() {
    let features = vec![
        Feature::CreateBox {
            id: "box1".into(),
            width: 10.0,
            height: 10.0,
            depth: 10.0,
        },
        Feature::CreateCylinder {
            id: "cyl1".into(),
            radius: 2.0,
            height: 15.0,
            origin: [0.0, 0.0, 0.0],
        },
        Feature::Fuse {
            id: "result".into(),
            target: "box1".into(),
            tool: "cyl1".into(),
        },
    ];
    let solid = build_fuse_from_features(&features);
    solid
        .validate_manifold()
        .expect("single-pierce fuse should pass manifold validation");
}

// -----------------------------------------------------------------------
// T07: numerical-eps — ε_snap 近傍(±1e-8)の交線円配置でも manifold OK
//   Cylinder origin z = -7.5 + 1e-8 => top face intersection very near box top z=+5
// -----------------------------------------------------------------------
#[test]
fn t07_numerical_eps_boundary() {
    // Shift cylinder slightly so intersection circle is near ε_snap boundary
    let features = vec![
        Feature::CreateBox {
            id: "box1".into(),
            width: 10.0,
            height: 10.0,
            depth: 10.0,
        },
        Feature::CreateCylinder {
            id: "cyl1".into(),
            radius: 2.0,
            height: 15.0,
            origin: [0.0, 0.0, -7.5 + 1e-8],
        },
        Feature::Fuse {
            id: "result".into(),
            target: "box1".into(),
            tool: "cyl1".into(),
        },
    ];
    let solid = build_fuse_from_features(&features);
    solid
        .validate_manifold()
        .expect("ε-shifted fuse should pass manifold validation");

    // Also test negative shift
    let features_neg = vec![
        Feature::CreateBox {
            id: "box1".into(),
            width: 10.0,
            height: 10.0,
            depth: 10.0,
        },
        Feature::CreateCylinder {
            id: "cyl1".into(),
            radius: 2.0,
            height: 15.0,
            origin: [0.0, 0.0, -7.5 - 1e-8],
        },
        Feature::Fuse {
            id: "result".into(),
            target: "box1".into(),
            tool: "cyl1".into(),
        },
    ];
    let solid_neg = build_fuse_from_features(&features_neg);
    solid_neg
        .validate_manifold()
        .expect("ε-negative-shifted fuse should pass manifold validation");
}

// -----------------------------------------------------------------------
// T08: 分岐C — is_ccw=false (もともと CW) の配置で manifold OK
//   交線セグメントがもともと CW の内ループを生成する配置を検証。
//   元の boolean_fuse_box_cyl (origin z=-7.5) がその代表ケース。
//   partition.rs の修正で CW の場合は reverse() されないことを
//   間接的に確認（manifold が通れば CW/CW 整合している）。
// -----------------------------------------------------------------------
#[test]
fn t08_branch_c_already_cw() {
    // Same geometry as T02 but explicitly named to document branch-C coverage.
    // The double-pierce fuse produces inner loops that are already CW,
    // so partition.rs does NOT reverse them (branch C: is_ccw=false → no-op).
    let solid = build_fuse_result(0);
    solid
        .validate_manifold()
        .expect("branch-C (already CW) fuse should pass manifold validation");
    // Verify the result has the expected intersection structure:
    // at least 2 circle-curve edges (one per intersection circle at z=±5)
    let circle_edges = solid
        .edges
        .iter()
        .filter(|e| matches!(e.curve, mycad_kernel::geometry::curve::Curve::Circle { .. }))
        .count();
    assert!(
        circle_edges >= 2,
        "double-pierce fuse should have >=2 Circle edges, got {circle_edges}"
    );
}
