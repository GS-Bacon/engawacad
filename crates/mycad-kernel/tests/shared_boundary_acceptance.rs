// Acceptance tests for Issue #144 (ADR-009 §Implementation Outline Phase 3, codex #129-F02):
// 隣接面の共有境界サンプル列を直接比較する acceptance テスト。
//
// 現状の Boolean tessellation は「両側面が独立に同じサンプル列 (k * 2π / N) を再導出する」
// 規約に依存する。この規約違反 (両側のサンプル列がずれる) が起きた瞬間に
// `エッジ index / 頂点 index / t_a / t_b / p_a / p_b / 距離 mm` を出力して
// 即座に検出するセーフティネット。
//
// ADR-009 本体実装 (Phase 1: 交線エッジ周期化, Phase 2: テッセレーション側のエッジ駆動化)
// が入った後は本テストは構造的に green になる。

use mycad_format::{EntityKind, EntityRef};
use mycad_kernel::booleans::{boolean, BooleanOp};
use mycad_kernel::brep::topology::{IdGenerator, Solid};
use mycad_kernel::geometry::curve::Curve;
use mycad_kernel::geometry::{Point, LENGTH_TOLERANCE};
use mycad_kernel::primitives::{make_cuboid, make_cylinder, make_sphere};
use mycad_kernel::tessellation::{tessellate_solid, TriangleMesh};
use std::collections::{HashMap, HashSet};

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// テスト側で `Face.name` をユニークに上書きする。
fn assign_unique_face_names(solid: &mut Solid) {
    for (i, face) in solid.faces.iter_mut().enumerate() {
        face.name = Some(
            EntityRef::try_named("shared_boundary_test", EntityKind::Face, format!("f{i}"))
                .expect("valid identifier"),
        );
    }
}

/// `mesh.face_ids[tri_idx]` を照合し per-face 頂点 idx 集合を構築。
fn collect_face_vertex_set(mesh: &TriangleMesh, face_id: &str) -> HashSet<usize> {
    let mut verts = HashSet::new();
    for tri in 0..mesh.triangle_count() {
        if mesh.face_ids[tri] == face_id {
            verts.insert(mesh.indices[tri * 3] as usize);
            verts.insert(mesh.indices[tri * 3 + 1] as usize);
            verts.insert(mesh.indices[tri * 3 + 2] as usize);
        }
    }
    verts
}

/// outer_loop + inner_loops を走査し half_edge_idx → face_idx の写像を構築。
fn build_he_to_face(solid: &Solid) -> HashMap<usize, usize> {
    let mut he_to_face = HashMap::new();
    for (face_idx, face) in solid.faces.iter().enumerate() {
        let outer = &solid.loops[face.outer_loop];
        for &he_idx in &outer.half_edges {
            he_to_face.insert(he_idx, face_idx);
        }
        for &inner_idx in &face.inner_loops {
            let inner = &solid.loops[inner_idx];
            for &he_idx in &inner.half_edges {
                he_to_face.insert(he_idx, face_idx);
            }
        }
    }
    he_to_face
}

/// 点を Circle に投影し `(t in [t_start, t_start+2π), distance)` を返す。
/// t は atan2(cy, cx) を使って [t_start, t_start + 2π) に正規化。
fn project_to_circle(
    p: [f64; 3],
    center: Point,
    normal: mycad_kernel::geometry::Vec3,
    radius: f64,
    t_start: f64,
) -> (f64, f64) {
    use mycad_kernel::geometry::math::orthonormal_basis;

    let p_pt = Point::new(p[0], p[1], p[2]);
    // 法線を単位化してから基底を作る (Curve::Circle.normal は単位ベクトル保証されないため)
    let n_hat = normal.normalize();
    let (u, v) = orthonormal_basis(&n_hat);

    // 点から円の平面への投影
    let cp = p_pt - center;
    let cu = cp.dot(&u);
    let cv = cp.dot(&v);
    // 法線方向 (axial) 成分 — 円の平面から離れた点を正しく除外するため必須。
    // 旧実装は radial drift しか見ておらず、円柱側面の UV グリッド頂点 (axial 方向にずれる)
    // が全て「円上」と誤判定され 130 件等の過剰サンプル数が出る原因になっていた。
    let axial = cp.dot(&n_hat);

    // Euclidean 距離 = sqrt(radial_residual² + axial²)
    let radial_residual = cu.hypot(cv) - radius;
    let dist = (radial_residual * radial_residual + axial * axial).sqrt();

    // atan2 で t を計算 (-π, π]
    let t_raw = cv.atan2(cu);

    // [t_start, t_start + 2π) に正規化
    let period = std::f64::consts::PI * 2.0;
    let mut t = t_raw - t_start;
    while t < 0.0 {
        t += period;
    }
    while t >= period {
        t -= period;
    }
    let t = t_start + t;

    (t, dist)
}

