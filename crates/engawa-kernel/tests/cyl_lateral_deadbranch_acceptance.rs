//! Acceptance tests for Issue #143 — cylinder lateral の adj_is_sphere 死に分岐削除 (ADR-009 Phase 3 即時実施分)
//!
//! テスト計画 ID:
//!   T01 — 決定性: cyl∩sphere intersect の tessellation を 2 回実行し positions/indices/normals が完全一致
//!   T02 — primitive cylinder fallback: arcs_per_rev=1 で n_u==angular_segments
//!   T03 — cyl∩sphere sphere cap regression: 共有境界 watertight (naked edge=0)
//!   T04 — cyl∩cuboid plane cap regression: 共有境界 watertight (naked edge=0)
//!   T_DEG_boundary_primitive_cyl — 退化境界: arcs_per_rev=1 で n_u>=3 が保証される

use engawa_kernel::booleans::{boolean, BooleanOp};
use engawa_kernel::brep::topology::IdGenerator;
use engawa_kernel::geometry::curve::Curve;
use engawa_kernel::geometry::surface::Surface;
use engawa_kernel::geometry::Point;
use engawa_kernel::primitives::{make_cuboid, make_cylinder, make_sphere};
use engawa_kernel::tessellation::{
    tessellate_solid, tessellate_solid_with, TessellationOptions, TriangleMesh,
};

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

/// Build cyl∩sphere intersect: cylinder(r=3, h=20) ∩ sphere(r=4, center=(0,0,10))
fn build_cyl_sphere_intersect() -> engawa_kernel::brep::topology::Solid {
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

/// Build cyl∩cuboid intersect: cuboid(10³) ∩ cylinder(r=2, h=15, origin=(0,0,-7.5))
fn build_cyl_cuboid_intersect() -> engawa_kernel::brep::topology::Solid {
    let mut gen = IdGenerator::new(0);
    let box_solid = make_cuboid(10.0, 10.0, 10.0, "cuboid", &mut gen).unwrap();
    let cyl = make_cylinder(2.0, 15.0, Point::new(0.0, 0.0, -7.5), &mut gen).unwrap();
    boolean(&box_solid, &cyl, BooleanOp::Intersect, &mut gen).expect("cyl intersect cuboid")
}

// ---------------------------------------------------------------------------
// T01: 決定性 — cyl∩sphere intersect の tessellation を 2 回実行し bit-identical
// ---------------------------------------------------------------------------

#[test]
fn t01_determinism_cyl_sphere_intersect() {
    let solid_a = build_cyl_sphere_intersect();
    let mesh_a = tessellate_solid(&solid_a).expect("tessellate A");

    let solid_b = build_cyl_sphere_intersect();
    let mesh_b = tessellate_solid(&solid_b).expect("tessellate B");

    assert_eq!(mesh_a.positions, mesh_b.positions, "T01: positions differ");
    assert_eq!(mesh_a.normals, mesh_b.normals, "T01: normals differ");
    assert_eq!(mesh_a.indices, mesh_b.indices, "T01: indices differ");
}

// ---------------------------------------------------------------------------
// T02: primitive cylinder fallback — arcs_per_rev=1 で n_u==angular_segments
// ---------------------------------------------------------------------------

#[test]
fn t02_primitive_cylinder_uses_angular_segments() {
    let mut gen = IdGenerator::new(0);
    let cyl = make_cylinder(3.0, 10.0, Point::origin(), &mut gen).unwrap();
    let angular_segments = 16;
    let opts = TessellationOptions::new(angular_segments, 2);

    let mesh = tessellate_solid_with(&cyl, &opts).expect("primitive cylinder tessellation");

    // primitive cylinder の頂点数:
    // lateral: (angular_segments + 1) × (axial_segments + 1) = (16+1)×(2+1) = 51
    // bottom cap: angular_segments = 16 (seam vertex は lateral と共有)
    // top cap: angular_segments = 16 (seam vertex は lateral と共有)
    // total: 51 + 16 + 16 = 83
    let expected_vertices = (angular_segments + 1) * (2 + 1) + angular_segments * 2;
    assert_eq!(
        mesh.positions.len(),
        expected_vertices,
        "T02: expected {expected_vertices} vertices for angular_segments={angular_segments}"
    );
    assert!(mesh.triangle_count() > 0, "T02: mesh is empty");
}

// ---------------------------------------------------------------------------
// T03: cyl∩sphere sphere cap regression — 共有境界 watertight (naked edge=0)
// ---------------------------------------------------------------------------

#[test]
fn t03_cyl_sphere_intersect_no_naked_edge() {
    let solid = build_cyl_sphere_intersect();
    let mesh = tessellate_solid(&solid).expect("tessellation");

    // 空メッシュが naked=0 で通り抜ける false positive を防ぐ (Codex F01 指摘)。
    assert!(
        mesh.triangle_count() > 0,
        "T03: mesh has 0 triangles (tessellation collapsed)"
    );

    let naked = count_naked_edges(&mesh, 1e-6);
    assert_eq!(naked, 0, "T03: expected 0 naked edges, got {}", naked);
}

// ---------------------------------------------------------------------------
// T04: cyl∩cuboid plane cap regression — 共有境界 watertight (naked edge=0)
// ---------------------------------------------------------------------------

#[test]
fn t04_cyl_cuboid_intersect_no_naked_edge() {
    let solid = build_cyl_cuboid_intersect();
    let mesh = tessellate_solid(&solid).expect("tessellation");

    // 空メッシュが naked=0 で通り抜ける false positive を防ぐ (Codex F01 指摘)。
    assert!(
        mesh.triangle_count() > 0,
        "T04: mesh has 0 triangles (tessellation collapsed)"
    );

    let naked = count_naked_edges(&mesh, 1e-6);
    assert_eq!(naked, 0, "T04: expected 0 naked edges, got {}", naked);
}

// ---------------------------------------------------------------------------
// T_DEG: 退化境界 — arcs_per_rev=1 で n_u>=3 が保証される
// ---------------------------------------------------------------------------

#[test]
fn t_deg_boundary_primitive_cyl_min_angular_segments() {
    let mut gen = IdGenerator::new(0);
    let cyl = make_cylinder(3.0, 10.0, Point::origin(), &mut gen).unwrap();

    // 構造体リテラルで angular_segments=1 を直接渡す。
    // TessellationOptions::new() は呼び出し時点で .max(3) clamp するため、
    // tessellate_face_uv_grid 側の opts.angular_segments.max(3) を実際に検証するには
    // コンストラクタを経由してはいけない (Codex F02 指摘)。
    let opts = TessellationOptions {
        angular_segments: 1,
        axial_segments: 2,
    };
    let mesh = tessellate_solid_with(&cyl, &opts).expect("min angular_segments tessellation");

    // n_u は max(3) で 3 に補正される。primitive cylinder の頂点内訳:
    //   lateral: (n_u + 1) × (n_v + 1) = 4 × 3 = 12
    //   bottom cap: n_u = 3
    //   top cap:    n_u = 3
    //   total:      18
    assert_eq!(
        mesh.positions.len(),
        18,
        "T_DEG: expected exactly 18 vertices (n_u=3 contract), got {}",
        mesh.positions.len()
    );
    assert!(mesh.triangle_count() > 0, "T_DEG: mesh is empty");
}
