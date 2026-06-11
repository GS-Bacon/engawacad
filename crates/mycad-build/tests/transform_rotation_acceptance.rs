//! Acceptance tests for Issue #135: Component.transform.rotation を build 層に配線する
//!
//! テスト計画 (plan.md 参照):
//!   T01 — 決定性: rotation=[30,45,60] で 2 回 build → 完全一致
//!   T02 — rotation=[0,0,90] の cuboid: X/Y 入替, Z 不変 (snap で exact)
//!   T03 — rotation=[0,0,0] のみ: rotation 配線前と byte-equal
//!   T04 — 親 rotation × 子 (position+rotation): 階層合成
//!   T05 — rotation × Boolean Fuse: manifold + Euler-Poincaré
//!   T06_boundary_seam — Cylinder 軸回転後の orthonormal_basis 整合 (1e-12)
//!   T07_degen_zero_rotation — 全 component が rotation=[0,0,0] のとき translate のみ経路と一致
//!   T08_boundary_nonfinite — rotation に NaN → InvalidParameter
//!   T09 — euler_to_matrix (rad シグネチャ) が Rx(90°) を返す
//!   T10_blocked_cut_sphere — rotation × Boolean Cut Sphere (#137 blocked)

use mycad_build::build_assembly;
use mycad_format::component::Component;
use mycad_format::document::Document;
use mycad_format::feature::Feature;
use mycad_kernel::brep::topology::IdGenerator;
use mycad_kernel::geometry::transform::euler_to_matrix;
use mycad_kernel::geometry::Vec3;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn cuboid_doc(name: &str, id: &str, w: f64, h: f64, d: f64) -> Document {
    let mut doc = Document::new(name);
    doc.root_component.features.push(Feature::CreateBox {
        id: id.to_string(),
        width: w,
        height: h,
        depth: d,
    });
    doc
}

fn cylinder_doc(name: &str, id: &str, radius: f64, height: f64) -> Document {
    let mut doc = Document::new(name);
    doc.root_component.features.push(Feature::CreateCylinder {
        id: id.to_string(),
        radius,
        height,
        origin: [0.0, 0.0, 0.0],
    });
    doc
}

// ---------------------------------------------------------------------------
// T01: 決定性 — rotation 付き Document を 2 回 build → 全 Body が同一
// ---------------------------------------------------------------------------

#[test]
fn t01_determinism() {
    let mut doc = cuboid_doc("R", "b", 2.0, 3.0, 4.0);
    doc.root_component.transform.rotation = [30.0, 45.0, 60.0];

    let mut gen1 = IdGenerator::new(42);
    let bodies1 = build_assembly(&doc, std::path::Path::new("."), &mut gen1).unwrap();

    let mut gen2 = IdGenerator::new(42);
    let bodies2 = build_assembly(&doc, std::path::Path::new("."), &mut gen2).unwrap();

    // 決定性チェック: 同じ入力 → 同じ出力
    assert_eq!(bodies1.len(), bodies2.len());
    for (b1, b2) in bodies1.iter().zip(bodies2.iter()) {
        assert_eq!(b1.feature_id, b2.feature_id);

        // Vertex 座標と EntityID が完全一致
        assert_eq!(b1.solid.vertices.len(), b2.solid.vertices.len());
        for (v1, v2) in b1.solid.vertices.iter().zip(b2.solid.vertices.iter()) {
            assert_eq!(v1.id, v2.id);
            // epsilon 比較
            assert!(
                (v1.point.x - v2.point.x).abs() < 1e-12,
                "vertex x mismatch: {} vs {}",
                v1.point.x,
                v2.point.x
            );
            assert!(
                (v1.point.y - v2.point.y).abs() < 1e-12,
                "vertex y mismatch: {} vs {}",
                v1.point.y,
                v2.point.y
            );
            assert!(
                (v1.point.z - v2.point.z).abs() < 1e-12,
                "vertex z mismatch: {} vs {}",
                v1.point.z,
                v2.point.z
            );
        }

        // Edge/Face/Shells の数も一致
        assert_eq!(b1.solid.edges.len(), b2.solid.edges.len());
        assert_eq!(b1.solid.faces.len(), b2.solid.faces.len());
        assert_eq!(b1.solid.shells.len(), b2.solid.shells.len());
    }
}

