// Acceptance tests for Issue #130:
// トリム曲面のテッセレーション実装 — tessellate_trimmed_uv_face
// 対象: boolean_box_cut (円筒部分スパン), boolean_box_void (球面 inner_loop),
//       boolean_cut_sphere_dimple (球面 inner_loop) の naked_edge = 0 検証。

use engawa_kernel::booleans::{boolean, BooleanOp};
use engawa_kernel::brep::topology::IdGenerator;
use engawa_kernel::geometry::Point;
use engawa_kernel::primitives::{make_cuboid, make_cylinder, make_sphere};
use engawa_kernel::tessellation::{tessellate_solid, TriangleMesh};

/// 位置ベースの整数グリッド量子化で頂点ウェルディング後、
/// naked edge（隣接三角形が 1 枚のみの辺）数を返す。
fn count_naked_edges(mesh: &TriangleMesh, tol: f64) -> usize {
    let quantize = |p: &[f64; 3]| -> [i64; 3] {
        [
            (p[0] / tol).round() as i64,
            (p[1] / tol).round() as i64,
            (p[2] / tol).round() as i64,
        ]
    };
    let qpos: Vec<[i64; 3]> = mesh.positions.iter().map(quantize).collect();
    let tri_count = mesh.indices.len() / 3;
    let mut edge_count: std::collections::HashMap<[[i64; 3]; 2], usize> =
        std::collections::HashMap::new();
    for tri in 0..tri_count {
        let i0 = mesh.indices[tri * 3] as usize;
        let i1 = mesh.indices[tri * 3 + 1] as usize;
        let i2 = mesh.indices[tri * 3 + 2] as usize;
        let q0 = qpos[i0];
        let q1 = qpos[i1];
        let q2 = qpos[i2];
        let mut seen_in_tri: Vec<[[i64; 3]; 2]> = Vec::with_capacity(3);
        for [qa, qb] in &[[q0, q1], [q1, q2], [q2, q0]] {
            if qa == qb {
                continue;
            }
            let key = if qa <= qb { [*qa, *qb] } else { [*qb, *qa] };
            if seen_in_tri.contains(&key) {
                continue;
            }
            seen_in_tri.push(key);
            *edge_count.entry(key).or_insert(0) += 1;
        }
    }
    edge_count.values().filter(|&&c| c == 1).count()
}

// ── T01: 決定性 ─────────────────────────────────────────────────────────────

/// T01: boolean_box_cut を 2 回実行し全座標・法線・インデックスが一致。
#[test]
fn t01_box_cut_determinism() {
    let build = || {
        let mut gen = IdGenerator::new(0);
        let box_solid = make_cuboid(4.0, 4.0, 4.0, "cuboid", &mut gen).expect("cuboid");
        let cyl = make_cylinder(1.0, 6.0, Point::new(0.0, 0.0, -3.0), &mut gen).expect("cylinder");
        let result = boolean(&box_solid, &cyl, BooleanOp::Cut, &mut gen).expect("cut");
        tessellate_solid(&result).expect("tessellate")
    };
    let m1 = build();
    let m2 = build();
    assert_eq!(m1.positions, m2.positions, "positions must be identical");
    assert_eq!(m1.normals, m2.normals, "normals must be identical");
    assert_eq!(m1.indices, m2.indices, "indices must be identical");
}

// ── T02–T04: 正常系 naked_edge = 0 ──────────────────────────────────────────

/// T02: box(4³) − cylinder(r=1, h=6) の naked_edge = 0。
/// 円筒側面が部分円スパンのトリム面。
#[test]
fn t02_box_cut_naked_edge_zero() {
    let mut gen = IdGenerator::new(0);
    let box_solid = make_cuboid(4.0, 4.0, 4.0, "cuboid", &mut gen).expect("cuboid");
    let cyl = make_cylinder(1.0, 6.0, Point::new(0.0, 0.0, -3.0), &mut gen).expect("cylinder");
    let result = boolean(&box_solid, &cyl, BooleanOp::Cut, &mut gen).expect("cut");
    let mesh = tessellate_solid(&result).expect("tessellate");
    assert!(mesh.triangle_count() > 0, "T02: mesh must be non-empty");
    let naked = count_naked_edges(&mesh, 1e-10);
    assert_eq!(
        naked, 0,
        "box(4³) - cylinder(r=1,h=6) should have zero naked edges"
    );
}

