//! Acceptance tests for #137: trimmed sphere tessellation の circ_normal 一般化

use mycad_kernel::booleans::{boolean, BooleanOp};
use mycad_kernel::brep::topology::IdGenerator;
use mycad_kernel::geometry::{curve::Curve, surface::Surface, Point, Vec3};
use mycad_kernel::primitives::{make_cuboid, make_sphere};
use mycad_kernel::tessellation::{tessellate_solid, TessellationError, TriangleMesh};
use mycad_kernel::LENGTH_TOLERANCE;
use std::f64::consts::PI;

/// 位置ベースの整数グリッド量化で頂点ウェルディング後、
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

/// T01: 既存の boolean_cut_sphere_dimple (box×sphere, 軸 +Z) を 2 回 tessellate し、
/// メッシュが完全一致 (positions / indices / normals) することを確認する。
#[test]
fn t01_determinism_axis_z() {
    let build = || {
        let mut gen = IdGenerator::new(0);
        let box_solid = make_cuboid(10.0, 10.0, 10.0, &mut gen).expect("cuboid");
        let sphere = make_sphere(3.0, Point::new(0.0, 0.0, 6.0), &mut gen).expect("sphere");
        let result = boolean(&box_solid, &sphere, BooleanOp::Cut, &mut gen).expect("cut");
        tessellate_solid(&result).expect("tessellate")
    };
    let m1 = build();
    let m2 = build();
    assert_eq!(m1.positions, m2.positions, "positions must be identical");
    assert_eq!(m1.normals, m2.normals, "normals must be identical");
    assert_eq!(m1.indices, m2.indices, "indices must be identical");
}

/// T02: 既存の boolean_cut_sphere_dimple を改修後も
///   - naked_edge カウントが 0 である
/// ことを確認する (回帰テスト)。
#[test]
fn t02_watertight_axis_z() {
    let mut gen = IdGenerator::new(0);
    let box_solid = make_cuboid(10.0, 10.0, 10.0, &mut gen).expect("cuboid");
    let sphere = make_sphere(3.0, Point::new(0.0, 0.0, 6.0), &mut gen).expect("sphere");
    let result = boolean(&box_solid, &sphere, BooleanOp::Cut, &mut gen).expect("cut");
    let mesh = tessellate_solid(&result).expect("tessellate");
    assert!(mesh.triangle_count() > 0, "mesh must be non-empty");
    let naked = count_naked_edges(&mesh, 1e-10);
    assert_eq!(
        naked, 0,
        "box(10³) - sphere(r=3,c=(0,0,6)) should have zero naked edges after axis refactor"
    );
}

/// T03: 任意軸 circ_normal (+X 方向) を持つ trim circle で tessellate が正しく動作する。
#[test]
fn t03_arbitrary_axis_circ_normal() {
    use mycad_kernel::brep::topology::Solid;

    // R=5 の球を原点中心に構築
    let radius = 5.0;
    let center = Point::origin();
    let mut gen = IdGenerator::new(0);
    let mut solid = Solid::new(gen.next());

    // 球の外殻 (full sphere) を作成
    let v_south = solid.add_vertex(gen.next(), center + Vec3::new(0.0, 0.0, -radius), None);
    let v_north = solid.add_vertex(gen.next(), center + Vec3::new(0.0, 0.0, radius), None);

    let e_seam = solid.add_edge(
        gen.next(),
        [v_south, v_north],
        Curve::Circle {
            center,
            normal: -Vec3::y(),
            radius,
        },
        [PI, 2.0 * PI],
        None,
    );

    let he_up = solid.add_half_edge(gen.next(), v_south, e_seam, true);
    let he_down = solid.add_half_edge(gen.next(), v_north, e_seam, false);
    let outer_loop = solid.add_loop(gen.next(), vec![he_up, he_down]);

    // ここで inner_loop を追加: +X 方向を normal とする circle
    // signed_offset = 3.0 (X 方向), circ_radius = sqrt(5² - 3²) = 4.0
    let signed_offset = 3.0;
    let circ_radius = (radius * radius - signed_offset * signed_offset).sqrt();
    let circ_normal = Vec3::new(1.0, 0.0, 0.0); // +X 方向
    let circ_center = center + signed_offset * circ_normal; // (3, 0, 0)

    // F02: inner_loop を 1 edge full-sweep に構築 (周期エッジ)
    // 円周上の 1 点 (θ=0 の位置) を周期エッジ用の頂点として作成
    let loop_vertex = solid.add_vertex(
        gen.next(),
        circ_center + circ_radius * Vec3::y(), // θ=0 → Y 方向
        None,
    );

    // 1 edge full-sweep: t_range = [0, 2π]、start_vertex == end_vertex
    let loop_edge = solid.add_edge(
        gen.next(),
        [loop_vertex, loop_vertex], // 周期エッジ: start = end
        Curve::Circle {
            center: circ_center,
            normal: circ_normal,
            radius: circ_radius,
        },
        [0.0, 2.0 * PI],
        None,
    );
    let loop_he = solid.add_half_edge(gen.next(), loop_vertex, loop_edge, true);
    let inner_loop = solid.add_loop(gen.next(), vec![loop_he]);

    let face = solid.add_face(
        gen.next(),
        Surface::Sphere { center, radius },
        outer_loop,
        vec![inner_loop],
        true,
        None,
    );

    solid.add_shell(gen.next(), vec![face], true);

    // tessellate 実行
    let mut mesh = TriangleMesh::new();
    let opts = mycad_kernel::tessellation::TessellationOptions {
        angular_segments: 16,
        ..Default::default()
    };

    let result = mycad_kernel::tessellation::tessellate_sphere_face_trimmed(
        &solid,
        &solid.faces[face],
        face,
        &opts,
        &mut mesh,
        "",
    );

    assert!(result.is_ok(), "tessellation should succeed: {:?}", result);

    // 全頂点が球面上にある
    for &pos in &mesh.positions {
        let p = Point::new(pos[0], pos[1], pos[2]);
        let dist = (p - center).norm();
        assert!(
            (dist - radius).abs() <= LENGTH_TOLERANCE,
            "vertex not on sphere surface: dist={dist}, expected={radius}"
        );
    }

    // 極頂点が circ_normal 方向にある
    let expected_pole = center + radius * circ_normal.normalize();
    let pole_idx = mesh.positions.len() - 1;
    let pole_pos = Point::new(
        mesh.positions[pole_idx][0],
        mesh.positions[pole_idx][1],
        mesh.positions[pole_idx][2],
    );
    assert!(
        (pole_pos - expected_pole).norm() <= LENGTH_TOLERANCE,
        "pole should be at circ_normal direction"
    );

    // 三角形が生成されている
    assert!(mesh.triangle_count() > 0, "mesh should have triangles");
}

