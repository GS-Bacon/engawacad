// Acceptance tests for Issue #129:
// Cylinder face UV-grid seam alignment with adjacent cap/hole boundaries.
//
// Root cause: tessellate_face_uv_grid uses u_min=0.0 (hardcoded), which diverges
// from the cap face's circle-edge t_range start after boolean seam relocation.
// Fix: derive u_min from the circle edge's t_range in the outer loop.

use mycad_kernel::booleans::{boolean, BooleanOp};
use mycad_kernel::brep::topology::IdGenerator;
use mycad_kernel::geometry::Point;
use mycad_kernel::primitives::{make_cuboid, make_cylinder};
use mycad_kernel::tessellation::{tessellate_solid, TriangleMesh};

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Signed volume of a mesh (divergence theorem on triangles).
fn signed_volume(mesh: &TriangleMesh) -> f64 {
    let mut vol = 0.0_f64;
    for tri in 0..mesh.indices.len() / 3 {
        let i0 = mesh.indices[tri * 3] as usize;
        let i1 = mesh.indices[tri * 3 + 1] as usize;
        let i2 = mesh.indices[tri * 3 + 2] as usize;
        let p0 = mesh.positions[i0];
        let p1 = mesh.positions[i1];
        let p2 = mesh.positions[i2];
        vol += p0[0] * (p1[1] * p2[2] - p2[1] * p1[2]);
        vol += p1[0] * (p2[1] * p0[2] - p0[1] * p2[2]);
        vol += p2[0] * (p0[1] * p1[2] - p1[1] * p0[2]);
    }
    vol / 6.0
}

/// Weld vertices within epsilon and check that every undirected edge
/// is shared by exactly 2 triangles.
fn assert_watertight_welded(positions: &[[f64; 3]], indices: &[u32], eps: f64, label: &str) {
    let n = positions.len();
    let mut parent = (0..n).collect::<Vec<usize>>();
    fn find(parent: &mut Vec<usize>, i: usize) -> usize {
        if parent[i] != i {
            parent[i] = find(parent, parent[i]);
        }
        parent[i]
    }
    for i in 0..n {
        for j in (i + 1)..n {
            let pi = &positions[i];
            let pj = &positions[j];
            if (pi[0] - pj[0]).abs() < eps
                && (pi[1] - pj[1]).abs() < eps
                && (pi[2] - pj[2]).abs() < eps
            {
                let ri = find(&mut parent, i);
                let rj = find(&mut parent, j);
                if ri != rj {
                    let new_root = ri.min(rj);
                    parent[ri] = new_root;
                    parent[rj] = new_root;
                }
            }
        }
    }
    let remapped: Vec<u32> = indices
        .iter()
        .map(|&idx| find(&mut parent, idx as usize) as u32)
        .collect();
    let tri_count = remapped.len() / 3;

    let mut edge_count: std::collections::HashMap<[u32; 2], usize> =
        std::collections::HashMap::new();

    for tri in 0..tri_count {
        let i0 = remapped[tri * 3];
        let i1 = remapped[tri * 3 + 1];
        let i2 = remapped[tri * 3 + 2];
        if i0 == i1 || i1 == i2 || i2 == i0 {
            continue;
        }
        for edge in &[[i0, i1], [i1, i2], [i2, i0]] {
            let key = if edge[0] < edge[1] {
                [edge[0], edge[1]]
            } else {
                [edge[1], edge[0]]
            };
            *edge_count.entry(key).or_insert(0) += 1;
        }
    }

    let bad: Vec<_> = edge_count.iter().filter(|(_, &count)| count != 2).collect();
    assert!(
        bad.is_empty(),
        "{label}: {} edges not shared by exactly 2 triangles (e.g. {:?})",
        bad.len(),
        bad.first()
    );
}

/// Build fuse solid: box(10³) ∪ cylinder(r=2, h=15, origin=(0,0,-7.5)).
fn build_fuse_box_cyl() -> mycad_kernel::brep::topology::Solid {
    let mut gen = IdGenerator::new(0);
    let box_solid = make_cuboid(10.0, 10.0, 10.0, &mut gen).unwrap();
    let cyl = make_cylinder(2.0, 15.0, Point::new(0.0, 0.0, -7.5), &mut gen).unwrap();
    boolean(&box_solid, &cyl, BooleanOp::Fuse, &mut gen).expect("fuse box cyl should succeed")
}

/// Build intersect solid: box(10³) ∩ cylinder(r=2, h=15, origin=(0,0,-7.5)).
fn build_intersect_box_cyl() -> mycad_kernel::brep::topology::Solid {
    let mut gen = IdGenerator::new(0);
    let box_solid = make_cuboid(10.0, 10.0, 10.0, &mut gen).unwrap();
    let cyl = make_cylinder(2.0, 15.0, Point::new(0.0, 0.0, -7.5), &mut gen).unwrap();
    boolean(&box_solid, &cyl, BooleanOp::Intersect, &mut gen)
        .expect("intersect box cyl should succeed")
}

