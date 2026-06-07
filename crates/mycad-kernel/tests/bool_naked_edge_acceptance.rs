// Acceptance tests for Issue #60:
// Boolean後メッシュに naked edge = 0 バリデーションを追加する。
// naked edge = 隣接三角形が 1 枚しかない辺（メッシュに穴がある証拠）。

use mycad_kernel::booleans::{boolean, BooleanOp};
use mycad_kernel::brep::topology::IdGenerator;
use mycad_kernel::geometry::curve::Curve;
use mycad_kernel::geometry::surface::Surface;
use mycad_kernel::geometry::Point;
use mycad_kernel::primitives::{make_cuboid, make_cylinder, make_sphere};
use mycad_kernel::tessellation::{tessellate_solid, TriangleMesh};

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
        // Skip the entire triangle if any two quantized vertices coincide.
        // A degenerate (A,B,A) triangle would otherwise count edge AB twice,
        // making it look shared (count=2) and hiding the exposed boundary.
        if q0 == q1 || q1 == q2 || q2 == q0 {
            continue;
        }
        for [qa, qb] in &[[q0, q1], [q1, q2], [q2, q0]] {
            let key = if qa <= qb { [*qa, *qb] } else { [*qb, *qa] };
            *edge_count.entry(key).or_insert(0) += 1;
        }
    }

    edge_count.values().filter(|&&c| c == 1).count()
}

fn make_box_minus_sphere(gen: &mut IdGenerator) -> mycad_kernel::brep::topology::Solid {
    let box_solid = make_cuboid(10.0, 10.0, 10.0, gen).expect("cuboid");
    let sphere = make_sphere(3.0, Point::origin(), gen).expect("sphere");
    boolean(&box_solid, &sphere, BooleanOp::Cut, gen).expect("box - sphere cut")
}

fn make_intersect_cyl_sphere() -> mycad_kernel::brep::topology::Solid {
    let mut gen = IdGenerator::new(0);
    let cyl = make_cylinder(3.0, 20.0, Point::origin(), &mut gen).unwrap();
    let mut sph = make_sphere(4.0, Point::origin(), &mut gen).unwrap();
    // sphere を z=10 に平行移動
    for v in &mut sph.vertices {
        v.point.coords.z += 10.0;
    }
    for e in &mut sph.edges {
        if let Curve::Circle { center, .. } = &mut e.curve {
            center.coords.z += 10.0;
        }
    }
    for f in &mut sph.faces {
        match &mut f.surface {
            Surface::Sphere { center, .. }
            | Surface::Plane { origin: center, .. }
            | Surface::Cylinder { origin: center, .. } => {
                center.coords.z += 10.0;
            }
            _ => {}
        }
    }
    boolean(&cyl, &sph, BooleanOp::Intersect, &mut gen).expect("cyl intersect sphere")
}

/// T01: 決定性 — box(10³)−sphere(r=3,origin) Cut を 2 回実行しメッシュが byte-identical。
#[test]
fn t01_determinism() {
    let solid_a = make_box_minus_sphere(&mut IdGenerator::new(0));
    let mesh_a = tessellate_solid(&solid_a).expect("tessellate A");

    let solid_b = make_box_minus_sphere(&mut IdGenerator::new(0));
    let mesh_b = tessellate_solid(&solid_b).expect("tessellate B");

    assert_eq!(
        mesh_a.positions, mesh_b.positions,
        "positions must be identical"
    );
    assert_eq!(mesh_a.normals, mesh_b.normals, "normals must be identical");
    assert_eq!(mesh_a.indices, mesh_b.indices, "indices must be identical");
}

/// T02: box(10³)−sphere(r=3, origin) Cut の naked_edge = 0。
#[test]
fn t02_box_sphere_void_naked_edge() {
    let solid = make_box_minus_sphere(&mut IdGenerator::new(0));
    let mesh = tessellate_solid(&solid).expect("tessellate");
    assert!(
        mesh.triangle_count() > 0,
        "T02: mesh must be non-empty (guards against silent empty-mesh regression)"
    );
    let naked = count_naked_edges(&mesh, 1e-10);
    assert_eq!(
        naked, 0,
        "box(10³) - sphere(r=3, origin) should have zero naked edges"
    );
}

/// T03: cylinder(r=3,h=20)∩sphere(r=4,center=(0,0,10)) の naked_edge = 0。
#[test]
fn t03_cyl_sph_intersect_naked_edge() {
    let solid = make_intersect_cyl_sphere();
    let mesh = tessellate_solid(&solid).expect("tessellate");
    assert!(
        mesh.triangle_count() > 0,
        "T03: mesh must be non-empty (guards against silent empty-mesh regression)"
    );
    let naked = count_naked_edges(&mesh, 1e-10);
    assert_eq!(
        naked, 0,
        "cyl(r=3,h=20) ∩ sphere(r=4,c=(0,0,10)) should have zero naked edges"
    );
}