/// T_boundary_tangent_circle: 接円 (signed_offset == sphere_radius) のメッシュを確認。
/// 現実装では v_lat = ±π/2 となり、境界 ring と極が同一点に縮退し、三角形 0 件のメッシュになる。
#[test]
fn t_boundary_tangent_circle_produces_empty_mesh() {
    use mycad_kernel::brep::topology::Solid;

    let radius = 5.0;
    let center = Point::origin();
    let mut gen = IdGenerator::new(0);
    let mut solid = Solid::new(gen.next());

    // 球の外殻
    let v_south = solid.add_vertex(gen.next(), center + Vec3::new(0.0, 0.0, -radius), None);
    let v_north = solid.add_vertex(gen.next(), center + Vec3::new(0.0, 0.0, radius), None);

    let e_seam = solid.add_edge(
        gen.next(),
        [v_south, v_north],
        Curve::Circle {
            center,
            normal: -Vec3::y(),
            radius,
        },
        [PI, 2.0 * PI],
        None,
    );

    let he_up = solid.add_half_edge(gen.next(), v_south, e_seam, true);
    let he_down = solid.add_half_edge(gen.next(), v_north, e_seam, false);
    let outer_loop = solid.add_loop(gen.next(), vec![he_up, he_down]);

    // 接円: signed_offset = radius (circ_radius = 0)
    let signed_offset = radius;
    let circ_normal = Vec3::new(0.0, 0.0, 1.0); // +Z
    let circ_center = center + signed_offset * circ_normal; // (0, 0, R) = 北極

    // circ_radius は 0 になる (接円)
    let circ_radius = 0.0;

    // inner_loop 用頂点: 北極と同じ位置に複数頂点を作成
    let n_boundary = 4;
    let mut inner_vertices = Vec::new();
    for _ in 0..n_boundary {
        inner_vertices.push(solid.add_vertex(gen.next(), circ_center, None));
    }

    let mut inner_half_edges = Vec::new();
    for k in 0..n_boundary {
        let v_start = inner_vertices[k];
        let v_end = inner_vertices[(k + 1) % n_boundary];
        let edge = solid.add_edge(
            gen.next(),
            [v_start, v_end],
            Curve::Circle {
                center: circ_center,
                normal: circ_normal,
                radius: circ_radius,
            },
            [0.0, 2.0 * PI],
            None,
        );
        let he = solid.add_half_edge(gen.next(), v_start, edge, true);
        inner_half_edges.push(he);
    }

    let inner_loop = solid.add_loop(gen.next(), inner_half_edges);

    let face = solid.add_face(
        gen.next(),
        Surface::Sphere { center, radius },
        outer_loop,
        vec![inner_loop],
        true,
        None,
    );

    solid.add_shell(gen.next(), vec![face], true);

    let mut mesh = TriangleMesh::new();
    let opts = mycad_kernel::tessellation::TessellationOptions {
        angular_segments: 16,
        ..Default::default()
    };

    let result = mycad_kernel::tessellation::tessellate_sphere_face_trimmed(
        &solid,
        &solid.faces[face],
        face,
        &opts,
        &mut mesh,
        "",
    );

    assert!(
        result.is_ok(),
        "tangent circle should succeed: {:?}",
        result
    );

    // 接円ケースでは極と境界 ring が同一位置に縮退し、三角形 0 件のメッシュになる
    assert_eq!(
        mesh.triangle_count(),
        0,
        "tangent circle produces empty mesh"
    );
}

/// T_boundary_great_circle: 大円 (signed_offset == 0, circ_radius == sphere_radius) で半球メッシュ。
#[test]
fn t_boundary_great_circle_axis_z() {
    use mycad_kernel::brep::topology::Solid;

    let radius = 5.0;
    let center = Point::origin();
    let mut gen = IdGenerator::new(0);
    let mut solid = Solid::new(gen.next());

    // 球の外殻
    let v_south = solid.add_vertex(gen.next(), center + Vec3::new(0.0, 0.0, -radius), None);
    let v_north = solid.add_vertex(gen.next(), center + Vec3::new(0.0, 0.0, radius), None);

    let e_seam = solid.add_edge(
        gen.next(),
        [v_south, v_north],
        Curve::Circle {
            center,
            normal: -Vec3::y(),
            radius,
        },
        [PI, 2.0 * PI],
        None,
    );

    let he_up = solid.add_half_edge(gen.next(), v_south, e_seam, true);
    let he_down = solid.add_half_edge(gen.next(), v_north, e_seam, false);
    let outer_loop = solid.add_loop(gen.next(), vec![he_up, he_down]);

    // 大円: signed_offset = 0 (circ_center = sphere_center), circ_radius = sphere_radius
    let _signed_offset = 0.0;
    let circ_normal = Vec3::new(0.0, 0.0, 1.0); // +Z
    let circ_center = center; // 大円は球中心を通る
    let circ_radius = radius;

    // F02: inner_loop を 1 edge full-sweep に構築 (周期エッジ)
    // 大円上の 1 点 (θ=0 の位置、X 軸方向)
    let loop_vertex = solid.add_vertex(gen.next(), center + circ_radius * Vec3::x(), None);

    // 1 edge full-sweep: t_range = [0, 2π]、start_vertex == end_vertex
    let loop_edge = solid.add_edge(
        gen.next(),
        [loop_vertex, loop_vertex], // 周期エッジ: start = end
        Curve::Circle {
            center: circ_center,
            normal: circ_normal,
            radius: circ_radius,
        },
        [0.0, 2.0 * PI],
        None,
    );
    let loop_he = solid.add_half_edge(gen.next(), loop_vertex, loop_edge, true);
    let inner_loop = solid.add_loop(gen.next(), vec![loop_he]);

    let face = solid.add_face(
        gen.next(),
        Surface::Sphere { center, radius },
        outer_loop,
        vec![inner_loop],
        true,
        None,
    );

    solid.add_shell(gen.next(), vec![face], true);

    let mut mesh = TriangleMesh::new();
    let opts = mycad_kernel::tessellation::TessellationOptions {
        angular_segments: 16,
        ..Default::default()
    };

    let result = mycad_kernel::tessellation::tessellate_sphere_face_trimmed(
        &solid,
        &solid.faces[face],
        face,
        &opts,
        &mut mesh,
        "",
    );

    assert!(result.is_ok(), "great circle should succeed: {:?}", result);

    // 全頂点が球面上
    for &pos in &mesh.positions {
        let p = Point::new(pos[0], pos[1], pos[2]);
        let dist = (p - center).norm();
        assert!(
            (dist - radius).abs() <= LENGTH_TOLERANCE,
            "vertex not on sphere: dist={dist}"
        );
    }

    // 極は (0, 0, ±R) のどちらか
    let pole_idx = mesh.positions.len() - 1;
    let pole_pos = Point::new(
        mesh.positions[pole_idx][0],
        mesh.positions[pole_idx][1],
        mesh.positions[pole_idx][2],
    );
    let north_pole = center + radius * Vec3::z();
    let south_pole = center - radius * Vec3::z();
    let is_north = (pole_pos - north_pole).norm() <= LENGTH_TOLERANCE;
    let is_south = (pole_pos - south_pole).norm() <= LENGTH_TOLERANCE;
    assert!(
        is_north || is_south,
        "pole should be at north or south pole"
    );

    // 半球らしい三角形数 (大円で半分に分割されるので、full sphere の約半分)
    assert!(
        mesh.triangle_count() > 0,
        "hemisphere should have triangles"
    );
}