/// candidates から円上 (≤ LENGTH_TOLERANCE) の頂点を抽出し t でソート。
///
/// 周期境界の seam 重複を除外するため、ソート後に隣接する LENGTH_TOLERANCE 以内の
/// 位置を 1 件にまとめる。UV グリッド面 (e.g. cylinder lateral) は seam vertex を
/// t=t_start と t=t_start+2π の 2 回ストアするが、両方とも同じ `Curve::evaluate(t_start)`
/// の結果で bitwise 一致するため、これは表現差であって規約違反ではない。論理サンプル列
/// (両側で共有されるべき円周上のサンプル点) で比較する。
fn vertices_on_circle_sorted(
    mesh: &TriangleMesh,
    candidates: &HashSet<usize>,
    center: Point,
    normal: mycad_kernel::geometry::Vec3,
    radius: f64,
    t_start: f64,
) -> Vec<(f64, [f64; 3])> {
    let mut raw: Vec<(f64, [f64; 3])> = Vec::new();
    for &v_idx in candidates {
        let p = mesh.positions[v_idx];
        let (t, dist) = project_to_circle(p, center, normal, radius, t_start);
        if dist <= LENGTH_TOLERANCE {
            raw.push((t, p));
        }
    }
    raw.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));

    // ソート後に隣接する同一位置 (LENGTH_TOLERANCE 以内) を dedup。
    let mut deduped: Vec<(f64, [f64; 3])> = Vec::with_capacity(raw.len());
    for (t, p) in raw {
        if let Some(&(_, last_p)) = deduped.last() {
            let dx = p[0] - last_p[0];
            let dy = p[1] - last_p[1];
            let dz = p[2] - last_p[2];
            let d = (dx * dx + dy * dy + dz * dz).sqrt();
            if d <= LENGTH_TOLERANCE {
                continue;
            }
        }
        deduped.push((t, p));
    }
    deduped
}

