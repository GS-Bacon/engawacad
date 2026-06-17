/// Acceptance tests for Issue #72: 幾何型の平行移動 transform 実装
///
/// Inline tests (topology.rs) の内容を統合テストとして再実装。
/// 公開 API 経由で Solid::translate, Plane/Surface/Curve::translate を検証する。
use engawa_kernel::brep::topology::{IdGenerator, Solid};
use engawa_kernel::geometry::curve::Curve;
use engawa_kernel::geometry::math::point_near;
use engawa_kernel::geometry::surface::Surface;
use engawa_kernel::geometry::{Plane, Point, Vec3};

fn make_test_cuboid() -> Solid {
    let mut gen = IdGenerator::new(1);
    engawa_kernel::primitives::make_cuboid(2.0, 3.0, 4.0, "cuboid", &mut gen).unwrap()
}

#[test]
fn t01_determinism_solid_translate_100_runs() {
    let cuboid = make_test_cuboid();
    let offset = Vec3::new(5.0, -3.0, 7.0);
    let mut first: Option<String> = None;
    for _ in 0..100 {
        let mut s = cuboid.clone();
        s.translate(offset);
        let yaml = serde_yaml::to_string(&s).unwrap();
        match &first {
            None => first = Some(yaml),
            Some(prev) => assert_eq!(prev, &yaml, "translate output must be deterministic"),
        }
    }
}

#[test]
fn t02_translate_moves_origin_points() {
    let offset = Vec3::new(1.0, 2.0, 3.0);

    // Plane
    let plane = Plane::xy();
    let moved = plane.translate(offset);
    assert!(point_near(&moved.origin, &Point::new(1.0, 2.0, 3.0)));

    // Surface::Plane
    let sp = Surface::Plane {
        origin: Point::origin(),
        normal: Vec3::z(),
        u_axis: Vec3::x(),
        v_axis: Vec3::y(),
    };
    let sp_moved = sp.translate(offset);
    if let Surface::Plane { origin, .. } = &sp_moved {
        assert!(point_near(origin, &Point::new(1.0, 2.0, 3.0)));
    } else {
        panic!("expected Surface::Plane");
    }

    // Surface::Cylinder
    let sc = Surface::Cylinder {
        origin: Point::origin(),
        axis: Vec3::z(),
        radius: 1.0,
    };
    let sc_moved = sc.translate(offset);
    if let Surface::Cylinder { origin, .. } = &sc_moved {
        assert!(point_near(origin, &Point::new(1.0, 2.0, 3.0)));
    } else {
        panic!("expected Surface::Cylinder");
    }

    // Surface::Sphere
    let ss = Surface::Sphere {
        center: Point::origin(),
        radius: 1.0,
    };
    let ss_moved = ss.translate(offset);
    if let Surface::Sphere { center, .. } = &ss_moved {
        assert!(point_near(center, &Point::new(1.0, 2.0, 3.0)));
    } else {
        panic!("expected Surface::Sphere");
    }

    // Surface::Cone
    let sco = Surface::Cone {
        apex: Point::origin(),
        axis: Vec3::z(),
        half_angle: 0.5,
    };
    let sco_moved = sco.translate(offset);
    if let Surface::Cone { apex, .. } = &sco_moved {
        assert!(point_near(apex, &Point::new(1.0, 2.0, 3.0)));
    } else {
        panic!("expected Surface::Cone");
    }

    // Curve::Line
    let cl = Curve::Line {
        origin: Point::origin(),
        direction: Vec3::x(),
    };
    let cl_moved = cl.translate(offset);
    if let Curve::Line { origin, .. } = &cl_moved {
        assert!(point_near(origin, &Point::new(1.0, 2.0, 3.0)));
    } else {
        panic!("expected Curve::Line");
    }

    // Curve::Circle
    let cc = Curve::Circle {
        center: Point::origin(),
        normal: Vec3::z(),
        radius: 1.0,
    };
    let cc_moved = cc.translate(offset);
    if let Curve::Circle { center, .. } = &cc_moved {
        assert!(point_near(center, &Point::new(1.0, 2.0, 3.0)));
    } else {
        panic!("expected Curve::Circle");
    }
}