/// T_degen_out_of_sphere: signed_offset > sphere_radius + LENGTH_TOLERANCE で InvalidTrimCircle。
#[test]
fn t_degen_signed_offset_exceeds_radius_returns_error() {
    use mycad_kernel::brep::topology::Solid;

    let radius = 5.0;
    let center = Point::origin();
    let mut gen = IdGenerator::new(0);
    let mut solid = Solid::new(gen.next());

    // 球の外殻
    let v_south = solid.add_vertex(gen.next(), center + Vec3::new(0.0, 0.0, -radius), None);
    let v_north = solid.add_vertex(gen.next(), center + Vec3::new(0.0, 0.0, radius), None);

    let e_seam = solid.add_edge(
        gen.next(),
        [v_south, v_north],
        Curve::Circle {
            center,
            normal: -Vec3::y(),
            radius,
        },
        [PI, 2.0 * PI],
        None,
    );

    let he_up = solid.add_half_edge(gen.next(), v_south, e_seam, true);
    let he_down = solid.add_half_edge(gen.next(), v_north, e_seam, false);
    let outer_loop = solid.add_loop(gen.next(), vec![he_up, he_down]);

    // 球外円: signed_offset = 10 > radius + LENGTH_TOLERANCE
    let signed_offset = 10.0;
    let circ_normal = Vec3::new(1.0, 0.0, 0.0); // +X
    let circ_center = center + signed_offset * circ_normal; // (10, 0, 0)
    let circ_radius = 4.0; // 値は何でもよい (球と接しない)

    // F02: inner_loop を 1 edge full-sweep に構築 (周期エッジ)
    let loop_vertex = solid.add_vertex(gen.next(), circ_center + circ_radius * Vec3::y(), None);

    let loop_edge = solid.add_edge(
        gen.next(),
        [loop_vertex, loop_vertex], // 周期エッジ: start = end
        Curve::Circle {
            center: circ_center,
            normal: circ_normal,
            radius: circ_radius,
        },
        [0.0, 2.0 * PI],
        None,
    );
    let loop_he = solid.add_half_edge(gen.next(), loop_vertex, loop_edge, true);
    let inner_loop = solid.add_loop(gen.next(), vec![loop_he]);

    let face = solid.add_face(
        gen.next(),
        Surface::Sphere { center, radius },
        outer_loop,
        vec![inner_loop],
        true,
        None,
    );

    solid.add_shell(gen.next(), vec![face], true);

    let mut mesh = TriangleMesh::new();
    let opts = mycad_kernel::tessellation::TessellationOptions {
        angular_segments: 16,
        ..Default::default()
    };

    let result = mycad_kernel::tessellation::tessellate_sphere_face_trimmed(
        &solid,
        &solid.faces[face],
        face,
        &opts,
        &mut mesh,
        "",
    );

    assert!(
        matches!(result, Err(TessellationError::InvalidTrimCircle { .. })),
        "out-of-sphere circle should return InvalidTrimCircle"
    );
}

/// T_degen_zero_axis: circ_normal = ゼロベクトルで InvalidTrimCircle。
#[test]
fn t_degen_zero_circ_normal_returns_error() {
    use mycad_kernel::brep::topology::Solid;

    let radius = 5.0;
    let center = Point::origin();
    let mut gen = IdGenerator::new(0);
    let mut solid = Solid::new(gen.next());

    // 球の外殻
    let v_south = solid.add_vertex(gen.next(), center + Vec3::new(0.0, 0.0, -radius), None);
    let v_north = solid.add_vertex(gen.next(), center + Vec3::new(0.0, 0.0, radius), None);

    let e_seam = solid.add_edge(
        gen.next(),
        [v_south, v_north],
        Curve::Circle {
            center,
            normal: -Vec3::y(),
            radius,
        },
        [PI, 2.0 * PI],
        None,
    );

    let he_up = solid.add_half_edge(gen.next(), v_south, e_seam, true);
    let he_down = solid.add_half_edge(gen.next(), v_north, e_seam, false);
    let outer_loop = solid.add_loop(gen.next(), vec![he_up, he_down]);

    // ゼロ circ_normal
    let circ_normal = Vec3::zeros();
    let circ_center = center;
    let circ_radius = 4.0;

    // F02: inner_loop を 1 edge full-sweep に構築 (周期エッジ)
    let loop_vertex = solid.add_vertex(gen.next(), circ_center + circ_radius * Vec3::x(), None);

    let loop_edge = solid.add_edge(
        gen.next(),
        [loop_vertex, loop_vertex], // 周期エッジ: start = end
        Curve::Circle {
            center: circ_center,
            normal: circ_normal,
            radius: circ_radius,
        },
        [0.0, 2.0 * PI],
        None,
    );
    let loop_he = solid.add_half_edge(gen.next(), loop_vertex, loop_edge, true);
    let inner_loop = solid.add_loop(gen.next(), vec![loop_he]);

    let face = solid.add_face(
        gen.next(),
        Surface::Sphere { center, radius },
        outer_loop,
        vec![inner_loop],
        true,
        None,
    );

    solid.add_shell(gen.next(), vec![face], true);

    let mut mesh = TriangleMesh::new();
    let opts = mycad_kernel::tessellation::TessellationOptions {
        angular_segments: 16,
        ..Default::default()
    };

    let result = mycad_kernel::tessellation::tessellate_sphere_face_trimmed(
        &solid,
        &solid.faces[face],
        face,
        &opts,
        &mut mesh,
        "",
    );

    assert!(
        matches!(result, Err(TessellationError::InvalidTrimCircle { .. })),
        "zero circ_normal should return InvalidTrimCircle"
    );
}