// ---------------------------------------------------------------------------
// T02: rotation=[0,0,90] cuboid — Z 軸 90° 回転で X/Y 入替・Z 不変
// (本テストはバグ再現テスト: 実装前は rotation が無視されるため FAIL する)
// ---------------------------------------------------------------------------

#[test]
fn t02_z_90_rotation_swaps_xy() {
    // rotation=[0,0,90] (Z 軸 +90°): (x,y,z) → (-y, x, z) [snap で exact]
    // 非対称な 2x1x1 の cuboid (原点中心、各辺±[1,0.5,0.5]) を使用
    // 元の頂点 (1, 0.5, 0.5) は (-0.5, 1, 0.5) に移る。
    let mut doc = cuboid_doc("R", "b", 2.0, 1.0, 1.0);
    doc.root_component.transform.rotation = [0.0, 0.0, 90.0];

    let mut gen = IdGenerator::new(0);
    let bodies = build_assembly(&doc, std::path::Path::new("."), &mut gen).unwrap();
    assert_eq!(bodies.len(), 1);

    let solid = &bodies[0].solid;

    // 回転後の頂点 (-0.5, 1, 0.5) が存在する
    let has_rotated = solid.vertices.iter().any(|v| {
        (v.point.coords.x - (-0.5)).abs() < 1e-12
            && (v.point.coords.y - 1.0).abs() < 1e-12
            && (v.point.coords.z - 0.5).abs() < 1e-12
    });
    assert!(
        has_rotated,
        "Z 軸 90° 回転後に vertex (-0.5, 1, 0.5) が存在するはず"
    );

    // 元の (1, 0.5, 0.5) は存在しない
    let has_unrotated = solid.vertices.iter().any(|v| {
        (v.point.coords.x - 1.0).abs() < 1e-12
            && (v.point.coords.y - 0.5).abs() < 1e-12
            && (v.point.coords.z - 0.5).abs() < 1e-12
    });
    assert!(
        !has_unrotated,
        "実装後は元の (1, 0.5, 0.5) は存在しないはず (回転で移動)"
    );
}

// ---------------------------------------------------------------------------
// T03: rotation=[0,0,0] のみ → rotation 配線前と byte-equal (回帰防止)
// ---------------------------------------------------------------------------

#[test]
fn t03_zero_rotation_byte_equal() {
    // rotation=[0,0,0] のとき IDENTITY3 ガード (lib.rs:435) により Solid::rotate が呼ばれず、
    // 全 vertex 座標が「元 cuboid + position」の exact 値 (浮動小数ノイズなし) になることを保証。
    let mut doc = cuboid_doc("R", "b", 2.0, 3.0, 4.0);
    doc.root_component.transform.rotation = [0.0, 0.0, 0.0];
    doc.root_component.transform.position = [5.0, 6.0, 7.0];

    let mut gen = IdGenerator::new(1);
    let bodies = build_assembly(&doc, std::path::Path::new("."), &mut gen).unwrap();

    assert_eq!(bodies.len(), 1);
    let solid = &bodies[0].solid;

    // 元 cuboid 8 頂点 (±1, ±1.5, ±2) + position (5,6,7) = 期待頂点集合
    // assert_eq! で exact 比較 (rotation skip により浮動小数ノイズが入らないことを保証)
    let mut expected: Vec<[f64; 3]> = Vec::new();
    for &x in &[-1.0_f64, 1.0] {
        for &y in &[-1.5_f64, 1.5] {
            for &z in &[-2.0_f64, 2.0] {
                expected.push([x + 5.0, y + 6.0, z + 7.0]);
            }
        }
    }
    expected.sort_by(|a, b| a.partial_cmp(b).unwrap());

    let mut actual: Vec<[f64; 3]> = solid
        .vertices
        .iter()
        .map(|v| [v.point.coords.x, v.point.coords.y, v.point.coords.z])
        .collect();
    actual.sort_by(|a, b| a.partial_cmp(b).unwrap());

    // exact 一致 (rotation skip 経路の保証)
    assert_eq!(
        actual, expected,
        "rotation=[0,0,0] 経路では浮動小数ノイズが入らず exact 一致するべき"
    );
}

