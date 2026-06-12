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

/// T01: 既存の boolean_cut_sphere_dimple (cyl×sphere, 軸 +Z) を 2 回 tessellate し、
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

/// T04: boolean_cut_sphere_dimple 結果で、trimmed sphere face と
/// 隣接 cyl lateral face の共有境界頂点が一致することを確認。
///
/// 現実装では TriangleMesh に face_id 情報があるため、直接頂点を比較するのは困難。
/// 代わりに naked_edge カウントが 0 であることで watertight 性を保証する。
#[test]
fn t04_shared_boundary_with_cyl_lateral() {
    let mut gen = IdGenerator::new(0);
    let box_solid = make_cuboid(10.0, 10.0, 10.0, &mut gen).expect("cuboid");
    let sphere = make_sphere(3.0, Point::new(0.0, 0.0, 6.0), &mut gen).expect("sphere");
    let result = boolean(&box_solid, &sphere, BooleanOp::Cut, &mut gen).expect("cut");
    let mesh = tessellate_solid(&result).expect("tessellate");

    // naked_edge が 0 であれば、隣接面間の境界が一致している
    let naked = count_naked_edges(&mesh, 1e-10);
    assert_eq!(
        naked, 0,
        "shared boundary should be watertight (zero naked edges)"
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
