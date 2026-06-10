// Acceptance tests for Issue #56: tessellation cap winding fix
// Tests T01–T07 as specified in the plan.

use mycad_kernel::booleans::{boolean, BooleanOp};
use mycad_kernel::brep::topology::IdGenerator;
use mycad_kernel::geometry::curve::Curve;
use mycad_kernel::geometry::surface::Surface;
use mycad_kernel::geometry::Point;
use mycad_kernel::primitives::{make_cuboid, make_cylinder, make_sphere};
use mycad_kernel::tessellation::tessellate_solid;

/// Shift all geometry in a solid by (dx, dy, dz).
fn shift_solid(solid: &mut mycad_kernel::brep::topology::Solid, dx: f64, dy: f64, dz: f64) {
    for v in &mut solid.vertices {
        v.point.coords.x += dx;
        v.point.coords.y += dy;
        v.point.coords.z += dz;
    }
    for e in &mut solid.edges {
        if let Curve::Circle { center, .. } = &mut e.curve {
            center.coords.x += dx;
            center.coords.y += dy;
            center.coords.z += dz;
        }
    }
    for f in &mut solid.faces {
        match &mut f.surface {
            Surface::Sphere { center, .. }
            | Surface::Plane { origin: center, .. }
            | Surface::Cylinder { origin: center, .. } => {
                center.coords.x += dx;
                center.coords.y += dy;
                center.coords.z += dz;
            }
            _ => {}
        }
    }
}

/// Build intersect_cyl_sphere: cylinder(r=3, h=20) at origin,
/// sphere(r=4) centered at (0,0,10).
fn build_intersect_cyl_sphere() -> mycad_kernel::brep::topology::Solid {
    let mut gen = IdGenerator::new(0);
    let cyl = make_cylinder(3.0, 20.0, Point::origin(), &mut gen).unwrap();
    let mut sph = make_sphere(4.0, Point::origin(), &mut gen).unwrap();
    shift_solid(&mut sph, 0.0, 0.0, 10.0);
    boolean(&cyl, &sph, BooleanOp::Intersect, &mut gen).expect("intersect should succeed")
}

/// Raw signed mesh volume (no abs). Positive = outward-consistent winding.
fn signed_volume(mesh: &mycad_kernel::tessellation::TriangleMesh) -> f64 {
    let mut vol = 0.0_f64;
    for tri in 0..mesh.triangle_count() {
        let i0 = mesh.indices[tri * 3] as usize;
        let i1 = mesh.indices[tri * 3 + 1] as usize;
        let i2 = mesh.indices[tri * 3 + 2] as usize;
        let p0 = &mesh.positions[i0];
        let p1 = &mesh.positions[i1];
        let p2 = &mesh.positions[i2];
        vol += (p0[0] * (p1[1] * p2[2] - p2[1] * p1[2])
            + p1[0] * (p2[1] * p0[2] - p0[1] * p2[2])
            + p2[0] * (p0[1] * p1[2] - p1[1] * p0[2]))
            / 6.0;
    }
    vol
}