// ---------------------------------------------------------------------------
// T04: 階層合成 — 親 rotation × 子 (position+rotation)
// ---------------------------------------------------------------------------

#[test]
fn t04_nested_transform_composition() {
    // 親: rotation=[90,0,0] = Rx(90°) = [[1,0,0],[0,0,-1],[0,1,0]]
    // 子: position=[1,0,0], rotation=[0,90,0] = Ry(90°) = [[0,0,1],[0,1,0],[-1,0,0]]
    //
    // 累積:
    //   total_offset   = parent_offset + parent_rot * local_offset
    //                  = (0,0,0) + Rx(90°)*(1,0,0) = (1, 0, 0)
    //   total_rotation = parent_rot * local_rot = Rx(90°) * Ry(90°)
    //                  = [[0,0,1],[1,0,0],[0,1,0]]   (手計算検証済み)
    //
    // 1x1x1 cuboid (原点中心) の頂点 (±0.5, ±0.5, ±0.5) を total_rotation 適用 + translate (1,0,0):
    //   p_local = (x,y,z) → total_rot * p = (z, x, y) → +translate = (z+1, x, y)
    let mut parent = Component::new("parent");
    parent.transform.rotation = [90.0, 0.0, 0.0];

    let mut child = Component::new("child");
    child.transform.position = [1.0, 0.0, 0.0];
    child.transform.rotation = [0.0, 90.0, 0.0];
    child.features.push(Feature::CreateBox {
        id: "box".to_string(),
        width: 1.0,
        height: 1.0,
        depth: 1.0,
    });

    parent.children.push(child);

    let mut doc = Document::new("nested");
    doc.root_component = parent;

    let mut gen = IdGenerator::new(1);
    let bodies = build_assembly(&doc, std::path::Path::new("."), &mut gen).unwrap();

    assert_eq!(bodies.len(), 1);
    let solid = &bodies[0].solid;

    // 期待頂点集合: 元 (x,y,z) → (z+1, x, y), 各成分 ±0.5
    let mut expected: Vec<[f64; 3]> = Vec::new();
    for &x in &[-0.5_f64, 0.5] {
        for &y in &[-0.5_f64, 0.5] {
            for &z in &[-0.5_f64, 0.5] {
                expected.push([z + 1.0, x, y]);
            }
        }
    }
    // 期待頂点が actual に全て含まれることを 1e-12 epsilon で確認
    for exp in &expected {
        let found = solid.vertices.iter().any(|v| {
            (v.point.coords.x - exp[0]).abs() < 1e-12
                && (v.point.coords.y - exp[1]).abs() < 1e-12
                && (v.point.coords.z - exp[2]).abs() < 1e-12
        });
        assert!(
            found,
            "手計算期待頂点 ({}, {}, {}) が見つからない (total_rot=Rx(90°)*Ry(90°), translate=(1,0,0))",
            exp[0], exp[1], exp[2]
        );
    }
    // 頂点数も一致
    assert_eq!(solid.vertices.len(), 8, "1x1x1 cuboid は 8 頂点");
}

// ---------------------------------------------------------------------------
// T05: rotation × Boolean Fuse — Euler-Poincaré 検証
// ---------------------------------------------------------------------------