/// Circle edge の両側面サンプル列を直接比較。検査した Circle edge 数を返す。
fn assert_shared_boundary(solid: &Solid, label: &str) -> usize {
    let mut named = solid.clone();
    assign_unique_face_names(&mut named);
    let mesh = tessellate_solid(&named).expect("tessellate should succeed");

    // face_idx → 頂点集合
    let mut face_vertices: Vec<HashSet<usize>> = Vec::new();
    for face in named.faces.iter() {
        let canonical = face
            .name
            .as_ref()
            .map(|n| n.canonical_name())
            .unwrap_or_default();
        face_vertices.push(collect_face_vertex_set(&mesh, &canonical));
    }

    let he_to_face = build_he_to_face(&named);
    let mut circle_edges_checked = 0;

    for (edge_idx, edge) in named.edges.iter().enumerate() {
        if let Curve::Circle {
            center,
            normal,
            radius,
        } = edge.curve
        {
            // 2つの half_edge を探す
            let mut he_indices = Vec::new();
            for (he_idx, he) in named.half_edges.iter().enumerate() {
                if he.edge == edge_idx {
                    he_indices.push(he_idx);
                }
            }

            // 各 Edge は HalfEdge を正確に 2 本持つ (B-rep 不変条件 — crates/mycad-kernel/CLAUDE.md
            // §Key Invariants 「すべての Edge は正確に 2 つの HalfEdge を持つ」)。
            // ここで `len() != 2` は本 acceptance の skip 条件ではなく invariant 違反なので即失敗させる。
            assert_eq!(
                he_indices.len(),
                2,
                "{label}: edge {edge_idx} (Curve::Circle) has {} HE (expected 2 — B-rep invariant)",
                he_indices.len()
            );

            let he_a = he_indices[0];
            let he_b = he_indices[1];

            // HE は必ず Loop に属し、Loop は必ず Face に属する (B-rep 不変条件)。
            // face 未解決は invariant 違反として silent skip ではなく即失敗させる
            // (Codex review #144-r2 §F01: face 未解決時の握りつぶし防止)。
            let fa = *he_to_face
                .get(&he_a)
                .unwrap_or_else(|| panic!("{label}: edge {edge_idx} HE {he_a} has no parent face"));
            let fb = *he_to_face
                .get(&he_b)
                .unwrap_or_else(|| panic!("{label}: edge {edge_idx} HE {he_b} has no parent face"));

            if fa == fb {
                // self-adjacent (seam) — 本テストの焦点外 (cross-face 境界のみ検査)
                continue;
            }

            // 異なる面の境界 — 比較実行
            let t_start = edge.t_range[0];
            let verts_a = &face_vertices[fa];
            let verts_b = &face_vertices[fb];

            let sorted_a =
                vertices_on_circle_sorted(&mesh, verts_a, center, normal, radius, t_start);
            let sorted_b =
                vertices_on_circle_sorted(&mesh, verts_b, center, normal, radius, t_start);

            // 両側とも空になるケースは「比較が成立していない」状態 (face_ids 逆引き or
            // 円上フィルタの破綻) なので、`assert_eq!(0, 0)` で vacuous pass しないよう
            // 個別に非空 assert する (Codex review #144-r2 §F01)。
            assert!(
                !sorted_a.is_empty(),
                "{label}: edge {edge_idx} face {fa} has 0 boundary samples on circle (expected ≥ 1 — face_ids 逆引き or 円上フィルタが破綻している可能性)"
            );
            assert!(
                !sorted_b.is_empty(),
                "{label}: edge {edge_idx} face {fb} has 0 boundary samples on circle (expected ≥ 1 — face_ids 逆引き or 円上フィルタが破綻している可能性)"
            );

            assert_eq!(
                sorted_a.len(),
                sorted_b.len(),
                "{label}: edge {edge_idx} sample count differs: face {fa} has {} samples, face {fb} has {} samples",
                sorted_a.len(),
                sorted_b.len()
            );

            for (i, ((t_a, p_a), (t_b, p_b))) in sorted_a.iter().zip(sorted_b.iter()).enumerate() {
                let dx = p_a[0] - p_b[0];
                let dy = p_a[1] - p_b[1];
                let dz = p_a[2] - p_b[2];
                let dist = (dx * dx + dy * dy + dz * dz).sqrt();
                assert!(
                    dist <= LENGTH_TOLERANCE,
                    "{label}: edge {edge_idx} sample {i}: t_a={t_a:.6}, t_b={t_b:.6}, p_a=({:.10},{:.10},{:.10}), p_b=({:.10},{:.10},{:.10}), distance={dist:.3e} mm > LENGTH_TOLERANCE ({:.0e})",
                    p_a[0], p_a[1], p_a[2], p_b[0], p_b[1], p_b[2], LENGTH_TOLERANCE
                );
            }

            // 実サンプル比較が成立した edge のみカウント (両側非空 + 同数 + 全 LENGTH_TOLERANCE 一致)
            circle_edges_checked += 1;
        }
    }

    circle_edges_checked
}

/// box(10³) − cyl(r=2, h=15, c=(0,0,-7.5)) Cut
/// 円柱が箱を貫通するため Cut 結果は **through-hole**:
///   - 箱の上下 2 面に inner loop (穴) が生成され、穴の境界は **円**
///   - cylinder lateral face (箱内部 5mm 分) の上下境界も同じ円
///   - → 上下それぞれで「箱の穴境界」と「cylinder 側面境界」が cross-face で円を共有
/// (`box(10³) - sphere(r=3, origin)` は sphere が完全内包で void shell となり、Cut の
/// cross-face 円境界を持たないため fixture として不適切。Codex review #144-F02 参照)
fn make_box_cut_cyl_through() -> Solid {
    let mut gen = IdGenerator::new(0);
    let box_solid = make_cuboid(10.0, 10.0, 10.0, &mut gen).expect("cuboid");
    let cyl = make_cylinder(2.0, 15.0, Point::new(0.0, 0.0, -7.5), &mut gen).expect("cylinder");
    boolean(&box_solid, &cyl, BooleanOp::Cut, &mut gen).expect("box - cyl through-hole cut")
}

