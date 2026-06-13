/// Acceptance tests for TriangleMesh::face_ids (Issue #92)
use engawa_kernel::{
    primitives::{make_cuboid, make_cylinder, make_sphere},
    tessellation::tessellate_solid,
    topology::IdGenerator,
};

use engawa_kernel::geometry::Point;

#[test]
fn t01_determinism() {
    // T01: 同一入力を2回実行し face_ids が完全一致
    let mesh1 = {
        let mut idgen = IdGenerator::new(0);
        let solid = make_cuboid(2.0, 3.0, 4.0, &mut idgen).unwrap();
        tessellate_solid(&solid).unwrap()
    };
    let mesh2 = {
        let mut idgen = IdGenerator::new(0);
        let solid = make_cuboid(2.0, 3.0, 4.0, &mut idgen).unwrap();
        tessellate_solid(&solid).unwrap()
    };
    assert_eq!(mesh1.face_ids, mesh2.face_ids);
}

#[test]
fn t02_length_invariant_cuboid() {
    // T02: cuboid の face_ids.len() == triangle_count()
    let mut idgen = IdGenerator::new(0);
    let solid = make_cuboid(1.0, 2.0, 3.0, &mut idgen).unwrap();
    let mesh = tessellate_solid(&solid).unwrap();
    assert_eq!(mesh.face_ids.len(), mesh.triangle_count());
}

#[test]
fn t03_cuboid_face_ids_correct() {
    // T03: cuboid の各 face_id が N(...;F:...) 形式の非空文字列、6種のidが出現
    let mut idgen = IdGenerator::new(0);
    let solid = make_cuboid(1.0, 1.0, 1.0, &mut idgen).unwrap();
    let mesh = tessellate_solid(&solid).unwrap();

    for id in &mesh.face_ids {
        assert!(!id.is_empty(), "face_id should not be empty for cuboid");
        assert!(
            id.starts_with("N("),
            "face_id should start with N(, got: {id}"
        );
    }

    let unique: std::collections::HashSet<_> = mesh.face_ids.iter().collect();
    assert_eq!(unique.len(), 6, "cuboid has 6 faces");
}

#[test]
fn t04_length_invariant_cylinder_sphere() {
    // T04: cylinder / sphere でも face_ids.len() == triangle_count()
    {
        let mut idgen = IdGenerator::new(0);
        let cyl = make_cylinder(1.0, 2.0, Point::origin(), &mut idgen).unwrap();
        let mesh = tessellate_solid(&cyl).unwrap();
        assert_eq!(mesh.face_ids.len(), mesh.triangle_count());
    }
    {
        let mut idgen = IdGenerator::new(0);
        let sph = make_sphere(5.0, Point::origin(), &mut idgen).unwrap();
        let mesh = tessellate_solid(&sph).unwrap();
        assert_eq!(mesh.face_ids.len(), mesh.triangle_count());
    }
}

#[test]
fn t05_boundary_unnamed_face() {
    // T05_boundary: Face.name == None の face を含む Solid で当該三角形の id が ""
    // 手動で最小限の Solid を構築する (1 triangular face, name=None)
    use engawa_kernel::brep::topology::Solid;
    use engawa_kernel::geometry::curve::Curve;
    use engawa_kernel::geometry::surface::Surface;
    use engawa_kernel::geometry::Vec3;

    let mut s = Solid::new(0);

    // 3 vertices of a triangle on z=0 plane
    let v0 = s.add_vertex(1, Point::new(0.0, 0.0, 0.0), None);
    let v1 = s.add_vertex(2, Point::new(1.0, 0.0, 0.0), None);
    let v2 = s.add_vertex(3, Point::new(0.0, 1.0, 0.0), None);

    // 3 edges (line segments)
    let e0 = s.add_edge(
        10,
        [v0, v1],
        Curve::Line {
            origin: Point::new(0.0, 0.0, 0.0),
            direction: Vec3::x(),
        },
        [0.0, 1.0],
        None,
    );
    let e1 = s.add_edge(
        11,
        [v1, v2],
        Curve::Line {
            origin: Point::new(1.0, 0.0, 0.0),
            direction: Vec3::new(-1.0, 1.0, 0.0),
        },
        [0.0, 1.0],
        None,
    );
    let e2 = s.add_edge(
        12,
        [v2, v0],
        Curve::Line {
            origin: Point::new(0.0, 1.0, 0.0),
            direction: Vec3::new(0.0, -1.0, 0.0),
        },
        [0.0, 1.0],
        None,
    );

    // 3 half-edges forming a loop
    let he0 = s.add_half_edge(20, v0, e0, true);
    let he1 = s.add_half_edge(21, v1, e1, true);
    let he2 = s.add_half_edge(22, v2, e2, true);

    let lp = s.add_loop(30, vec![he0, he1, he2]);

    // 1 face with name=None (the key test condition)
    s.add_face(
        40,
        Surface::Plane {
            origin: Point::origin(),
            normal: Vec3::z(),
            u_axis: Vec3::x(),
            v_axis: Vec3::y(),
        },
        lp,
        vec![],
        true,
        None, // ← unnamed face
    );
    s.add_shell(50, vec![0], true);

    let mesh = tessellate_solid(&s).unwrap();

    // All face_ids should be empty strings for this unnamed face
    assert!(mesh.triangle_count() > 0, "should have at least 1 triangle");
    for id in &mesh.face_ids {
        assert!(
            id.is_empty(),
            "unnamed face should produce empty face_id, got: {id}"
        );
    }
    assert_eq!(mesh.face_ids.len(), mesh.triangle_count());
}

#[test]
fn t06_degen_skip_invariant() {
    // T06_degen: 退化三角形がスキップされる入力でも face_ids.len() == triangle_count()
    // tiny sphere (r=1e-10) produces all degenerate triangles → 0 triangles, 0 face_ids
    let mut idgen = IdGenerator::new(0);
    let sph = make_sphere(1e-10, Point::origin(), &mut idgen).unwrap();
    let mesh = tessellate_solid(&sph).unwrap();
    assert_eq!(
        mesh.triangle_count(),
        0,
        "tiny sphere should have 0 triangles"
    );
    assert_eq!(mesh.face_ids.len(), 0, "face_ids should also be empty");
    assert_eq!(mesh.face_ids.len(), mesh.triangle_count());
}