#[test]
fn t05_rotation_boolean_fuse_manifold() {
    let mut doc = Document::new("fuse_test");

    // 1つ目の cuboid
    doc.root_component.features.push(Feature::CreateBox {
        id: "box1".to_string(),
        width: 2.0,
        height: 2.0,
        depth: 2.0,
    });

    // 2つ目の cuboid (Fuse target)
    doc.root_component.features.push(Feature::CreateBox {
        id: "box2".to_string(),
        width: 2.0,
        height: 2.0,
        depth: 2.0,
    });

    // Fuse
    doc.root_component.features.push(Feature::Fuse {
        id: "fused".to_string(),
        target: "box1".to_string(),
        tool: "box2".to_string(),
    });

    // 回転
    doc.root_component.transform.rotation = [30.0, 45.0, 60.0];

    let mut gen = IdGenerator::new(1);
    let bodies = build_assembly(&doc, std::path::Path::new("."), &mut gen).unwrap();

    assert_eq!(bodies.len(), 1);
    let solid = &bodies[0].solid;

    // Euler-Poincaré: V - E + F - 2S = 0 (単一閉立体の場合)
    assert_eq!(solid.euler_poincare(), 0);
    // 完全な manifold 検証 (Codex F01 対応): 半辺対構造・隣接整合まで確認
    solid
        .validate_manifold()
        .expect("rotation × Boolean Fuse 結果は manifold であるべき");
}

// ---------------------------------------------------------------------------
// T06_boundary_seam — Cylinder 軸回転後の orthonormal_basis 整合 (1e-12)
// ---------------------------------------------------------------------------

#[test]
fn t06_boundary_seam_cylinder_axis_rotation() {
    let mut doc = cylinder_doc("cyl", "c", 2.5, 5.0);
    // rotation=[90,0,0] (X軸90°): Rx(90°) = [[1,0,0],[0,0,-1],[0,1,0]]
    // 元 axis = +Z → 回転後 axis = -Y (Z → -Y)
    doc.root_component.transform.rotation = [90.0, 0.0, 0.0];

    let mut gen = IdGenerator::new(1);
    let bodies = build_assembly(&doc, std::path::Path::new("."), &mut gen).unwrap();

    assert_eq!(bodies.len(), 1);
    let solid = &bodies[0].solid;

    // Cylinder surface を持つ face を探す
    let cyl_axis = solid.faces.iter().find_map(|f| {
        if let mycad_kernel::geometry::surface::Surface::Cylinder { axis, .. } = &f.surface {
            Some(*axis)
        } else {
            None
        }
    });

    assert!(
        cyl_axis.is_some(),
        "Cylinder surface を持つ face が存在する"
    );

    let axis = cyl_axis.unwrap();
    // axis が -Y 軸方向を向いている (snap で exact)
    let neg_y_axis = -Vec3::y();
    let dot = axis.dot(&neg_y_axis);
    assert!(
        (dot - 1.0).abs() < 1e-12,
        "回転後の axis は -Y 方向を向くべき, dot={}",
        dot
    );

    // ADR-007 §2: orthonormal_basis(new_axis) の精度 1e-12 検証 (Codex F01 対応)
    use mycad_kernel::geometry::math::orthonormal_basis;
    let (u, v) = orthonormal_basis(&axis);
    // u, v が単位ベクトルかつ axis と直交、互いに直交
    assert!(
        (u.norm() - 1.0).abs() < 1e-12,
        "orthonormal_basis u は単位ベクトル, |u|={}",
        u.norm()
    );
    assert!(
        (v.norm() - 1.0).abs() < 1e-12,
        "orthonormal_basis v は単位ベクトル, |v|={}",
        v.norm()
    );
    assert!(u.dot(&axis).abs() < 1e-12, "u ⊥ axis, dot={}", u.dot(&axis));
    assert!(v.dot(&axis).abs() < 1e-12, "v ⊥ axis, dot={}", v.dot(&axis));
    assert!(u.dot(&v).abs() < 1e-12, "u ⊥ v, dot={}", u.dot(&v));
}

// ---------------------------------------------------------------------------
// T07_degen_zero_rotation — 全 component rotation=[0,0,0] → translate のみと一致
// ---------------------------------------------------------------------------

