use crate::brep::topology::{IdGenerator, Solid};
use crate::error::KernelError;
use crate::geometry::curve::Curve;
use crate::geometry::surface::Surface;
use crate::geometry::{Point, Vec3};
use mycad_format::{EntityKind, EntityRef};

/// Create a sphere B-rep solid.
///
/// Center at `center`, given radius. Seam on the +X meridian (XZ half-plane).
/// Topology: 2 vertices (poles), 1 edge (seam half-circle), 2 half-edges,
/// 1 loop, 1 face (self-adjacent periodic), 1 closed shell.
pub fn make_sphere(
    radius: f64,
    center: Point,
    id_gen: &mut IdGenerator,
) -> Result<Solid, KernelError> {
    if !radius.is_finite() || radius <= 0.0 {
        return Err(KernelError::InvalidParameter { kind: "radius" });
    }
    if ![center.x, center.y, center.z].iter().all(|v| v.is_finite()) {
        return Err(KernelError::InvalidParameter { kind: "center" });
    }

    let fid = "sphere";
    let mut solid = Solid::new(id_gen.next());

    let origin = center;

    let v_south = solid.add_vertex(
        id_gen.next(),
        origin + Vec3::new(0.0, 0.0, -radius),
        EntityRef::try_named(fid, EntityKind::Vertex, "south_pole").ok(),
    );
    let v_north = solid.add_vertex(
        id_gen.next(),
        origin + Vec3::new(0.0, 0.0, radius),
        EntityRef::try_named(fid, EntityKind::Vertex, "north_pole").ok(),
    );

    let e_seam = solid.add_edge(
        id_gen.next(),
        [v_south, v_north],
        Curve::Circle {
            center: origin,
            normal: -Vec3::y(),
            radius,
        },
        [std::f64::consts::PI, 2.0 * std::f64::consts::PI],
        EntityRef::try_named(fid, EntityKind::Edge, "seam").ok(),
    );

    let he_up = solid.add_half_edge(id_gen.next(), v_south, e_seam, true);
    let he_down = solid.add_half_edge(id_gen.next(), v_north, e_seam, false);

    let lp = solid.add_loop(id_gen.next(), vec![he_up, he_down]);

    let f = solid.add_face(
        id_gen.next(),
        Surface::Sphere {
            center: origin,
            radius,
        },
        lp,
        vec![],
        true,
        EntityRef::try_named(fid, EntityKind::Face, "surface").ok(),
    );

    solid.add_shell(id_gen.next(), vec![f], true);

    Ok(solid)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::geometry::surface::TessellationStrategy;
    use std::f64::consts::PI;

    #[test]
    fn test_sphere_deterministic() {
        let mut gen1 = IdGenerator::new(0);
        let mut gen2 = IdGenerator::new(0);
        let s1 = make_sphere(5.0, Point::origin(), &mut gen1).unwrap();
        let s2 = make_sphere(5.0, Point::origin(), &mut gen2).unwrap();

        assert_eq!(s1.vertices.len(), s2.vertices.len());
        assert_eq!(s1.edges.len(), s2.edges.len());
        assert_eq!(s1.faces.len(), s2.faces.len());
        assert_eq!(s1.half_edges.len(), s2.half_edges.len());
        assert_eq!(s1.loops.len(), s2.loops.len());
        assert_eq!(s1.shells.len(), s2.shells.len());

        for (v1, v2) in s1.vertices.iter().zip(s2.vertices.iter()) {
            assert_eq!(v1.id, v2.id);
            assert_eq!(v1.point, v2.point);
        }
        for (e1, e2) in s1.edges.iter().zip(s2.edges.iter()) {
            assert_eq!(e1.id, e2.id);
            assert_eq!(e1.vertices, e2.vertices);
            assert_eq!(e1.t_range, e2.t_range);
            match (&e1.curve, &e2.curve) {
                (
                    Curve::Circle {
                        center: c1,
                        normal: n1,
                        radius: r1,
                    },
                    Curve::Circle {
                        center: c2,
                        normal: n2,
                        radius: r2,
                    },
                ) => {
                    assert_eq!(c1, c2);
                    assert_eq!(n1, n2);
                    assert_eq!(r1, r2);
                }
                _ => panic!("curve variant mismatch"),
            }
        }
        for (he1, he2) in s1.half_edges.iter().zip(s2.half_edges.iter()) {
            assert_eq!(he1.id, he2.id);
            assert_eq!(he1.edge, he2.edge);
            assert_eq!(he1.forward, he2.forward);
            assert_eq!(he1.start_vertex, he2.start_vertex);
        }
        for (l1, l2) in s1.loops.iter().zip(s2.loops.iter()) {
            assert_eq!(l1.id, l2.id);
            assert_eq!(l1.half_edges, l2.half_edges);
        }
        for (f1, f2) in s1.faces.iter().zip(s2.faces.iter()) {
            assert_eq!(f1.id, f2.id);
            assert_eq!(f1.outer_loop, f2.outer_loop);
            assert_eq!(f1.inner_loops, f2.inner_loops);
            assert_eq!(f1.same_sense, f2.same_sense);
            match (&f1.surface, &f2.surface) {
                (
                    Surface::Sphere {
                        center: c1,
                        radius: r1,
                    },
                    Surface::Sphere {
                        center: c2,
                        radius: r2,
                    },
                ) => {
                    assert_eq!(c1, c2);
                    assert_eq!(r1, r2);
                }
                _ => panic!("surface variant mismatch"),
            }
        }
        for (sh1, sh2) in s1.shells.iter().zip(s2.shells.iter()) {
            assert_eq!(sh1.id, sh2.id);
            assert_eq!(sh1.faces, sh2.faces);
            assert_eq!(sh1.closed, sh2.closed);
        }
    }

    #[test]
    fn test_sphere_topology() {
        let mut gen = IdGenerator::new(0);
        let s = make_sphere(5.0, Point::origin(), &mut gen).unwrap();

        assert_eq!(s.vertices.len(), 2, "2 poles");
        assert_eq!(s.edges.len(), 1, "1 seam edge");
        assert_eq!(s.faces.len(), 1, "1 self-adjacent face");
        assert_eq!(s.shells.len(), 1);
        assert!(s.shells[0].closed);
        assert_eq!(s.half_edges.len(), 2, "2 half-edges on seam");
        assert_eq!(s.loops.len(), 1, "1 loop");
    }

    #[test]
    fn test_sphere_euler() {
        let mut gen = IdGenerator::new(0);
        let s = make_sphere(5.0, Point::origin(), &mut gen).unwrap();

        let v = s.vertices.len() as i64;
        let e = s.edges.len() as i64;
        let f = s.faces.len() as i64;
        let shells = s.shells.len() as i64;
        assert_eq!(v - e + f, 2 * shells, "Euler-Poincaré");
    }

    #[test]
    fn test_sphere_manifold_and_loop_closure() {
        let mut gen = IdGenerator::new(0);
        let s = make_sphere(5.0, Point::origin(), &mut gen).unwrap();
        let eps = 1e-10;

        let mut edge_he_count: std::collections::HashMap<usize, Vec<bool>> =
            std::collections::HashMap::new();
        for he in &s.half_edges {
            edge_he_count.entry(he.edge).or_default().push(he.forward);
        }
        for (edge_idx, forwards) in &edge_he_count {
            assert_eq!(forwards.len(), 2, "edge {edge_idx} must have 2 HEs");
            assert_ne!(
                forwards[0], forwards[1],
                "edge {edge_idx} HEs must be opposite"
            );
        }

        for (loop_idx, lp) in s.loops.iter().enumerate() {
            assert!(
                !lp.half_edges.is_empty(),
                "loop {loop_idx} must not be empty"
            );
            for i in 0..lp.half_edges.len() {
                let he_cur = &s.half_edges[lp.half_edges[i]];
                let he_next = &s.half_edges[lp.half_edges[(i + 1) % lp.half_edges.len()]];

                let cur_edge = &s.edges[he_cur.edge];
                let end_v = if he_cur.forward {
                    cur_edge.vertices[1]
                } else {
                    cur_edge.vertices[0]
                };

                let next_start = he_next.start_vertex;

                assert!(
                    (s.vertices[end_v].point - s.vertices[next_start].point).norm() < eps,
                    "loop {loop_idx}: HE {} end ({:?}) != HE {} start ({:?})",
                    i,
                    s.vertices[end_v].point,
                    (i + 1) % lp.half_edges.len(),
                    s.vertices[next_start].point,
                );
            }
        }
    }

    #[test]
    fn test_sphere_geometry() {
        let mut gen = IdGenerator::new(0);
        let s = make_sphere(5.0, Point::origin(), &mut gen).unwrap();
        let eps = 1e-10;

        assert!((s.vertices[0].point - Point::new(0.0, 0.0, -5.0)).norm() < eps);
        assert!((s.vertices[1].point - Point::new(0.0, 0.0, 5.0)).norm() < eps);

        match &s.faces[0].surface {
            Surface::Sphere { center, radius } => {
                assert!((center - &Point::origin()).norm() < eps);
                assert!((radius - 5.0).abs() < eps);
            }
            _ => panic!("face should be Sphere surface"),
        }

        let edge = &s.edges[0];
        match &edge.curve {
            Curve::Circle {
                center,
                normal,
                radius,
            } => {
                assert!((center - &Point::origin()).norm() < eps);
                assert!(
                    (*normal + Vec3::y()).norm() < eps,
                    "seam normal should be -Y"
                );
                assert!((radius - 5.0).abs() < eps);
                assert!((edge.t_range[0] - PI).abs() < eps);
                assert!((edge.t_range[1] - 2.0 * PI).abs() < eps);
            }
            _ => panic!("seam should be Circle curve"),
        }
    }

    #[test]
    fn test_sphere_degenerate_inputs() {
        let mut gen = IdGenerator::new(0);

        let err = make_sphere(0.0, Point::origin(), &mut gen).unwrap_err();
        assert!(matches!(
            err,
            KernelError::InvalidParameter { kind: "radius" }
        ));

        let err = make_sphere(-5.0, Point::origin(), &mut gen).unwrap_err();
        assert!(matches!(
            err,
            KernelError::InvalidParameter { kind: "radius" }
        ));

        let err = make_sphere(f64::NAN, Point::origin(), &mut gen).unwrap_err();
        assert!(matches!(
            err,
            KernelError::InvalidParameter { kind: "radius" }
        ));

        let err = make_sphere(f64::INFINITY, Point::origin(), &mut gen).unwrap_err();
        assert!(matches!(
            err,
            KernelError::InvalidParameter { kind: "radius" }
        ));

        let err = make_sphere(f64::NEG_INFINITY, Point::origin(), &mut gen).unwrap_err();
        assert!(matches!(
            err,
            KernelError::InvalidParameter { kind: "radius" }
        ));

        let err = make_sphere(f64::MIN_POSITIVE, Point::origin(), &mut gen);
        assert!(err.is_ok(), "tiny but positive radius should succeed");
    }

    #[test]
    fn test_sphere_surface_tessellation_strategy() {
        let s = Surface::Sphere {
            center: Point::origin(),
            radius: 5.0,
        };
        assert_eq!(s.tessellation_strategy(), TessellationStrategy::UvSphere);
    }

    #[test]
    fn test_sphere_self_adjacent_validate_manifold() {
        let mut gen = IdGenerator::new(0);
        let s = make_sphere(5.0, Point::origin(), &mut gen).unwrap();
        assert!(
            s.validate_manifold().is_ok(),
            "full sphere must pass manifold validation"
        );
    }

    #[test]
    fn test_sphere_100_run_determinism() {
        let first = {
            let mut gen = IdGenerator::new(0);
            make_sphere(3.0, Point::origin(), &mut gen).unwrap()
        };
        for i in 1..100 {
            let mut gen = IdGenerator::new(0);
            let s = make_sphere(3.0, Point::origin(), &mut gen).unwrap();
            for (v1, v2) in first.vertices.iter().zip(s.vertices.iter()) {
                assert_eq!(v1.id, v2.id, "run {i}: vertex id mismatch");
                assert_eq!(v1.point, v2.point, "run {i}: vertex point mismatch");
            }
            for (e1, e2) in first.edges.iter().zip(s.edges.iter()) {
                assert_eq!(e1.id, e2.id, "run {i}: edge id mismatch");
                assert_eq!(e1.vertices, e2.vertices, "run {i}: edge vertices mismatch");
            }
            assert_eq!(first.faces.len(), s.faces.len(), "run {i}: face count");
        }
    }

    #[test]
    fn test_sphere_roundtrip_serde() {
        let mut gen = IdGenerator::new(0);
        let original = make_sphere(7.0, Point::origin(), &mut gen).unwrap();
        let json = serde_json::to_string(&original).unwrap();
        let restored: Solid = serde_json::from_str(&json).unwrap();

        assert_eq!(original.vertices.len(), restored.vertices.len());
        assert_eq!(original.edges.len(), restored.edges.len());
        assert_eq!(original.faces.len(), restored.faces.len());
        assert_eq!(original.half_edges.len(), restored.half_edges.len());
        assert_eq!(original.loops.len(), restored.loops.len());
        assert_eq!(original.shells.len(), restored.shells.len());

        for (v1, v2) in original.vertices.iter().zip(restored.vertices.iter()) {
            assert_eq!(v1.id, v2.id);
            assert_eq!(v1.point, v2.point);
        }
    }

    #[test]
    fn test_sphere_tiny_radius() {
        let mut gen = IdGenerator::new(0);
        let s = make_sphere(1e-10, Point::origin(), &mut gen).unwrap();
        assert_eq!(s.vertices.len(), 2);
    }

    #[test]
    fn test_sphere_large_radius() {
        let mut gen = IdGenerator::new(0);
        let s = make_sphere(1e10, Point::origin(), &mut gen).unwrap();
        assert_eq!(s.vertices.len(), 2);
    }

    #[test]
    fn test_sphere_negative_zero_radius() {
        let mut gen = IdGenerator::new(0);
        let err = make_sphere(-0.0, Point::origin(), &mut gen).unwrap_err();
        assert!(matches!(
            err,
            KernelError::InvalidParameter { kind: "radius" }
        ));
    }

    #[test]
    fn test_sphere_entity_names() {
        let mut gen = IdGenerator::new(0);
        let s = make_sphere(5.0, Point::origin(), &mut gen).unwrap();

        let v_south_name = s.vertices[0].name.as_ref().expect("south pole name");
        match v_south_name {
            EntityRef::Named {
                feature_id,
                kind,
                role,
            } => {
                assert_eq!(feature_id, "sphere");
                assert_eq!(*kind, EntityKind::Vertex);
                assert_eq!(role, "south_pole");
            }
            _ => panic!("expected Named, got Derived"),
        }

        let v_north_name = s.vertices[1].name.as_ref().expect("north pole name");
        match v_north_name {
            EntityRef::Named {
                feature_id,
                kind,
                role,
            } => {
                assert_eq!(feature_id, "sphere");
                assert_eq!(*kind, EntityKind::Vertex);
                assert_eq!(role, "north_pole");
            }
            _ => panic!("expected Named, got Derived"),
        }

        let e_name = s.edges[0].name.as_ref().expect("seam edge name");
        match e_name {
            EntityRef::Named {
                feature_id,
                kind,
                role,
            } => {
                assert_eq!(feature_id, "sphere");
                assert_eq!(*kind, EntityKind::Edge);
                assert_eq!(role, "seam");
            }
            _ => panic!("expected Named, got Derived"),
        }

        let f_name = s.faces[0].name.as_ref().expect("face name");
        match f_name {
            EntityRef::Named {
                feature_id,
                kind,
                role,
            } => {
                assert_eq!(feature_id, "sphere");
                assert_eq!(*kind, EntityKind::Face);
                assert_eq!(role, "surface");
            }
            _ => panic!("expected Named, got Derived"),
        }
    }

    #[test]
    fn test_sphere_name_determinism() {
        let mut gen1 = IdGenerator::new(0);
        let mut gen2 = IdGenerator::new(0);
        let s1 = make_sphere(5.0, Point::origin(), &mut gen1).unwrap();
        let s2 = make_sphere(5.0, Point::origin(), &mut gen2).unwrap();

        for (i, (v1, v2)) in s1.vertices.iter().zip(s2.vertices.iter()).enumerate() {
            assert_eq!(v1.name, v2.name, "vertex {i} name mismatch");
        }
        for (i, (e1, e2)) in s1.edges.iter().zip(s2.edges.iter()).enumerate() {
            assert_eq!(e1.name, e2.name, "edge {i} name mismatch");
        }
        for (i, (f1, f2)) in s1.faces.iter().zip(s2.faces.iter()).enumerate() {
            assert_eq!(f1.name, f2.name, "face {i} name mismatch");
        }
    }

    #[test]
    fn test_validate_manifold_detects_broken_loop() {
        let mut s = Solid::new(0);
        let v0 = s.add_vertex(1, Point::new(0.0, 0.0, -5.0), None);
        let v1 = s.add_vertex(2, Point::new(0.0, 0.0, 5.0), None);
        let e0 = s.add_edge(
            3,
            [v0, v1],
            Curve::Circle {
                center: Point::origin(),
                normal: -Vec3::y(),
                radius: 5.0,
            },
            [PI, 2.0 * PI],
            None,
        );
        let he0 = s.add_half_edge(4, v0, e0, true);
        let he1 = s.add_half_edge(5, v0, e0, false);
        let lp = s.add_loop(6, vec![he0, he1]);
        s.add_face(
            7,
            Surface::Sphere {
                center: Point::origin(),
                radius: 5.0,
            },
            lp,
            vec![],
            true,
            None,
        );
        s.add_shell(8, vec![0], true);
        assert!(
            s.validate_manifold().is_err(),
            "broken loop should fail validation"
        );
    }
}