/// Solid から twin HalfEdge を見つける。
fn find_twin_halfedge(solid: &mycad_kernel::brep::topology::Solid, he_idx: usize) -> Option<usize> {
    let target_edge = solid.half_edges[he_idx].edge;
    (0..solid.half_edges.len()).find(|&i| i != he_idx && solid.half_edges[i].edge == target_edge)
}

/// HalfEdge が属する face の index を見つける。
fn find_face_for_halfedge(
    solid: &mycad_kernel::brep::topology::Solid,
    he_idx: usize,
) -> Option<usize> {
    (0..solid.faces.len()).find(|&f| {
        let face = &solid.faces[f];
        solid.loops[face.outer_loop].half_edges.contains(&he_idx)
            || face
                .inner_loops
                .iter()
                .any(|&l| solid.loops[l].half_edges.contains(&he_idx))
    })
}

/// mesh から特定の face_id に属する triangle のインデックス集合を抽出する。
fn extract_face_triangles(mesh: &TriangleMesh, face_id: &str) -> Vec<usize> {
    mesh.face_ids
        .iter()
        .enumerate()
        .filter(|(_, id)| *id == face_id)
        .map(|(tri_idx, _)| tri_idx)
        .collect()
}

/// triangles から boundary edge (1 triangle にしか含まれない edge) を抽出する。
/// 返り値は量子化済み edge (q0, q1) の集合 (q0 < q1 で正規化済み)。
///
/// 同一 triangle 内で量子化後に重複する edge は 1 回だけ数える
/// (sliver triangle で 2 頂点が同一点へ潰れる場合に boundary edge が消えるのを防ぐ)。
fn extract_boundary_edges(
    mesh: &TriangleMesh,
    tri_indices: &[usize],
    tol: f64,
) -> std::collections::HashSet<([i64; 3], [i64; 3])> {
    let quantize = |p: &[f64; 3]| -> [i64; 3] {
        [
            (p[0] / tol).round() as i64,
            (p[1] / tol).round() as i64,
            (p[2] / tol).round() as i64,
        ]
    };
    let mut edge_count: std::collections::HashMap<([i64; 3], [i64; 3]), usize> =
        std::collections::HashMap::new();
    for &tri_idx in tri_indices {
        let i0 = mesh.indices[tri_idx * 3] as usize;
        let i1 = mesh.indices[tri_idx * 3 + 1] as usize;
        let i2 = mesh.indices[tri_idx * 3 + 2] as usize;
        let p0 = quantize(&mesh.positions[i0]);
        let p1 = quantize(&mesh.positions[i1]);
        let p2 = quantize(&mesh.positions[i2]);
        let mut seen_in_tri: Vec<([i64; 3], [i64; 3])> = Vec::with_capacity(3);
        for (qa, qb) in &[(p0, p1), (p1, p2), (p2, p0)] {
            if qa == qb {
                continue;
            }
            let key = if qa <= qb { (*qa, *qb) } else { (*qb, *qa) };
            if seen_in_tri.contains(&key) {
                continue;
            }
            seen_in_tri.push(key);
            *edge_count.entry(key).or_insert(0) += 1;
        }
    }
    edge_count
        .into_iter()
        .filter(|(_, count)| *count == 1)
        .map(|(key, _)| key)
        .collect()
}

/// boundary edge 集合から順序付き polyline ring を構築する (panic 安全)。
/// closed ring (各 vertex の degree = 2) を仮定。条件外なら None を返す。
fn try_build_ordered_ring_mesh(
    boundary_edges: &std::collections::HashSet<([i64; 3], [i64; 3])>,
) -> Option<Vec<[i64; 3]>> {
    if boundary_edges.is_empty() {
        return None;
    }
    let mut adj: std::collections::HashMap<[i64; 3], Vec<[i64; 3]>> =
        std::collections::HashMap::new();
    for (a, b) in boundary_edges.iter() {
        adj.entry(*a).or_default().push(*b);
        adj.entry(*b).or_default().push(*a);
    }
    if adj.values().any(|v| v.len() != 2) {
        return None;
    }
    let start = *adj.keys().next()?;
    let mut ring = vec![start];
    let mut prev = start;
    let mut current = start;
    let max_steps = boundary_edges.len() + 1;
    for _ in 0..max_steps {
        let neighbors = adj.get(&current)?;
        let next = if ring.len() == 1 {
            neighbors[0]
        } else if neighbors[0] == prev {
            neighbors[1]
        } else {
            neighbors[0]
        };
        if next == start {
            return Some(ring);
        }
        ring.push(next);
        prev = current;
        current = next;
    }
    None
}

/// quantize された 3D 点同士の差ノルム (mesh 頂点列の microscopic 比較用)。
fn point_diff_3d(a: &[f64; 3], b: &[f64; 3]) -> f64 {
    ((a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2) + (a[2] - b[2]).powi(2)).sqrt()
}