#[test]
fn t07_degen_zero_rotation_pure_translate_path() {
    let mut doc = cuboid_doc("R", "b", 2.0, 3.0, 4.0);
    doc.root_component.transform.rotation = [0.0, 0.0, 0.0];
    doc.root_component.transform.position = [10.0, 20.0, 30.0];

    let mut gen = IdGenerator::new(1);
    let bodies = build_assembly(&doc, std::path::Path::new("."), &mut gen).unwrap();

    assert_eq!(bodies.len(), 1);
    let solid = &bodies[0].solid;

    // T03 と同じく、rotation skip 経路では浮動小数ノイズが入らない。
    // 期待頂点集合: 元 (±1, ±1.5, ±2) + position (10,20,30) → exact 一致 (Codex F01 対応)
    let mut expected: Vec<[f64; 3]> = Vec::new();
    for &x in &[-1.0_f64, 1.0] {
        for &y in &[-1.5_f64, 1.5] {
            for &z in &[-2.0_f64, 2.0] {
                expected.push([x + 10.0, y + 20.0, z + 30.0]);
            }
        }
    }
    expected.sort_by(|a, b| a.partial_cmp(b).unwrap());

    let mut actual: Vec<[f64; 3]> = solid
        .vertices
        .iter()
        .map(|v| [v.point.coords.x, v.point.coords.y, v.point.coords.z])
        .collect();
    actual.sort_by(|a, b| a.partial_cmp(b).unwrap());

    assert_eq!(
        actual, expected,
        "rotation=[0,0,0] では IDENTITY3 ガードにより rotate() が呼ばれず exact 一致するべき"
    );
}

// ---------------------------------------------------------------------------
// T08_boundary_nonfinite — rotation に NaN → InvalidParameter
// ---------------------------------------------------------------------------

#[test]
fn t08_boundary_nonfinite_rotation_rejected() {
    let mut doc = cuboid_doc("R", "b", 2.0, 3.0, 4.0);
    doc.root_component.transform.rotation = [f64::NAN, 0.0, 0.0];

    let mut gen = IdGenerator::new(1);
    let result = build_assembly(&doc, std::path::Path::new("."), &mut gen);

    assert!(result.is_err());
    match result {
        Err(mycad_kernel::error::KernelError::InvalidParameter { kind }) => {
            assert_eq!(kind, "transform.rotation");
        }
        _ => panic!("Expected InvalidParameter error"),
    }
}

// ---------------------------------------------------------------------------
// T09: euler_to_matrix が rad シグネチャで Rx(90°) を返す
// ---------------------------------------------------------------------------

#[test]
fn t09_euler_to_matrix_rad_signature_rx_90() {
    // 90° = π/2 rad
    let m = euler_to_matrix(90.0_f64.to_radians(), 0.0, 0.0);
    // Rx(90°) = [[1,0,0],[0,0,-1],[0,1,0]]
    assert_eq!(m[0], [1.0, 0.0, 0.0]);
    assert_eq!(m[1], [0.0, 0.0, -1.0]);
    assert_eq!(m[2], [0.0, 1.0, 0.0]);
}

// ---------------------------------------------------------------------------
// T10_blocked_cut_sphere — rotation × Boolean Cut Sphere (#137 blocked)
// ---------------------------------------------------------------------------

#[test]
#[ignore = "blocked: #137 trimmed sphere tessellation"]
fn t10_blocked_rotation_cut_sphere() {
    // #137 が解決するまで skeleton のみ
    let mut doc = Document::new("cut_sphere");
    doc.root_component.features.push(Feature::CreateSphere {
        id: "sphere".to_string(),
        radius: 5.0,
        center: [0.0, 0.0, 0.0],
    });
    doc.root_component.features.push(Feature::CreateBox {
        id: "box".to_string(),
        width: 6.0,
        height: 6.0,
        depth: 6.0,
    });
    doc.root_component.features.push(Feature::Cut {
        id: "cut".to_string(),
        target: "sphere".to_string(),
        tool: "box".to_string(),
    });
    doc.root_component.transform.rotation = [45.0, 30.0, 60.0];

    let mut gen = IdGenerator::new(1);
    let _ = build_assembly(&doc, std::path::Path::new("."), &mut gen).unwrap();
}