/// T04: box(10³)−cylinder(r=2,h=6) Cut は naked_edge > 0 の既知問題。
/// seam vertex mismatch (#56 out-of-scope) のため ignore。
#[test]
#[ignore = "known seam mismatch: tracked separately"]
fn t04_boundary_degen_cut_cyl() {
    let mut gen = IdGenerator::new(0);
    let box_solid = make_cuboid(10.0, 10.0, 10.0, &mut gen).expect("cuboid");
    let cyl = make_cylinder(2.0, 6.0, Point::origin(), &mut gen).expect("cylinder");
    let result = boolean(&box_solid, &cyl, BooleanOp::Cut, &mut gen).expect("cut");
    let mesh = tessellate_solid(&result).expect("tessellate");
    let naked = count_naked_edges(&mesh, 1e-10);
    assert_eq!(
        naked, 0,
        "box - cylinder cut should have zero naked edges (currently known to fail)"
    );
}

// ── Edge case / boundary / degenerate tests ──────────────────────────

/// T05: 空メッシュ（positions = [], indices = []）の場合 count_naked_edges = 0。
#[test]
fn t05_empty_mesh_naked_edge() {
    let empty = TriangleMesh::new();
    assert_eq!(
        count_naked_edges(&empty, 1e-10),
        0,
        "empty mesh should have zero naked edges"
    );
}

/// T06: 決定性 100回 — box(10³)−sphere(r=3,origin) Cut を 100 回実行し全メッシュが同一。
#[test]
fn t06_determinism_100_runs() {
    let first_solid = make_box_minus_sphere(&mut IdGenerator::new(0));
    let first_mesh = tessellate_solid(&first_solid).expect("tessellate first");

    for run in 1..100 {
        let solid = make_box_minus_sphere(&mut IdGenerator::new(0));
        let mesh = tessellate_solid(&solid).unwrap_or_else(|e| panic!("run {run}: {e}"));
        assert_eq!(
            first_mesh.positions, mesh.positions,
            "positions differ at run {run}"
        );
        assert_eq!(
            first_mesh.normals, mesh.normals,
            "normals differ at run {run}"
        );
        assert_eq!(
            first_mesh.indices, mesh.indices,
            "indices differ at run {run}"
        );
    }
}

/// T07: 退化三角形（全頂点同一位置）が naked edge カウントから除外されること。
#[test]
fn t07_degenerate_triangle_no_naked_edge() {
    let mesh = TriangleMesh {
        positions: vec![[0.0, 0.0, 0.0]; 3],
        normals: vec![[0.0, 0.0, 1.0]; 3],
        indices: vec![0, 1, 2],
        face_ids: vec!["".to_string()],
    };
    // 全頂点が同一 → i0==i1 after welding → degenerate → skip
    assert_eq!(
        count_naked_edges(&mesh, 1e-10),
        0,
        "degenerate triangles should contribute zero naked edges"
    );
}

/// T08: -0.0 と +0.0 が同じ位置としてウェルドされること。
#[test]
fn t08_negative_zero_welding() {
    let mesh = TriangleMesh {
        positions: vec![[-0.0, -0.0, -0.0], [0.0, 0.0, 0.0], [1.0, 0.0, 0.0]],
        normals: vec![[0.0, 0.0, 1.0]; 3],
        indices: vec![0, 1, 2],
        face_ids: vec!["".to_string()],
    };
    // v0 == v1 after quantization → whole triangle skipped → 0 naked edges.
    assert_eq!(
        count_naked_edges(&mesh, 1e-10),
        0,
        "-0.0 and +0.0 weld to same grid cell; degenerate triangle is fully skipped"
    );
}

/// T09: tol=1e-10 境界付近のウェルディング精度確認。
/// 1e-11 離れた頂点はウェルドされ、共有エッジは naked にならない。
#[test]
fn t09_near_boundary_welding() {
    let mesh = TriangleMesh {
        positions: vec![
            [0.0, 0.0, 0.0],   // 0
            [1.0, 0.0, 0.0],   // 1
            [0.0, 1.0, 0.0],   // 2
            [0.0, 0.0, 1e-11], // 3 — within tol=1e-10 of vertex 0
            [1.0, 0.0, 1e-11], // 4 — within tol=1e-10 of vertex 1
            [1.0, 1.0, 0.0],   // 5
        ],
        normals: vec![[0.0, 0.0, 1.0]; 6],
        indices: vec![0, 1, 2, 3, 4, 5],
        face_ids: vec!["".to_string(), "".to_string()],
    };
    // After welding: 3→0, 4→1
    // Triangles: (0,1,2) and (0,1,5) share edge (0,1) → count 2
    // Boundary edges: (0,2),(1,2),(0,5),(1,5) → count 1 each → 4 naked edges
    assert_eq!(
        count_naked_edges(&mesh, 1e-10),
        4,
        "near-boundary vertices should be welded, leaving 4 boundary edges"
    );
}