/// T04_strict: boolean_cut_sphere_dimple 結果で、trimmed sphere face と
/// 隣接 face (Plane) の共有境界頂点が twin 経由で厳密に一致することを確認。
///
/// #147 strict 版実装 (F01 round 2):
/// 1. twin HalfEdge 経由で sphere face と adjacent face を対応付け
/// 2. mesh.face_ids と mesh.indices から各 face の triangle 集合を抽出
/// 3. 各 face の triangles 内で boundary edge (1 triangle にしか属さない edge) を抽出
/// 4. 位相一致: sphere 側と adj 側の boundary edge 集合が量子化下で完全一致
/// 5. microscopic check: 各 face の mesh 頂点のみで境界頂点を比較
/// 6. 向きずれ検出: boundary edges から構築した polyline ring を比較
#[test]
fn t04_shared_boundary_with_cyl_lateral() {
    let mut gen = IdGenerator::new(0);
    let box_solid = make_cuboid(10.0, 10.0, 10.0, &mut gen).expect("cuboid");
    let sphere = make_sphere(3.0, Point::new(0.0, 0.0, 6.0), &mut gen).expect("sphere");
    let result = boolean(&box_solid, &sphere, BooleanOp::Cut, &mut gen).expect("cut");
    let mesh = tessellate_solid(&result).expect("tessellate");

    // naked_edge が 0 であれば watertight (回帰テスト)
    let naked = count_naked_edges(&mesh, 1e-10);
    assert_eq!(
        naked, 0,
        "shared boundary should be watertight (zero naked edges)"
    );

    // strict 版: twin ベース境界頂点比較
    // 1. Surface::Sphere 型の trimmed face (inner_loops.len() == 1) を特定
    use mycad_kernel::geometry::surface::Surface;
    let sphere_face_idx = result
        .faces
        .iter()
        .position(|f| matches!(f.surface, Surface::Sphere { .. }) && f.inner_loops.len() == 1);
    assert!(sphere_face_idx.is_some(), "No trimmed sphere face found");
    let sphere_face_idx = sphere_face_idx.unwrap();
    let sphere_face = &result.faces[sphere_face_idx];

    // sphere face の face_id を取得
    let sphere_face_id = sphere_face
        .name
        .as_ref()
        .map(|n| n.canonical_name())
        .unwrap_or_default();

    // 2. inner_loops[0].half_edges[0] の twin から対面 face を特定
    let sphere_inner_loop = &result.loops[sphere_face.inner_loops[0]];
    let sphere_he_idx = sphere_inner_loop.half_edges.first().unwrap();
    let twin_he_idx = find_twin_halfedge(&result, *sphere_he_idx);
    assert!(
        twin_he_idx.is_some(),
        "No twin half-edge found for sphere inner loop edge"
    );
    let twin_he_idx = twin_he_idx.unwrap();

    let adj_face_idx = find_face_for_halfedge(&result, twin_he_idx);
    assert!(
        adj_face_idx.is_some(),
        "No adjacent face found via twin half-edge"
    );
    let adj_face_idx = adj_face_idx.unwrap();
    let adj_face = &result.faces[adj_face_idx];

    // 確認: 対面は Plane 型であるはず (box の上面)
    assert!(
        matches!(adj_face.surface, Surface::Plane { .. }),
        "Adjacent face should be Plane, got {:?}",
        adj_face.surface
    );

    // adjacent face の face_id を取得
    let adj_face_id = adj_face
        .name
        .as_ref()
        .map(|n| n.canonical_name())
        .unwrap_or_default();

    // 3. mesh から両 face の triangle 集合を抽出 (face_id 経由のみ、B-rep vertex/edge 不使用)
    let sphere_tris = extract_face_triangles(&mesh, &sphere_face_id);
    let adj_tris = extract_face_triangles(&mesh, &adj_face_id);

    assert!(!sphere_tris.is_empty(), "Sphere face should have triangles");
    assert!(!adj_tris.is_empty(), "Adjacent face should have triangles");

    // 4. boundary edge (1 triangle にしか含まれない edge) を mesh から直接抽出
    let tol = LENGTH_TOLERANCE;
    let sphere_boundary = extract_boundary_edges(&mesh, &sphere_tris, tol);
    let adj_boundary = extract_boundary_edges(&mesh, &adj_tris, tol);

    assert!(
        !sphere_boundary.is_empty(),
        "Sphere face should have boundary edges"
    );
    assert!(
        !adj_boundary.is_empty(),
        "Adjacent face should have boundary edges"
    );

    // 5. 両面 mesh boundary の順序付き polyline ring を独立に構築し直接比較
    //    (Codex F01: 各 face 自身の tri 集合から polyline を再構成して直接比較。
    //     B-rep の result.edges / result.vertices / mesh.positions 全体走査は使わない)

    // 5-1. sphere face の boundary edges (Step 4 で抽出済み) は inner_loop に対応
    //      (sphere face の outer_loop は seam edge で Edge 共有 `count==2` のため
    //       boundary 集合 (`count==1`) に出現しない)
    // 5-2. adj face の boundary edges のうち sphere との共有部分のみ抽出
    //      (adj face は box 上面で、box 側面との境界 (outer_loop) も持つため subset を取る)
    let adj_inner_boundary_mesh: std::collections::HashSet<([i64; 3], [i64; 3])> = adj_boundary
        .intersection(&sphere_boundary)
        .cloned()
        .collect();

    // 5-3. 両 face の共有境界 edge 集合の完全一致 (位相一致)
    assert_eq!(
        sphere_boundary,
        adj_inner_boundary_mesh,
        "sphere face と adj face の共有 boundary edge 集合が一致しません: \
         sphere only={:?}, adj only={:?}",
        sphere_boundary
            .difference(&adj_inner_boundary_mesh)
            .collect::<Vec<_>>(),
        adj_inner_boundary_mesh
            .difference(&sphere_boundary)
            .collect::<Vec<_>>(),
    );

    // 5-4. 順序付き ring を両 face 独立に構築
    let sphere_ring_keys = try_build_ordered_ring_mesh(&sphere_boundary)
        .expect("sphere face boundary should form a closed ring");
    let adj_ring_keys = try_build_ordered_ring_mesh(&adj_inner_boundary_mesh)
        .expect("adj face boundary should form a closed ring");

    assert_eq!(
        sphere_ring_keys.len(),
        adj_ring_keys.len(),
        "ring vertex count mismatch: sphere={}, adj={}",
        sphere_ring_keys.len(),
        adj_ring_keys.len(),
    );

    // 5-5. 各 face triangle 集合から「量子化キー → その face 自身の mesh position」逆引きを準備
    let quantize_local = |p: &[f64; 3]| -> [i64; 3] {
        [
            (p[0] / tol).round() as i64,
            (p[1] / tol).round() as i64,
            (p[2] / tol).round() as i64,
        ]
    };
    let pos_for_key = |tri_indices: &[usize], key: &[i64; 3]| -> Option<[f64; 3]> {
        for &tri in tri_indices {
            for k in 0..3 {
                let idx = mesh.indices[tri * 3 + k] as usize;
                let p = mesh.positions[idx];
                if quantize_local(&p) == *key {
                    return Some(p);
                }
            }
        }
        None
    };

    let sphere_positions: Vec<[f64; 3]> = sphere_ring_keys
        .iter()
        .map(|k| pos_for_key(&sphere_tris, k).expect("sphere ring key not found in sphere tris"))
        .collect();
    let adj_positions: Vec<[f64; 3]> = adj_ring_keys
        .iter()
        .map(|k| pos_for_key(&adj_tris, k).expect("adj ring key not found in adj tris"))
        .collect();

    // 5-6. rotation / reverse を考慮した各 index 直接距離比較 (各 face 自身の mesh 頂点列のみ)
    let n = sphere_positions.len();
    let mut matched_forward = false;
    let mut matched_reverse = false;
    for shift in 0..n {
        let mut all_match = true;
        for i in 0..n {
            let sp = &sphere_positions[i];
            let ap = &adj_positions[(i + shift) % n];
            if point_diff_3d(sp, ap) > LENGTH_TOLERANCE {
                all_match = false;
                break;
            }
        }
        if all_match {
            matched_forward = true;
            break;
        }
    }
    if !matched_forward {
        let adj_rev: Vec<_> = adj_positions.iter().rev().cloned().collect();
        for shift in 0..n {
            let mut all_match = true;
            for i in 0..n {
                let sp = &sphere_positions[i];
                let ap = &adj_rev[(i + shift) % n];
                if point_diff_3d(sp, ap) > LENGTH_TOLERANCE {
                    all_match = false;
                    break;
                }
            }
            if all_match {
                matched_reverse = true;
                break;
            }
        }
    }
    assert!(
        matched_forward || matched_reverse,
        "sphere face boundary polyline と adj face boundary polyline が一致しません \
         (rotation/reverse 込みでも tol={} を超える距離差が残ります)",
        LENGTH_TOLERANCE,
    );
}