#[test]
fn t03_translate_axes_radii_invariant() {
    let offset = Vec3::new(10.0, 20.0, 30.0);

    // Plane: normal, u_axis, v_axis unchanged
    let plane = Plane::xy();
    let moved = plane.translate(offset);
    assert_eq!(moved.normal, plane.normal);
    assert_eq!(moved.u_axis, plane.u_axis);
    assert_eq!(moved.v_axis, plane.v_axis);

    // Surface::Cylinder: axis, radius unchanged
    let cyl = Surface::Cylinder {
        origin: Point::origin(),
        axis: Vec3::z(),
        radius: 2.5,
    };
    let cyl_moved = cyl.translate(offset);
    if let Surface::Cylinder { axis, radius, .. } = cyl_moved {
        assert_eq!(axis, Vec3::z());
        assert_eq!(radius, 2.5);
    }

    // Surface::Sphere: radius unchanged
    let sph = Surface::Sphere {
        center: Point::origin(),
        radius: 3.0,
    };
    let sph_moved = sph.translate(offset);
    if let Surface::Sphere { radius, .. } = sph_moved {
        assert_eq!(radius, 3.0);
    }

    // Surface::Cone: axis, half_angle unchanged
    let cone = Surface::Cone {
        apex: Point::origin(),
        axis: Vec3::z(),
        half_angle: 0.4,
    };
    let cone_moved = cone.translate(offset);
    if let Surface::Cone {
        axis, half_angle, ..
    } = cone_moved
    {
        assert_eq!(axis, Vec3::z());
        assert_eq!(half_angle, 0.4);
    }

    // Curve::Line: direction unchanged
    let line = Curve::Line {
        origin: Point::origin(),
        direction: Vec3::x(),
    };
    let line_moved = line.translate(offset);
    if let Curve::Line { direction, .. } = line_moved {
        assert_eq!(direction, Vec3::x());
    }

    // Curve::Circle: normal, radius unchanged
    let circle = Curve::Circle {
        center: Point::origin(),
        normal: Vec3::z(),
        radius: 1.5,
    };
    let circle_moved = circle.translate(offset);
    if let Curve::Circle { normal, radius, .. } = circle_moved {
        assert_eq!(normal, Vec3::z());
        assert_eq!(radius, 1.5);
    }
}

#[test]
fn t04_entity_id_invariant_after_translate() {
    let original = make_test_cuboid();
    let mut moved = original.clone();
    let offset = Vec3::new(1.0, 2.0, 3.0);
    moved.translate(offset);

    // Vertex IDs
    for (o, m) in original.vertices.iter().zip(moved.vertices.iter()) {
        assert_eq!(o.id, m.id, "vertex ID must be preserved");
    }
    // Edge IDs and vertex indices
    for (o, m) in original.edges.iter().zip(moved.edges.iter()) {
        assert_eq!(o.id, m.id, "edge ID must be preserved");
        assert_eq!(
            o.vertices, m.vertices,
            "edge vertex indices must be preserved"
        );
        assert_eq!(o.t_range, m.t_range, "edge t_range must be preserved");
    }
    // HalfEdge IDs, start_vertex, edge, forward
    for (o, m) in original.half_edges.iter().zip(moved.half_edges.iter()) {
        assert_eq!(o.id, m.id, "half_edge ID must be preserved");
        assert_eq!(o.start_vertex, m.start_vertex);
        assert_eq!(o.edge, m.edge);
        assert_eq!(o.forward, m.forward);
    }
    // Loop IDs and half_edge indices
    for (o, m) in original.loops.iter().zip(moved.loops.iter()) {
        assert_eq!(o.id, m.id, "loop ID must be preserved");
        assert_eq!(o.half_edges, m.half_edges);
    }
    // Face IDs and loop indices
    for (o, m) in original.faces.iter().zip(moved.faces.iter()) {
        assert_eq!(o.id, m.id, "face ID must be preserved");
        assert_eq!(o.outer_loop, m.outer_loop);
        assert_eq!(o.inner_loops, m.inner_loops);
        assert_eq!(o.same_sense, m.same_sense);
    }
    // Shell IDs and face indices
    for (o, m) in original.shells.iter().zip(moved.shells.iter()) {
        assert_eq!(o.id, m.id, "shell ID must be preserved");
        assert_eq!(o.faces, m.faces);
        assert_eq!(o.closed, m.closed);
    }
}