/// T03: box(6³) − sphere(r=2, center=(0,0,3.5)) の naked_edge = 0。
/// 球面が inner_loop 付き（box の上面で切られたディンプル）。
#[test]
fn t03_box_void_naked_edge_zero() {
    let mut gen = IdGenerator::new(0);
    let box_solid = make_cuboid(6.0, 6.0, 6.0, "cuboid", &mut gen).expect("cuboid");
    let sphere = make_sphere(2.0, Point::new(0.0, 0.0, 3.5), &mut gen).expect("sphere");
    let result = boolean(&box_solid, &sphere, BooleanOp::Cut, &mut gen).expect("cut");
    let mesh = tessellate_solid(&result).expect("tessellate");
    assert!(mesh.triangle_count() > 0, "T03: mesh must be non-empty");
    let naked = count_naked_edges(&mesh, 1e-10);
    assert_eq!(
        naked, 0,
        "box(6³) - sphere(r=2,c=(0,0,3.5)) should have zero naked edges"
    );
}

/// T04: box(10³) − sphere(r=3, center=(0,0,6)) の naked_edge = 0。
/// 球面キャップのトリム（sphere の一部が box 上面に突き出る）。
#[test]
fn t04_sphere_dimple_naked_edge_zero() {
    let mut gen = IdGenerator::new(0);
    let box_solid = make_cuboid(10.0, 10.0, 10.0, "cuboid", &mut gen).expect("cuboid");
    let sphere = make_sphere(3.0, Point::new(0.0, 0.0, 6.0), &mut gen).expect("sphere");
    let result = boolean(&box_solid, &sphere, BooleanOp::Cut, &mut gen).expect("cut");
    let mesh = tessellate_solid(&result).expect("tessellate");
    assert!(mesh.triangle_count() > 0, "T04: mesh must be non-empty");
    let naked = count_naked_edges(&mesh, 1e-10);
    assert_eq!(
        naked, 0,
        "box(10³) - sphere(r=3,c=(0,0,6)) should have zero naked edges"
    );
}

// ── T07: 退化/境界ケース ──────────────────────────────────────────────────

/// T07_degen_inner: inner_loop が極めて小さい（3 点相当）場合でもクラッシュしない。
/// earcut が退化三角形を出すことがあるが、panic/Err でなく正常終了すること。
#[test]
fn t07_degen_inner_small_loop_no_panic() {
    let mut gen = IdGenerator::new(0);
    let box_solid = make_cuboid(10.0, 10.0, 10.0, "cuboid", &mut gen).expect("cuboid");
    // Very small sphere that barely clips the top face → tiny inner_loop
    let sphere = make_sphere(0.05, Point::new(0.0, 0.0, 6.0), &mut gen).expect("sphere");
    // Main contract: no panic. Boolean may succeed or fail for degenerate overlap.
    let result = boolean(&box_solid, &sphere, BooleanOp::Cut, &mut gen);
    match result {
        Ok(solid) => {
            let mesh = tessellate_solid(&solid);
            assert!(mesh.is_ok(), "tessellate should not error on tiny dimple");
            let mesh = mesh.expect("mesh");
            // At minimum the box should produce some triangles
            assert!(
                mesh.triangle_count() > 0,
                "tessellated box should have at least some triangles"
            );
        }
        Err(_) => {
            // Boolean itself may fail for degenerate overlap — acceptable
        }
    }
}

/// T08_boundary_seam: seam を跨ぐ円筒面（u = 0/2π 境界にエッジが存在する）で
/// UV アンラップが正しく機能し naked_edge = 0 になること。
/// boolean_box_cut がこのケースに相当する（seam 付近に line HE がある）。
#[test]
fn t08_boundary_seam_cylinder_unwrap() {
    let mut gen = IdGenerator::new(0);
    let box_solid = make_cuboid(4.0, 4.0, 4.0, "cuboid", &mut gen).expect("cuboid");
    let cyl = make_cylinder(1.0, 6.0, Point::new(0.0, 0.0, -3.0), &mut gen).expect("cyl");
    let result = boolean(&box_solid, &cyl, BooleanOp::Cut, &mut gen).expect("cut");
    let mesh = tessellate_solid(&result).expect("tessellate");

    // Confirm no naked edges (seam line HE should not break UV continuity)
    let naked = count_naked_edges(&mesh, 1e-6);
    assert_eq!(
        naked, 0,
        "seam cylinder should have zero naked edges after UV unwrap"
    );
    // Confirm triangle count is plausible (at least top + bottom caps + cylindrical side)
    assert!(
        mesh.triangle_count() >= 32 * 2,
        "should have cylinder ring triangles, got {}",
        mesh.triangle_count()
    );
}