/// F01 round 2 回帰テスト: 1HE ループだが t_range が半円 [0, π] のみ → TrimmedFaceUnsupported。
/// 周期エッジ検証: start_vertex == end_vertex かつ t_range が 2π 以外は reject。
#[test]
fn t_degen_partial_arc_1he_rejected() {
    use mycad_kernel::brep::topology::Solid;

    let radius = 5.0;
    let center = Point::origin();
    let mut gen = IdGenerator::new(0);
    let mut solid = Solid::new(gen.next());

    // 球の外殻
    let v_south = solid.add_vertex(gen.next(), center + Vec3::new(0.0, 0.0, -radius), None);
    let v_north = solid.add_vertex(gen.next(), center + Vec3::new(0.0, 0.0, radius), None);

    let e_seam = solid.add_edge(
        gen.next(),
        [v_south, v_north],
        Curve::Circle {
            center,
            normal: -Vec3::y(),
            radius,
        },
        [PI, 2.0 * PI],
        None,
    );

    let he_up = solid.add_half_edge(gen.next(), v_south, e_seam, true);
    let he_down = solid.add_half_edge(gen.next(), v_north, e_seam, false);
    let outer_loop = solid.add_loop(gen.next(), vec![he_up, he_down]);

    // 1HE ループだが t_range が半円 [0, π] のみ（full circle ではない）
    let circ_normal = Vec3::new(0.0, 0.0, 1.0);
    let circ_center = center;
    let circ_radius = 4.0;

    // 周期エッジ用頂点: start == end
    let loop_vertex = solid.add_vertex(gen.next(), circ_center + circ_radius * Vec3::x(), None);

    // t_range = [0, π] （半円、2π ではない）
    let loop_edge = solid.add_edge(
        gen.next(),
        [loop_vertex, loop_vertex],
        Curve::Circle {
            center: circ_center,
            normal: circ_normal,
            radius: circ_radius,
        },
        [0.0, std::f64::consts::PI], // 半円のみ
        None,
    );
    let loop_he = solid.add_half_edge(gen.next(), loop_vertex, loop_edge, true);
    let inner_loop = solid.add_loop(gen.next(), vec![loop_he]);

    let face = solid.add_face(
        gen.next(),
        Surface::Sphere { center, radius },
        outer_loop,
        vec![inner_loop],
        true,
        None,
    );

    solid.add_shell(gen.next(), vec![face], true);

    let mut mesh = TriangleMesh::new();
    let opts = mycad_kernel::tessellation::TessellationOptions {
        angular_segments: 16,
        ..Default::default()
    };

    let result = mycad_kernel::tessellation::tessellate_sphere_face_trimmed(
        &solid,
        &solid.faces[face],
        face,
        &opts,
        &mut mesh,
        "",
    );

    assert!(
        matches!(result, Err(TessellationError::TrimmedFaceUnsupported)),
        "partial arc (1HE with t_range != 2π) should return TrimmedFaceUnsupported: {:?}",
        result
    );
}

/// F01 回帰テスト: circ_normal が非ゼロだが LENGTH_TOLERANCE 未満の極小ベクトル
/// (例: Vec3::new(1e-15, 0.0, 0.0)) でも InvalidTrimCircle を返すこと。
#[test]
fn t_degen_subtolerance_circ_normal_returns_error() {
    use mycad_kernel::brep::topology::Solid;

    let radius = 5.0;
    let center = Point::origin();
    let mut gen = IdGenerator::new(0);
    let mut solid = Solid::new(gen.next());

    // 球の外殻
    let v_south = solid.add_vertex(gen.next(), center + Vec3::new(0.0, 0.0, -radius), None);
    let v_north = solid.add_vertex(gen.next(), center + Vec3::new(0.0, 0.0, radius), None);

    let e_seam = solid.add_edge(
        gen.next(),
        [v_south, v_north],
        Curve::Circle {
            center,
            normal: -Vec3::y(),
            radius,
        },
        [PI, 2.0 * PI],
        None,
    );

    let he_up = solid.add_half_edge(gen.next(), v_south, e_seam, true);
    let he_down = solid.add_half_edge(gen.next(), v_north, e_seam, false);
    let outer_loop = solid.add_loop(gen.next(), vec![he_up, he_down]);

    // 極小 circ_normal: LENGTH_TOLERANCE 未満の非ゼロベクトル
    let circ_normal = Vec3::new(1e-15, 0.0, 0.0); // norm = 1e-15 << LENGTH_TOLERANCE (1e-9)
    let circ_center = center;
    let circ_radius = 4.0;

    // F02: inner_loop を 1 edge full-sweep に構築
    let loop_vertex = solid.add_vertex(gen.next(), circ_center + circ_radius * Vec3::x(), None);

    let loop_edge = solid.add_edge(
        gen.next(),
        [loop_vertex, loop_vertex],
        Curve::Circle {
            center: circ_center,
            normal: circ_normal,
            radius: circ_radius,
        },
        [0.0, 2.0 * PI],
        None,
    );
    let loop_he = solid.add_half_edge(gen.next(), loop_vertex, loop_edge, true);
    let inner_loop = solid.add_loop(gen.next(), vec![loop_he]);

    let face = solid.add_face(
        gen.next(),
        Surface::Sphere { center, radius },
        outer_loop,
        vec![inner_loop],
        true,
        None,
    );

    solid.add_shell(gen.next(), vec![face], true);

    let mut mesh = TriangleMesh::new();
    let opts = mycad_kernel::tessellation::TessellationOptions {
        angular_segments: 16,
        ..Default::default()
    };

    let result = mycad_kernel::tessellation::tessellate_sphere_face_trimmed(
        &solid,
        &solid.faces[face],
        face,
        &opts,
        &mut mesh,
        "",
    );

    assert!(
        matches!(result, Err(TessellationError::InvalidTrimCircle { .. })),
        "subtolerance circ_normal should return InvalidTrimCircle"
    );
}