/// cyl(r=3,h=20) ∩ sphere(r=4, c=(0,0,10))
fn make_cyl_intersect_sphere() -> Solid {
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
        use mycad_kernel::geometry::surface::Surface;
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

/// box(10³) ∪ cyl(r=2, h=15, c=(0,0,-7.5))
fn make_box_fuse_cyl() -> Solid {
    let mut gen = IdGenerator::new(0);
    let box_solid = make_cuboid(10.0, 10.0, 10.0, &mut gen).expect("cuboid");
    let cyl = make_cylinder(2.0, 15.0, Point::new(0.0, 0.0, -7.5), &mut gen).unwrap();
    boolean(&box_solid, &cyl, BooleanOp::Fuse, &mut gen).expect("fuse box cyl")
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

/// T01: 決定性 — `make_box_cut_cyl_through` を 2 回ビルド → mesh が byte-identical
#[test]
fn t01_determinism() {
    let solid_a = make_box_cut_cyl_through();
    let mesh_a = tessellate_solid(&solid_a).expect("tessellate A");

    let solid_b = make_box_cut_cyl_through();
    let mesh_b = tessellate_solid(&solid_b).expect("tessellate B");

    assert_eq!(
        mesh_a.positions, mesh_b.positions,
        "positions must be identical"
    );
    assert_eq!(mesh_a.normals, mesh_b.normals, "normals must be identical");
    assert_eq!(mesh_a.indices, mesh_b.indices, "indices must be identical");
}

/// T02: Cut — `make_box_cut_cyl_through` → Circle edge ≥ 1 (上下それぞれの through-hole 境界)
#[test]
fn t02_cut_box_cyl_through_shared_boundary() {
    let solid = make_box_cut_cyl_through();
    let count = assert_shared_boundary(&solid, "T02");
    assert!(
        count > 0,
        "T02: box - cyl through-hole Cut は穴の上下端で cross-face Circle 境界を持つはず, got {count}"
    );
}

/// T03: Intersect — `make_cyl_intersect_sphere` → Circle edge ≥ 1
#[test]
fn t03_intersect_cyl_sphere_shared_boundary() {
    let solid = make_cyl_intersect_sphere();
    let count = assert_shared_boundary(&solid, "T03");
    assert!(
        count > 0,
        "T03: expected at least 1 Circle edge, got {count}"
    );
}

/// T04: Fuse — `make_box_fuse_cyl` → Circle edge ≥ 1
#[test]
fn t04_fuse_box_cyl_shared_boundary() {
    let solid = make_box_fuse_cyl();
    let count = assert_shared_boundary(&solid, "T04");
    assert!(
        count > 0,
        "T04: expected at least 1 Circle edge, got {count}"
    );
}

/// T05: degen — sphere 単体 (seam edge = self-adjacent) で skip 経路、panic なし、戻り値 = 0
#[test]
fn t05_degen_self_adjacent_seam_no_panic() {
    let mut gen = IdGenerator::new(0);
    let sphere = make_sphere(3.0, Point::origin(), &mut gen).expect("sphere");
    let count = assert_shared_boundary(&sphere, "T05");
    assert_eq!(
        count, 0,
        "T05: seam edge should be skipped, got {count} Circle edges"
    );
}

/// T06: boundary — cuboid 単体 (Line edge のみ、Circle edge 0) で vacuous pass、戻り値 = 0
#[test]
fn t06_boundary_pure_cuboid_no_circle_edges() {
    let mut gen = IdGenerator::new(0);
    let cuboid = make_cuboid(10.0, 10.0, 10.0, &mut gen).expect("cuboid");
    let count = assert_shared_boundary(&cuboid, "T06");
    assert_eq!(count, 0, "T06: cuboid has no Circle edges, got {count}");
}

/// T07: 繰り返し決定性 — `make_box_cut_cyl_through` を 100 回ビルド → 全て byte-identical
#[test]
fn t07_repeat_determinism_100_runs() {
    const RUNS: usize = 100;

    let solid_ref = make_box_cut_cyl_through();
    let mesh_ref = tessellate_solid(&solid_ref).expect("tessellate reference");

    for i in 0..RUNS {
        let solid = make_box_cut_cyl_through();
        let mesh = tessellate_solid(&solid).expect(&format!("tessellate run {i}"));

        assert_eq!(
            mesh.positions, mesh_ref.positions,
            "run {i}: positions differ from reference"
        );
        assert_eq!(
            mesh.normals, mesh_ref.normals,
            "run {i}: normals differ from reference"
        );
        assert_eq!(
            mesh.indices, mesh_ref.indices,
            "run {i}: indices differ from reference"
        );
    }
}