#[test]
fn t05_translate_inverse_roundtrip() {
    use approx::assert_relative_eq;

    let original = make_test_cuboid();
    let offset = Vec3::new(5.0, -3.0, 7.0);
    let mut roundtrip = original.clone();
    roundtrip.translate(offset);
    roundtrip.translate(-offset);

    for (o, r) in original.vertices.iter().zip(roundtrip.vertices.iter()) {
        assert_relative_eq!(o.point.x, r.point.x, epsilon = 1e-12);
        assert_relative_eq!(o.point.y, r.point.y, epsilon = 1e-12);
        assert_relative_eq!(o.point.z, r.point.z, epsilon = 1e-12);
    }
    for (o, r) in original.edges.iter().zip(roundtrip.edges.iter()) {
        match (&o.curve, &r.curve) {
            (Curve::Line { origin: oo, .. }, Curve::Line { origin: ro, .. }) => {
                assert_relative_eq!(oo.x, ro.x, epsilon = 1e-12);
                assert_relative_eq!(oo.y, ro.y, epsilon = 1e-12);
                assert_relative_eq!(oo.z, ro.z, epsilon = 1e-12);
            }
            (
                Curve::Circle {
                    center: oc,
                    radius: or_,
                    ..
                },
                Curve::Circle {
                    center: rc,
                    radius: rr,
                    ..
                },
            ) => {
                assert_relative_eq!(oc.x, rc.x, epsilon = 1e-12);
                assert_relative_eq!(oc.y, rc.y, epsilon = 1e-12);
                assert_relative_eq!(oc.z, rc.z, epsilon = 1e-12);
                assert_relative_eq!(or_, rr, epsilon = 1e-12);
            }
            _ => panic!("curve variant mismatch after roundtrip"),
        }
    }
}

#[test]
fn t06_boundary_zero_offset_no_change() {
    let original = make_test_cuboid();
    let mut moved = original.clone();
    moved.translate(Vec3::new(0.0, 0.0, 0.0));

    // Strict equality — zero offset must produce identical geometry
    for (o, m) in original.vertices.iter().zip(moved.vertices.iter()) {
        assert_eq!(
            o.point, m.point,
            "vertex point must be identical with zero offset"
        );
    }
    for (o, m) in original.edges.iter().zip(moved.edges.iter()) {
        assert_eq!(
            o.curve, m.curve,
            "edge curve must be identical with zero offset"
        );
    }
    for (o, m) in original.faces.iter().zip(moved.faces.iter()) {
        assert_eq!(
            o.surface, m.surface,
            "face surface must be identical with zero offset"
        );
    }
}

#[test]
fn t07_degen_large_offset_stays_finite() {
    let mut cuboid = make_test_cuboid();
    cuboid.translate(Vec3::new(1e9, -1e9, 1e9));

    for v in &cuboid.vertices {
        assert!(v.point.x.is_finite(), "vertex x must be finite");
        assert!(v.point.y.is_finite(), "vertex y must be finite");
        assert!(v.point.z.is_finite(), "vertex z must be finite");
    }
    for e in &cuboid.edges {
        match &e.curve {
            Curve::Line { origin, .. } => {
                assert!(origin.x.is_finite());
                assert!(origin.y.is_finite());
                assert!(origin.z.is_finite());
            }
            Curve::Circle { center, .. } => {
                assert!(center.x.is_finite());
                assert!(center.y.is_finite());
                assert!(center.z.is_finite());
            }
        }
    }
}