// #147: 退化入力テスト（STEP 6 GLM 実装）

/// T_degen_offset_axis_circ_center_rejected: circ_center を axis 直交方向に
/// ずらした入力で InvalidTrimCircle を返すことを検証。
///
/// circ_center が axis（circ_normal）から直交方向にずれていると、
/// 円は球面上の正しい緯線として再合成されず、隣接面との境界が破綻する。
#[test]
fn t_degen_offset_axis_circ_center_rejected() {
    use mycad_kernel::brep::topology::Solid;

    let radius = 5.0;
    let center = Point::origin();
    let mut gen = IdGenerator::new(0);
    let mut solid = Solid::new(gen.next());

    // 球の外殻
    let v_south = solid.add_vertex(gen.next(), center + Vec3::new(0.0, 0.0, -radius), None);
    let v_north = solid.add_vertex(gen.next(), center + Vec3::new(0.0, 0.0, radius), None);

    let e_seam = solid.add_edge(
        gen.next(),
        [v_south, v_north],
        Curve::Circle {
            center,
            normal: -Vec3::y(),
            radius,
        },
        [PI, 2.0 * PI],
        None,
    );

    let he_up = solid.add_half_edge(gen.next(), v_south, e_seam, true);
    let he_down = solid.add_half_edge(gen.next(), v_north, e_seam, false);
    let outer_loop = solid.add_loop(gen.next(), vec![he_up, he_down]);

    // circ_normal = +Z、signed_offset = 0 (circ_center は球中心にあるべき)
    let circ_normal = Vec3::new(0.0, 0.0, 1.0);
    let circ_center = center + Vec3::new(0.01, 0.0, 0.0); // axis 直交方向に 0.01mm ずらす
    let circ_radius = 4.0;

    // inner_loop を 1 edge full-sweep に構築 (周期エッジ)
    let loop_vertex = solid.add_vertex(gen.next(), circ_center + circ_radius * Vec3::y(), None);

    let loop_edge = solid.add_edge(
        gen.next(),
        [loop_vertex, loop_vertex],
        Curve::Circle {
            center: circ_center,
            normal: circ_normal,
            radius: circ_radius,
        },
        [0.0, 2.0 * PI],
        None,
    );
    let loop_he = solid.add_half_edge(gen.next(), loop_vertex, loop_edge, true);
    let inner_loop = solid.add_loop(gen.next(), vec![loop_he]);

    let face = solid.add_face(
        gen.next(),
        Surface::Sphere { center, radius },
        outer_loop,
        vec![inner_loop],
        true,
        None,
    );

    solid.add_shell(gen.next(), vec![face], true);

    let mut mesh = TriangleMesh::new();
    let opts = mycad_kernel::tessellation::TessellationOptions {
        angular_segments: 16,
        ..Default::default()
    };

    let result = mycad_kernel::tessellation::tessellate_sphere_face_trimmed(
        &solid,
        &solid.faces[face],
        face,
        &opts,
        &mut mesh,
        "",
    );

    assert!(
        matches!(result, Err(TessellationError::InvalidTrimCircle { .. })),
        "circ_center offset from axis should return InvalidTrimCircle: {:?}",
        result
    );
}

/// T_degen_mismatched_circ_radius_rejected: circ_radius を期待値
/// sqrt(R^2 - signed_offset^2) からずらした入力で InvalidTrimCircle を返すことを検証。
///
/// circ_radius が球面上の正しい緯線半径と一致しない場合、
/// 隣接面側の境界半径と接続できない。
#[test]
fn t_degen_mismatched_circ_radius_rejected() {
    use mycad_kernel::brep::topology::Solid;

    let radius = 5.0_f64;
    let center = Point::origin();
    let signed_offset = 3.0_f64; // 期待 circ_radius = sqrt(25 - 9) = 4.0
    let expected_circ_radius = (radius * radius - signed_offset * signed_offset).sqrt();

    let mut gen = IdGenerator::new(0);
    let mut solid = Solid::new(gen.next());

    // 球の外殻
    let v_south = solid.add_vertex(gen.next(), center + Vec3::new(0.0, 0.0, -radius), None);
    let v_north = solid.add_vertex(gen.next(), center + Vec3::new(0.0, 0.0, radius), None);

    let e_seam = solid.add_edge(
        gen.next(),
        [v_south, v_north],
        Curve::Circle {
            center,
            normal: -Vec3::y(),
            radius,
        },
        [PI, 2.0 * PI],
        None,
    );

    let he_up = solid.add_half_edge(gen.next(), v_south, e_seam, true);
    let he_down = solid.add_half_edge(gen.next(), v_north, e_seam, false);
    let outer_loop = solid.add_loop(gen.next(), vec![he_up, he_down]);

    // circ_normal = +Z、signed_offset = 3.0、circ_center = (0, 0, 3.0)
    let circ_normal = Vec3::new(0.0, 0.0, 1.0);
    let circ_center = center + signed_offset * circ_normal;
    let circ_radius = expected_circ_radius + 0.01; // 期待値から 0.01mm ずらす

    // inner_loop を 1 edge full-sweep に構築 (周期エッジ)
    let loop_vertex = solid.add_vertex(gen.next(), circ_center + circ_radius * Vec3::y(), None);

    let loop_edge = solid.add_edge(
        gen.next(),
        [loop_vertex, loop_vertex],
        Curve::Circle {
            center: circ_center,
            normal: circ_normal,
            radius: circ_radius,
        },
        [0.0, 2.0 * PI],
        None,
    );
    let loop_he = solid.add_half_edge(gen.next(), loop_vertex, loop_edge, true);
    let inner_loop = solid.add_loop(gen.next(), vec![loop_he]);

    let face = solid.add_face(
        gen.next(),
        Surface::Sphere { center, radius },
        outer_loop,
        vec![inner_loop],
        true,
        None,
    );

    solid.add_shell(gen.next(), vec![face], true);

    let mut mesh = TriangleMesh::new();
    let opts = mycad_kernel::tessellation::TessellationOptions {
        angular_segments: 16,
        ..Default::default()
    };

    let result = mycad_kernel::tessellation::tessellate_sphere_face_trimmed(
        &solid,
        &solid.faces[face],
        face,
        &opts,
        &mut mesh,
        "",
    );

    assert!(
        matches!(result, Err(TessellationError::InvalidTrimCircle { .. })),
        "mismatched circ_radius should return InvalidTrimCircle: {:?}",
        result
    );
}