// ---------------------------------------------------------------------------
// T01: Determinism — fuse_box_cyl produces identical mesh on two runs
// ---------------------------------------------------------------------------
#[test]
fn t01_determinism_fuse_box_cyl() {
    let solid1 = build_fuse_box_cyl();
    let solid2 = build_fuse_box_cyl();

    let m1 = tessellate_solid(&solid1).expect("tessellate 1");
    let m2 = tessellate_solid(&solid2).expect("tessellate 2");

    assert_eq!(m1.positions, m2.positions, "T01: positions differ");
    assert_eq!(m1.normals, m2.normals, "T01: normals differ");
    assert_eq!(m1.indices, m2.indices, "T01: indices differ");
}

// ---------------------------------------------------------------------------
// T03_boundary: Boundary alignment — cylinder seam boundary points match
//               adjacent cap boundary points within 1e-9
// ---------------------------------------------------------------------------
#[test]
fn t03_boundary_cylinder_cap_alignment() {
    let solid = build_fuse_box_cyl();
    let mesh = tessellate_solid(&solid).expect("tessellate fuse box cyl");

    // Collect all boundary ring positions from the cylinder lateral face.
    // After fix A, the cylinder UV grid starts at the same seam angle as the
    // cap face's circle edge, so the v_min and v_max rows of the UV grid
    // should be within floating-point epsilon of the cap boundary points.
    //
    // We verify by checking the mesh is watertight with a tight tolerance,
    // which implies boundary alignment.
    let eps = 1e-9;

    // Find all positions at z ≈ -7.5 (cylinder bottom) and z ≈ 7.5 (cylinder top)
    // These are the boundary rows shared between cylinder lateral and cap faces.
    let z_bottom = -7.5_f64;
    let z_top = 7.5_f64;

    let bottom_pts: Vec<usize> = mesh
        .positions
        .iter()
        .enumerate()
        .filter(|(_, p)| (p[2] - z_bottom).abs() < 0.01)
        .map(|(i, _)| i)
        .collect();
    let top_pts: Vec<usize> = mesh
        .positions
        .iter()
        .enumerate()
        .filter(|(_, p)| (p[2] - z_top).abs() < 0.01)
        .map(|(i, _)| i)
        .collect();

    assert!(
        !bottom_pts.is_empty(),
        "T03: no points found at z ≈ {z_bottom}"
    );
    assert!(!top_pts.is_empty(), "T03: no points found at z ≈ {z_top}");

    // Every boundary position at the cylinder top/bottom should be within eps
    // of at least one other position (welded pair from lateral + cap).
    // Since positions are not welded in the mesh, we check that pairs from
    // different triangles sharing the same geometric point are within eps.
    let boundary_pts: Vec<usize> = bottom_pts.iter().chain(top_pts.iter()).copied().collect();

    // For each boundary point, verify it's close to at least one other point
    // (its counterpart from the adjacent face).
    for &i in &boundary_pts {
        let pi = &mesh.positions[i];
        let has_neighbor = boundary_pts.iter().any(|&j| {
            j != i
                && (pi[0] - mesh.positions[j][0]).abs() < eps
                && (pi[1] - mesh.positions[j][1]).abs() < eps
                && (pi[2] - mesh.positions[j][2]).abs() < eps
        });
        assert!(
            has_neighbor,
            "T03: boundary point {i} at ({:.6}, {:.6}, {:.6}) has no matching point within {eps}",
            pi[0], pi[1], pi[2]
        );
    }
}

// ---------------------------------------------------------------------------
// T04: No NaN / degenerate triangles in fuse_box_cyl mesh
// ---------------------------------------------------------------------------
#[test]
fn t04_no_nan_degenerate_fuse_box_cyl() {
    let solid = build_fuse_box_cyl();
    let mesh = tessellate_solid(&solid).expect("tessellate fuse box cyl");

    assert!(
        mesh.triangle_count() > 0,
        "T04: fuse box cyl should produce triangles"
    );
    assert!(
        mesh.positions
            .iter()
            .all(|p| p.iter().all(|x| x.is_finite())),
        "T04: no NaN/Inf in positions"
    );
    assert!(
        mesh.normals.iter().all(|n| n.iter().all(|x| x.is_finite())),
        "T04: no NaN/Inf in normals"
    );
    let vol = signed_volume(&mesh);
    assert!(
        vol.is_finite() && vol != 0.0,
        "T04: signed volume should be finite non-zero, got {vol}"
    );
}

// ---------------------------------------------------------------------------
// T05_intersect: box∩cyl intersect produces planar caps; cross-face adjacency (#131)
// selects n_u = arcs_per_rev so the cylinder seam rows align with the planar cap
// boundary. This is the planar-cap regression for the adjacency logic (sphere-cap
// branch is covered by tessellation_cap_acceptance::t02_watertight_intersect).
// ---------------------------------------------------------------------------
#[test]
fn t05_intersect_box_cyl_watertight() {
    let solid = build_intersect_box_cyl();
    let mesh = tessellate_solid(&solid).expect("tessellate intersect box cyl");

    assert!(
        mesh.triangle_count() > 0,
        "T05: intersect box cyl should produce triangles"
    );
    assert!(
        mesh.positions
            .iter()
            .all(|p| p.iter().all(|x| x.is_finite())),
        "T05: no NaN/Inf in positions"
    );
    assert_watertight_welded(
        &mesh.positions,
        &mesh.indices,
        1e-6,
        "t05_intersect_box_cyl",
    );
}