/// Check outward normals using mesh centroid (works for convex shapes).
fn assert_outward_normals(mesh: &mycad_kernel::tessellation::TriangleMesh, label: &str) {
    let n = mesh.positions.len() as f64;
    let cx: f64 = mesh.positions.iter().map(|p| p[0]).sum();
    let cy: f64 = mesh.positions.iter().map(|p| p[1]).sum();
    let cz: f64 = mesh.positions.iter().map(|p| p[2]).sum();
    let centroid = [cx / n, cy / n, cz / n];

    for tri in 0..mesh.triangle_count() {
        let i0 = mesh.indices[tri * 3] as usize;
        let i1 = mesh.indices[tri * 3 + 1] as usize;
        let i2 = mesh.indices[tri * 3 + 2] as usize;
        let p0 = mesh.positions[i0];
        let p1 = mesh.positions[i1];
        let p2 = mesh.positions[i2];

        let fc = [
            (p0[0] + p1[0] + p2[0]) / 3.0,
            (p0[1] + p1[1] + p2[1]) / 3.0,
            (p0[2] + p1[2] + p2[2]) / 3.0,
        ];

        let u = [p1[0] - p0[0], p1[1] - p0[1], p1[2] - p0[2]];
        let v = [p2[0] - p0[0], p2[1] - p0[1], p2[2] - p0[2]];
        let nx = u[1] * v[2] - u[2] * v[1];
        let ny = u[2] * v[0] - u[0] * v[2];
        let nz = u[0] * v[1] - u[1] * v[0];

        let dx = fc[0] - centroid[0];
        let dy = fc[1] - centroid[1];
        let dz = fc[2] - centroid[2];
        let dot = nx * dx + ny * dy + nz * dz;

        assert!(
            dot > 0.0,
            "{label} facet {tri}: outward dot={dot} fc=({:.3},{:.3},{:.3})",
            fc[0],
            fc[1],
            fc[2]
        );
    }
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

/// T01: Determinism — intersect_cyl_sphere tessellated twice yields identical mesh.
#[test]
fn t01_determinism() {
    let solid1 = build_intersect_cyl_sphere();
    let solid2 = build_intersect_cyl_sphere();

    let m1 = tessellate_solid(&solid1).expect("tessellate 1");
    let m2 = tessellate_solid(&solid2).expect("tessellate 2");

    assert_eq!(m1.positions, m2.positions, "T01: positions differ");
    assert_eq!(m1.normals, m2.normals, "T01: normals differ");
    assert_eq!(m1.indices, m2.indices, "T01: indices differ");
}

/// T02: Watertight — intersect_cyl_sphere after vertex welding.
#[test]
fn t02_watertight_intersect() {
    let solid = build_intersect_cyl_sphere();
    let mesh = tessellate_solid(&solid).expect("tessellate intersect");
    assert_watertight_welded(&mesh.positions, &mesh.indices, 1e-6, "intersect_cyl_sphere");
}

/// T03: Outward normals — intersect_cyl_sphere all facet normals point outward.
#[test]
fn t03_outward_normals_intersect() {
    let solid = build_intersect_cyl_sphere();
    let mesh = tessellate_solid(&solid).expect("tessellate intersect");
    assert_outward_normals(&mesh, "intersect_cyl_sphere");
}

/// T04: box(10³) − cylinder(r=2, h=6) at origin — watertight via welded vertex check.
#[test]
fn t04_watertight_cut_hole() {
    let mut gen = IdGenerator::new(0);
    let box_solid = make_cuboid(10.0, 10.0, 10.0, &mut gen).unwrap();
    let cyl = make_cylinder(2.0, 6.0, Point::origin(), &mut gen).unwrap();
    let solid =
        boolean(&box_solid, &cyl, BooleanOp::Cut, &mut gen).expect("cut cylinder should succeed");

    let mesh = tessellate_solid(&solid).expect("tessellate cut cylinder");
    assert!(
        mesh.triangle_count() > 0,
        "T04: cut cylinder should produce triangles"
    );
    assert!(
        mesh.positions
            .iter()
            .all(|p| p.iter().all(|x| x.is_finite())),
        "T04: no NaN/Inf in positions"
    );
    let vol = signed_volume(&mesh);
    assert!(
        vol > 0.0,
        "T04: signed volume should be positive, got {vol}"
    );
    assert!(
        vol < 1000.0,
        "T04: volume should be less than box volume (1000), got {vol}"
    );
    assert_watertight_welded(&mesh.positions, &mesh.indices, 1e-6, "t04_cut_cylinder");
}

/// T05: box(10³) ∪ cylinder(r=2, h=15, origin=(0,0,-7.5)) — watertight via welded vertex check.
/// Previously ignored due to seam mismatch (u_min=0.0 hardcoded). Fixed in #129 by deriving
/// u_min from the circle edge's t_range, aligning UV grid seam with cap boundary.
#[test]
fn t05_watertight_fuse() {
    let mut gen = IdGenerator::new(0);
    let box_solid = make_cuboid(10.0, 10.0, 10.0, &mut gen).unwrap();
    let cyl = make_cylinder(2.0, 15.0, Point::new(0.0, 0.0, -7.5), &mut gen).unwrap();
    let solid =
        boolean(&box_solid, &cyl, BooleanOp::Fuse, &mut gen).expect("fuse box cyl should succeed");

    let mesh = tessellate_solid(&solid).expect("tessellate fuse box cyl");
    assert!(
        mesh.triangle_count() > 0,
        "T05: fuse box cyl should produce triangles"
    );
    assert!(
        mesh.positions
            .iter()
            .all(|p| p.iter().all(|x| x.is_finite())),
        "T05: no NaN/Inf in positions"
    );
    let vol = signed_volume(&mesh);
    assert!(
        vol != 0.0 && vol.is_finite(),
        "T05: signed volume should be finite non-zero, got {vol}"
    );
    assert_watertight_welded(&mesh.positions, &mesh.indices, 1e-6, "t05_fuse_box_cyl");
}

/// T06: Near-equator sphere cap — box cut with large sphere.
/// Uses box(10,10,10) - sphere(r=5, center=(5,5,7)).
/// No panic, all coordinates finite, positive signed volume.
#[test]
fn t06_degen_sphere_equator() {
    let mut gen = IdGenerator::new(0);
    let box_solid = make_cuboid(10.0, 10.0, 10.0, &mut gen).unwrap();
    let mut sph = make_sphere(5.0, Point::origin(), &mut gen).unwrap();
    shift_solid(&mut sph, 5.0, 5.0, 7.0);
    let solid = boolean(&box_solid, &sph, BooleanOp::Cut, &mut gen)
        .expect("large sphere cut should succeed");

    let mesh = tessellate_solid(&solid).expect("tessellate large sphere cut");
    assert!(
        mesh.triangle_count() > 0,
        "T06: should produce triangles, got 0"
    );
    assert!(
        mesh.positions
            .iter()
            .all(|p| p.iter().all(|x| x.is_finite())),
        "T06: no NaN/Inf"
    );
    let vol = signed_volume(&mesh);
    assert!(
        vol > 0.0,
        "T06: signed volume should be positive, got {vol}"
    );
}

/// T07: Boundary — same_sense=false sphere cap has positive signed volume.
/// Uses the proven geometry from sphere_trimmed_volume_sign:
/// box(10,10,10) - sphere(r=3, center=(0,0,6)).
/// Positive signed volume confirms correct outward winding for the sphere cap.
#[test]
fn t07_boundary_same_sense_false() {
    let mut gen = IdGenerator::new(0);
    let box_solid = make_cuboid(10.0, 10.0, 10.0, &mut gen).unwrap();
    let mut sph = make_sphere(3.0, Point::origin(), &mut gen).unwrap();
    shift_solid(&mut sph, 0.0, 0.0, 6.0);
    let solid =
        boolean(&box_solid, &sph, BooleanOp::Cut, &mut gen).expect("box - sphere should succeed");

    let mesh = tessellate_solid(&solid).expect("tessellate cut sphere");
    let vol = signed_volume(&mesh);
    assert!(
        vol > 0.0,
        "T07: signed volume should be positive (outward winding), got {vol}"
    );
    // V_cap = 28π/3 (same as sphere_trimmed_volume_sign)
    let expected = 1000.0 - 28.0 * std::f64::consts::PI / 3.0;
    assert!(
        (vol - expected).abs() < 2.0,
        "T07: volume ≈ {expected:.1}, got {vol:.1}"
    );
}

// --- Adversarial edge-case tests (beyond spec T01–T07) ---

/// Build box(10³) − sphere(r=3, center=(0,0,6)) Cut solid.
fn build_cut_hole() -> mycad_kernel::brep::topology::Solid {
    let mut gen = IdGenerator::new(0);
    let box_solid = make_cuboid(10.0, 10.0, 10.0, &mut gen).unwrap();
    let mut sph = make_sphere(3.0, Point::origin(), &mut gen).unwrap();
    shift_solid(&mut sph, 0.0, 0.0, 6.0);
    boolean(&box_solid, &sph, BooleanOp::Cut, &mut gen).expect("cut should succeed")
}

/// Build boolean_fuse_box_cyl: make_cuboid(10,10,10) ∪ cylinder(r=2, h=15, origin=(0,0,-7.5)).
fn build_fuse() -> mycad_kernel::brep::topology::Solid {
    let mut gen = IdGenerator::new(0);
    let box_solid = make_cuboid(10.0, 10.0, 10.0, &mut gen).unwrap();
    let cyl = make_cylinder(2.0, 15.0, Point::new(0.0, 0.0, -7.5), &mut gen).unwrap();
    boolean(&box_solid, &cyl, BooleanOp::Fuse, &mut gen).expect("fuse box cyl should succeed")
}

/// T08: 100-run determinism — intersect_cyl_sphere tessellation byte-identical 100 times.
/// Covers the trimmed sphere cap path through tessellate_sphere_face_trimmed.
#[test]
fn t08_100_run_determinism_intersect() {
    let first = {
        let solid = build_intersect_cyl_sphere();
        tessellate_solid(&solid).expect("tessellate run 0")
    };
    for i in 1..100 {
        let solid = build_intersect_cyl_sphere();
        let mesh = tessellate_solid(&solid).expect("tessellate run {i}");
        assert_eq!(first.positions, mesh.positions, "run {i}: positions differ");
        assert_eq!(first.normals, mesh.normals, "run {i}: normals differ");
        assert_eq!(first.indices, mesh.indices, "run {i}: indices differ");
    }
}

/// T09: Finite coordinates — box − sphere cut produces no NaN/Inf in any mesh vertex.
/// Box-sphere booleans share vertices between planar and sphere-cap faces via
/// different sampling paths, so strict watertight is not guaranteed.
/// Adversarial check: at least all coordinates are finite.
#[test]
fn t09_finite_coords_cut_hole() {
    let solid = build_cut_hole();
    let mesh = tessellate_solid(&solid).expect("tessellate cut hole");
    assert!(
        mesh.triangle_count() > 0,
        "T09: cut hole should produce triangles"
    );
    assert!(
        mesh.positions
            .iter()
            .all(|p| p.iter().all(|x| x.is_finite())),
        "T09: no NaN/Inf in positions"
    );
    assert!(
        mesh.normals.iter().all(|n| n.iter().all(|x| x.is_finite())),
        "T09: no NaN/Inf in normals"
    );
}

/// T10: Finite coordinates — box ∩ sphere fuse produces no NaN/Inf.
#[test]
fn t10_finite_coords_fuse() {
    let solid = build_fuse();
    let mesh = tessellate_solid(&solid).expect("tessellate fuse");
    assert!(
        mesh.triangle_count() > 0,
        "T10: fuse should produce triangles"
    );
    assert!(
        mesh.positions
            .iter()
            .all(|p| p.iter().all(|x| x.is_finite())),
        "T10: no NaN/Inf in positions"
    );
    assert!(
        mesh.normals.iter().all(|n| n.iter().all(|x| x.is_finite())),
        "T10: no NaN/Inf in normals"
    );
}

/// T11: YAML roundtrip — boolean solid → serialize → deserialize → tessellate → identical.
/// Verifies that the trimmed sphere topology survives YAML roundtrip.
#[test]
fn t11_yaml_roundtrip_boolean_tessellation() {
    let solid = build_intersect_cyl_sphere();
    let m1 = tessellate_solid(&solid).expect("original tessellate");

    let yaml = serde_yaml::to_string(&solid).expect("YAML serialize");
    let restored: mycad_kernel::brep::topology::Solid =
        serde_yaml::from_str(&yaml).expect("YAML deserialize");
    let m2 = tessellate_solid(&restored).expect("restored tessellate");

    assert_eq!(
        m1.positions, m2.positions,
        "YAML roundtrip: positions differ"
    );
    assert_eq!(m1.normals, m2.normals, "YAML roundtrip: normals differ");
    assert_eq!(m1.indices, m2.indices, "YAML roundtrip: indices differ");
}

/// T12: Signed volume positive for cut hole with different sphere positions.
/// Adversarial variant: sphere center shifted to (0,0,8) for a shallower dimple.
#[test]
fn t12_shallow_dimple_positive_volume() {
    let mut gen = IdGenerator::new(0);
    let box_solid = make_cuboid(10.0, 10.0, 10.0, &mut gen).unwrap();
    let mut sph = make_sphere(3.0, Point::origin(), &mut gen).unwrap();
    shift_solid(&mut sph, 0.0, 0.0, 8.0);
    let solid = boolean(&box_solid, &sph, BooleanOp::Cut, &mut gen)
        .expect("shallow dimple cut should succeed");
    let mesh = tessellate_solid(&solid).expect("tessellate shallow dimple");
    let vol = signed_volume(&mesh);
    assert!(
        vol > 0.0,
        "T12: shallow dimple signed volume should be positive, got {vol}"
    );
    assert!(
        mesh.positions
            .iter()
            .all(|p| p.iter().all(|x| x.is_finite())),
        "T12: no NaN/Inf"
    );
}

/// T13: Outward winding — box ∪ cylinder fuse. Signed volume > 0 confirms outward winding.
/// Uses signed_volume instead of centroid-based check because fuse shape is non-convex.
#[test]
fn t13_outward_normals_fuse() {
    let solid = build_fuse();
    let mesh = tessellate_solid(&solid).expect("tessellate fuse");
    let vol = signed_volume(&mesh);
    assert!(
        vol > 0.0,
        "T13: fuse signed volume should be positive (outward winding), got {vol}"
    );
}

/// T14: 100-run determinism for box − sphere cut tessellation.
#[test]
fn t14_100_run_determinism_cut_hole() {
    let first = {
        let solid = build_cut_hole();
        tessellate_solid(&solid).expect("tessellate cut 0")
    };
    for i in 1..100 {
        let solid = build_cut_hole();
        let mesh = tessellate_solid(&solid).unwrap_or_else(|_| panic!("tessellate cut {i}"));
        assert_eq!(
            first.positions, mesh.positions,
            "cut run {i}: positions differ"
        );
        assert_eq!(first.indices, mesh.indices, "cut run {i}: indices differ");
    }
}