/// F02: circ_radius = NaN で InvalidTrimCircle を返す。
#[test]
fn t_degen_nan_circ_radius_rejected() {
    use mycad_kernel::brep::topology::Solid;

    let radius = 5.0_f64;
    let center = Point::origin();
    let signed_offset = 3.0_f64;
    let expected_circ_radius = (radius * radius - signed_offset * signed_offset).sqrt();

    let mut gen = IdGenerator::new(0);
    let mut solid = Solid::new(gen.next());

    // 球の外殻
    let v_south = solid.add_vertex(gen.next(), center + Vec3::new(0.0, 0.0, -radius), None);
    let v_north = solid.add_vertex(gen.next(), center + Vec3::new(0.0, 0.0, radius), None);

    let e_seam = solid.add_edge(
        gen.next(),
        [v_south, v_north],
        Curve::Circle {
            center,
            normal: -Vec3::y(),
            radius,
        },
        [PI, 2.0 * PI],
        None,
    );

    let he_up = solid.add_half_edge(gen.next(), v_south, e_seam, true);
    let he_down = solid.add_half_edge(gen.next(), v_north, e_seam, false);
    let outer_loop = solid.add_loop(gen.next(), vec![he_up, he_down]);

    // circ_normal = +Z、signed_offset = 3.0、circ_center = (0, 0, 3.0)
    let circ_normal = Vec3::new(0.0, 0.0, 1.0);
    let circ_center = center + signed_offset * circ_normal;
    let circ_radius = f64::NAN; // NaN を設定

    // inner_loop を 1 edge full-sweep に構築 (周期エッジ)
    let loop_vertex = solid.add_vertex(
        gen.next(),
        circ_center + expected_circ_radius * Vec3::y(),
        None,
    );

    let loop_edge = solid.add_edge(
        gen.next(),
        [loop_vertex, loop_vertex],
        Curve::Circle {
            center: circ_center,
            normal: circ_normal,
            radius: circ_radius,
        },
        [0.0, 2.0 * PI],
        None,
    );
    let loop_he = solid.add_half_edge(gen.next(), loop_vertex, loop_edge, true);
    let inner_loop = solid.add_loop(gen.next(), vec![loop_he]);

    let face = solid.add_face(
        gen.next(),
        Surface::Sphere { center, radius },
        outer_loop,
        vec![inner_loop],
        true,
        None,
    );

    solid.add_shell(gen.next(), vec![face], true);

    let mut mesh = TriangleMesh::new();
    let opts = mycad_kernel::tessellation::TessellationOptions {
        angular_segments: 16,
        ..Default::default()
    };

    let result = mycad_kernel::tessellation::tessellate_sphere_face_trimmed(
        &solid,
        &solid.faces[face],
        face,
        &opts,
        &mut mesh,
        "",
    );

    assert!(
        matches!(result, Err(TessellationError::InvalidTrimCircle { .. })),
        "NaN circ_radius should return InvalidTrimCircle: {:?}",
        result
    );
}

/// F02: circ_center に NaN 成分が含まれる場合 InvalidTrimCircle を返す。
#[test]
fn t_degen_nan_circ_center_rejected() {
    use mycad_kernel::brep::topology::Solid;

    let radius = 5.0_f64;
    let center = Point::origin();
    let signed_offset = 3.0_f64;
    let expected_circ_radius = (radius * radius - signed_offset * signed_offset).sqrt();

    let mut gen = IdGenerator::new(0);
    let mut solid = Solid::new(gen.next());

    // 球の外殻
    let v_south = solid.add_vertex(gen.next(), center + Vec3::new(0.0, 0.0, -radius), None);
    let v_north = solid.add_vertex(gen.next(), center + Vec3::new(0.0, 0.0, radius), None);

    let e_seam = solid.add_edge(
        gen.next(),
        [v_south, v_north],
        Curve::Circle {
            center,
            normal: -Vec3::y(),
            radius,
        },
        [PI, 2.0 * PI],
        None,
    );

    let he_up = solid.add_half_edge(gen.next(), v_south, e_seam, true);
    let he_down = solid.add_half_edge(gen.next(), v_north, e_seam, false);
    let outer_loop = solid.add_loop(gen.next(), vec![he_up, he_down]);

    // circ_normal = +Z、signed_offset = 3.0、circ_center に NaN 成分を含める
    let circ_normal = Vec3::new(0.0, 0.0, 1.0);
    let circ_center = Point::new(f64::NAN, 0.0, 3.0); // x 成分が NaN
    let circ_radius = expected_circ_radius;

    // inner_loop を 1 edge full-sweep に構築 (周期エッジ)
    // NaN center の場合でも curve 定義自体は可能なので、ここでは構築のみ行う
    let loop_vertex =
        solid.add_vertex(gen.next(), Point::new(0.0, expected_circ_radius, 3.0), None);

    let loop_edge = solid.add_edge(
        gen.next(),
        [loop_vertex, loop_vertex],
        Curve::Circle {
            center: circ_center,
            normal: circ_normal,
            radius: circ_radius,
        },
        [0.0, 2.0 * PI],
        None,
    );
    let loop_he = solid.add_half_edge(gen.next(), loop_vertex, loop_edge, true);
    let inner_loop = solid.add_loop(gen.next(), vec![loop_he]);

    let face = solid.add_face(
        gen.next(),
        Surface::Sphere { center, radius },
        outer_loop,
        vec![inner_loop],
        true,
        None,
    );

    solid.add_shell(gen.next(), vec![face], true);

    let mut mesh = TriangleMesh::new();
    let opts = mycad_kernel::tessellation::TessellationOptions {
        angular_segments: 16,
        ..Default::default()
    };

    let result = mycad_kernel::tessellation::tessellate_sphere_face_trimmed(
        &solid,
        &solid.faces[face],
        face,
        &opts,
        &mut mesh,
        "",
    );

    assert!(
        matches!(result, Err(TessellationError::InvalidTrimCircle { .. })),
        "NaN circ_center should return InvalidTrimCircle: {:?}",
        result
    );
}
