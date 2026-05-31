pub use self::types::BooleanOp;

mod assemble;
mod classify;
mod partition;
mod types;

use crate::brep::topology::{IdGenerator, Solid};
use crate::error::KernelError;
use crate::geometry::math::LENGTH_TOLERANCE;
use crate::geometry::surface::Surface;

/// Perform a boolean operation between two solids.
pub fn boolean(
    target: &Solid,
    tool: &Solid,
    op: BooleanOp,
    id_gen: &mut IdGenerator,
) -> Result<Solid, KernelError> {
    // Validate inputs in fixed order (Codex R03)
    validate_boolean_input(target, "target")?;
    validate_boolean_input(tool, "tool")?;

    // Phase A: Partition
    let (target_fragments, tool_fragments) =
        partition::partition_faces(target, tool, op).map_err(KernelError::BooleanInternal)?;

    if target_fragments.is_empty() && tool_fragments.is_empty() {
        return Err(KernelError::EmptyBooleanResult);
    }

    // Phase B: Classify
    let classified =
        classify::classify_fragments(&target_fragments, &tool_fragments, target, tool, op, &[])
            .map_err(KernelError::BooleanInternal)?;

    // Phase C: Assemble
    let result = assemble::assemble(&classified, op, id_gen)?;

    Ok(result)
}

fn validate_boolean_input(solid: &Solid, _label: &str) -> Result<(), KernelError> {
    // 1. validate_manifold
    solid
        .validate_manifold()
        .map_err(|_| KernelError::OpenBooleanInput)?;

    // 2. All faces must be Plane, Cylinder, or Sphere (Cone unsupported)
    for face in &solid.faces {
        match &face.surface {
            Surface::Plane { .. } | Surface::Cylinder { .. } | Surface::Sphere { .. } => {}
            Surface::Cone { .. } => {
                return Err(KernelError::NonPlanarBooleanInput {
                    kind: "cone surface",
                });
            }
        }
    }

    // 3. Inner loops: accept up to 1 level of nesting, reject empty or multi-nested
    for face in &solid.faces {
        for inner_idx in &face.inner_loops {
            let lp = &solid.loops[*inner_idx];
            if lp.half_edges.is_empty() {
                return Err(KernelError::UnsupportedBooleanInput {
                    reason: "inner loop has no half-edges",
                });
            }
        }
    }

    // 4. Planar faces: validate outer loop geometry (no degeneracy).
    //    Curved faces: no polygon check (surface UV handles shape).
    for face in &solid.faces {
        if matches!(face.surface, Surface::Plane { .. }) {
            validate_planar_face_outer_loop_basic(face, solid)?;
        }
    }

    // 5. All faces/edges/vertices must have names
    for face in &solid.faces {
        if face.name.is_none() {
            return Err(KernelError::MissingEntityName);
        }
    }
    for edge in &solid.edges {
        if edge.name.is_none() {
            return Err(KernelError::MissingEntityName);
        }
    }
    for vertex in &solid.vertices {
        if vertex.name.is_none() {
            return Err(KernelError::MissingEntityName);
        }
    }

    Ok(())
}

fn validate_planar_face_outer_loop_basic(
    face: &crate::brep::topology::Face,
    solid: &Solid,
) -> Result<(), KernelError> {
    let lp = &solid.loops[face.outer_loop];
    let hes = &lp.half_edges;

    // Analytic Circle outer loop: single half-edge on a Circle curve
    if hes.len() == 1 {
        let he = &solid.half_edges[hes[0]];
        let edge = &solid.edges[he.edge];
        if let crate::geometry::curve::Curve::Circle { radius, .. } = &edge.curve {
            if *radius <= LENGTH_TOLERANCE {
                return Err(KernelError::UnsupportedBooleanInput {
                    reason: "analytic circle radius below LENGTH_TOLERANCE",
                });
            }
            return Ok(());
        }
    }

    let points: Vec<crate::geometry::Point> = hes
        .iter()
        .map(|&he_idx| solid.vertices[solid.half_edges[he_idx].start_vertex].point)
        .collect();

    let n = points.len();
    if n < 3 {
        return Err(KernelError::UnsupportedBooleanInput {
            reason: "planar face outer loop has fewer than 3 vertices",
        });
    }

    // Zero-length edge check
    for i in 0..n {
        let j = (i + 1) % n;
        let dist = (points[j] - points[i]).norm();
        if dist <= LENGTH_TOLERANCE {
            return Err(KernelError::UnsupportedBooleanInput {
                reason: "planar face outer loop has zero-length edge",
            });
        }
    }

    // Degenerate area check: compute signed area via 2D projection
    let area_eps = LENGTH_TOLERANCE * LENGTH_TOLERANCE;
    let mut normal = crate::geometry::Vec3::new(0.0, 0.0, 0.0);
    for i in 0..n {
        let j = (i + 1) % n;
        normal.x += (points[i].y - points[j].y) * (points[i].z + points[j].z);
        normal.y += (points[i].z - points[j].z) * (points[i].x + points[j].x);
        normal.z += (points[i].x - points[j].x) * (points[i].y + points[j].y);
    }
    let (u_idx, v_idx) = if normal.x.abs() >= normal.y.abs() && normal.x.abs() >= normal.z.abs() {
        (1, 2)
    } else if normal.y.abs() >= normal.z.abs() {
        (0, 2)
    } else {
        (0, 1)
    };

    let mut area = 0.0_f64;
    for i in 0..n {
        let j = (i + 1) % n;
        area += (points[i].coords[u_idx] - points[j].coords[u_idx])
            * (points[i].coords[v_idx] + points[j].coords[v_idx]);
    }
    if area.abs() <= area_eps {
        return Err(KernelError::UnsupportedBooleanInput {
            reason: "planar face outer loop has zero area",
        });
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::brep::topology::{IdGenerator, Solid};
    use crate::geometry::curve::Curve;
    use crate::geometry::surface::Surface;
    use crate::geometry::{Point, Vec3};
    use crate::primitives::{make_cuboid, make_sphere};

    fn make_planar_solid_with_points(points: Vec<Point>, id_gen: &mut IdGenerator) -> Solid {
        let mut solid = Solid::new(id_gen.next());

        let normal = Vec3::z();
        let u_axis = Vec3::x();
        let v_axis = Vec3::y();
        let origin = Point::origin();

        let v_indices: Vec<usize> = points
            .into_iter()
            .map(|p| {
                let name = mycad_format::EntityRef::try_named(
                    "test",
                    mycad_format::EntityKind::Vertex,
                    "v",
                )
                .ok();
                solid.add_vertex(id_gen.next(), p, name)
            })
            .collect();

        let mut he_indices = Vec::new();
        for i in 0..v_indices.len() {
            let j = (i + 1) % v_indices.len();
            let p0 = solid.vertices[v_indices[i]].point;
            let p1 = solid.vertices[v_indices[j]].point;
            let e_idx = solid.add_edge(
                id_gen.next(),
                [v_indices[i], v_indices[j]],
                Curve::Line {
                    origin: p0,
                    direction: p1 - p0,
                },
                [0.0, 1.0],
                mycad_format::EntityRef::try_named("test", mycad_format::EntityKind::Edge, "e")
                    .ok(),
            );
            let he_idx = solid.add_half_edge(id_gen.next(), v_indices[i], e_idx, true);
            he_indices.push(he_idx);
        }

        let lp = solid.add_loop(id_gen.next(), he_indices);
        let name =
            mycad_format::EntityRef::try_named("test", mycad_format::EntityKind::Face, "f").ok();
        let fi = solid.add_face(
            id_gen.next(),
            Surface::Plane {
                origin,
                normal,
                u_axis,
                v_axis,
            },
            lp,
            vec![],
            true,
            name,
        );
        solid.add_shell(id_gen.next(), vec![fi], true);
        solid
    }

    #[test]
    fn t07_non_convex_polygon_accepted() {
        let mut gen = IdGenerator::new(0);
        let points = vec![
            Point::new(0.0, 0.0, 0.0),
            Point::new(2.0, 0.0, 0.0),
            Point::new(1.0, 0.5, 0.0),
            Point::new(2.0, 2.0, 0.0),
            Point::new(0.0, 2.0, 0.0),
        ];
        let solid = make_planar_solid_with_points(points, &mut gen);
        let face = &solid.faces[0];
        assert!(
            validate_planar_face_outer_loop_basic(face, &solid).is_ok(),
            "non-convex polygon should be accepted"
        );
    }

    #[test]
    fn t08_two_vertex_polygon_rejected() {
        let mut gen = IdGenerator::new(0);
        let points = vec![Point::new(0.0, 0.0, 0.0), Point::new(1.0, 0.0, 0.0)];
        let solid = make_planar_solid_with_points(points, &mut gen);
        let face = &solid.faces[0];
        let result = validate_planar_face_outer_loop_basic(face, &solid);
        assert!(result.is_err());
        let err = format!("{}", result.unwrap_err());
        assert!(err.contains("fewer than 3 vertices"), "got: {err}");
    }

    #[test]
    fn t13_analytic_circle_loop_accepted() {
        let mut gen = IdGenerator::new(0);
        let cyl = crate::primitives::make_cylinder(2.0, 4.0, &mut gen).unwrap();
        // Find a planar face (cap) with Circle outer loop
        for face in &cyl.faces {
            if matches!(face.surface, Surface::Plane { .. }) {
                assert!(
                    validate_planar_face_outer_loop_basic(face, &cyl).is_ok(),
                    "cylinder cap with analytic Circle loop should pass"
                );
                return;
            }
        }
        panic!("cylinder should have planar faces");
    }

    #[test]
    fn t16a_collinear_polygon_rejected() {
        let mut gen = IdGenerator::new(0);
        let points = vec![
            Point::new(0.0, 0.0, 0.0),
            Point::new(1.0, 0.0, 0.0),
            Point::new(2.0, 0.0, 0.0),
        ];
        let solid = make_planar_solid_with_points(points, &mut gen);
        let face = &solid.faces[0];
        let result = validate_planar_face_outer_loop_basic(face, &solid);
        assert!(result.is_err());
        let err = format!("{}", result.unwrap_err());
        assert!(err.contains("zero area"), "got: {err}");
    }

    #[test]
    fn t16b_zero_area_polygon_rejected() {
        let mut gen = IdGenerator::new(0);
        let points = vec![
            Point::new(0.0, 0.0, 0.0),
            Point::new(1e-15, 0.0, 0.0),
            Point::new(0.5e-15, 1e-15, 0.0),
        ];
        let solid = make_planar_solid_with_points(points, &mut gen);
        let face = &solid.faces[0];
        let result = validate_planar_face_outer_loop_basic(face, &solid);
        assert!(result.is_err(), "zero area polygon should be rejected");
    }

    #[test]
    fn t16c_zero_length_edge_rejected() {
        let mut gen = IdGenerator::new(0);
        let points = vec![
            Point::new(0.0, 0.0, 0.0),
            Point::new(0.0, 0.0, 0.0),
            Point::new(1.0, 1.0, 0.0),
            Point::new(0.0, 1.0, 0.0),
        ];
        let solid = make_planar_solid_with_points(points, &mut gen);
        let face = &solid.faces[0];
        let result = validate_planar_face_outer_loop_basic(face, &solid);
        assert!(result.is_err());
        let err = format!("{}", result.unwrap_err());
        assert!(err.contains("zero-length edge"), "got: {err}");
    }

    #[test]
    fn t16d_tiny_analytic_circle_rejected() {
        let mut gen = IdGenerator::new(0);
        let mut solid = Solid::new(gen.next());
        let v0 = solid.add_vertex(
            gen.next(),
            Point::new(0.0, 0.0, 0.0),
            mycad_format::EntityRef::try_named("test", mycad_format::EntityKind::Vertex, "v").ok(),
        );
        let v1 = solid.add_vertex(
            gen.next(),
            Point::new(0.0, 0.0, 0.0),
            mycad_format::EntityRef::try_named("test", mycad_format::EntityKind::Vertex, "v2").ok(),
        );
        let e = solid.add_edge(
            gen.next(),
            [v0, v1],
            Curve::Circle {
                center: Point::origin(),
                normal: Vec3::z(),
                radius: LENGTH_TOLERANCE / 2.0,
            },
            [0.0, std::f64::consts::TAU],
            mycad_format::EntityRef::try_named("test", mycad_format::EntityKind::Edge, "e").ok(),
        );
        let he = solid.add_half_edge(gen.next(), v0, e, true);
        let lp = solid.add_loop(gen.next(), vec![he]);
        let fi = solid.add_face(
            gen.next(),
            Surface::Plane {
                origin: Point::origin(),
                normal: Vec3::z(),
                u_axis: Vec3::x(),
                v_axis: Vec3::y(),
            },
            lp,
            vec![],
            true,
            mycad_format::EntityRef::try_named("test", mycad_format::EntityKind::Face, "f").ok(),
        );
        solid.add_shell(gen.next(), vec![fi], true);
        let face = &solid.faces[fi];
        let result = validate_planar_face_outer_loop_basic(face, &solid);
        assert!(result.is_err());
        let err = format!("{}", result.unwrap_err());
        assert!(
            err.contains("analytic circle radius below LENGTH_TOLERANCE"),
            "got: {err}"
        );
    }

    #[test]
    fn t_validate_cuboid_passes() {
        let mut gen = IdGenerator::new(0);
        let cuboid = make_cuboid(2.0, 2.0, 2.0, &mut gen).unwrap();
        assert!(validate_boolean_input(&cuboid, "test").is_ok());
    }

    #[test]
    fn t_validate_sphere_passes() {
        let mut gen = IdGenerator::new(0);
        let sphere = make_sphere(5.0, &mut gen).unwrap();
        assert!(validate_boolean_input(&sphere, "test").is_ok());
    }

    /// TX1 — validate_boolean_input accepts a mixed solid with sphere + cuboid faces
    #[test]
    fn tx1_validate_mixed_sphere_cuboid_solid() {
        let mut gen = IdGenerator::new(0);
        let cuboid = make_cuboid(2.0, 2.0, 2.0, &mut gen).unwrap();
        let sphere = make_sphere(3.0, &mut gen).unwrap();

        let mut mixed = cuboid;

        // Merge sphere topology into cuboid as second shell
        let mut v_map = std::collections::HashMap::new();
        for (i, v) in sphere.vertices.iter().enumerate() {
            let new_idx = mixed.add_vertex(gen.next(), v.point, v.name.clone());
            v_map.insert(i, new_idx);
        }

        let mut e_map = std::collections::HashMap::new();
        for (i, e) in sphere.edges.iter().enumerate() {
            let new_verts = [v_map[&e.vertices[0]], v_map[&e.vertices[1]]];
            let new_idx = mixed.add_edge(
                gen.next(),
                new_verts,
                e.curve.clone(),
                e.t_range,
                e.name.clone(),
            );
            e_map.insert(i, new_idx);
        }

        let mut he_map = std::collections::HashMap::new();
        for (i, he) in sphere.half_edges.iter().enumerate() {
            let new_v = v_map[&he.start_vertex];
            let new_e = e_map[&he.edge];
            let new_idx = mixed.add_half_edge(gen.next(), new_v, new_e, he.forward);
            he_map.insert(i, new_idx);
        }

        let mut l_map = std::collections::HashMap::new();
        for (i, lp) in sphere.loops.iter().enumerate() {
            let new_hes: Vec<usize> = lp.half_edges.iter().map(|&h| he_map[&h]).collect();
            let new_idx = mixed.add_loop(gen.next(), new_hes);
            l_map.insert(i, new_idx);
        }

        let mut f_map = std::collections::HashMap::new();
        for (i, f) in sphere.faces.iter().enumerate() {
            let outer = l_map[&f.outer_loop];
            let inner: Vec<usize> = f.inner_loops.iter().map(|&l| l_map[&l]).collect();
            let new_idx = mixed.add_face(
                gen.next(),
                f.surface.clone(),
                outer,
                inner,
                f.same_sense,
                f.name.clone(),
            );
            f_map.insert(i, new_idx);
        }

        let shell_faces: Vec<usize> = sphere.shells[0].faces.iter().map(|&f| f_map[&f]).collect();
        mixed.add_shell(gen.next(), shell_faces, true);

        assert_eq!(mixed.faces.len(), 7, "6 cuboid + 1 sphere face");
        let has_plane = mixed
            .faces
            .iter()
            .any(|f| matches!(f.surface, Surface::Plane { .. }));
        let has_sphere = mixed
            .faces
            .iter()
            .any(|f| matches!(f.surface, Surface::Sphere { .. }));
        assert!(has_plane, "should have planar faces");
        assert!(has_sphere, "should have sphere face");

        let result = validate_boolean_input(&mixed, "mixed");
        assert!(
            result.is_ok(),
            "mixed sphere+cuboid solid should pass: {:?}",
            result
        );
    }

    /// TX4 — CW polygon (negative area) passes validate_planar_face_outer_loop_basic
    #[test]
    fn tx4_cw_polygon_accepted() {
        let mut gen = IdGenerator::new(0);
        // Clockwise triangle in XY plane (viewed from +Z)
        let points = vec![
            Point::new(0.0, 0.0, 0.0),
            Point::new(0.0, 1.0, 0.0),
            Point::new(1.0, 0.0, 0.0),
        ];
        let solid = make_planar_solid_with_points(points, &mut gen);
        let face = &solid.faces[0];
        assert!(
            validate_planar_face_outer_loop_basic(face, &solid).is_ok(),
            "CW polygon should be accepted (area.abs() check)"
        );
    }

    /// TX6 — radius exactly LENGTH_TOLERANCE is rejected (<= check)
    #[test]
    fn tx6_exact_tolerance_circle_rejected() {
        let mut gen = IdGenerator::new(0);
        let mut solid = Solid::new(gen.next());
        let v0 = solid.add_vertex(
            gen.next(),
            Point::new(0.0, 0.0, 0.0),
            mycad_format::EntityRef::try_named("test", mycad_format::EntityKind::Vertex, "v").ok(),
        );
        let v1 = solid.add_vertex(
            gen.next(),
            Point::new(0.0, 0.0, 0.0),
            mycad_format::EntityRef::try_named("test", mycad_format::EntityKind::Vertex, "v2").ok(),
        );
        let e = solid.add_edge(
            gen.next(),
            [v0, v1],
            Curve::Circle {
                center: Point::origin(),
                normal: Vec3::z(),
                radius: LENGTH_TOLERANCE,
            },
            [0.0, std::f64::consts::TAU],
            mycad_format::EntityRef::try_named("test", mycad_format::EntityKind::Edge, "e").ok(),
        );
        let he = solid.add_half_edge(gen.next(), v0, e, true);
        let lp = solid.add_loop(gen.next(), vec![he]);
        let fi = solid.add_face(
            gen.next(),
            Surface::Plane {
                origin: Point::origin(),
                normal: Vec3::z(),
                u_axis: Vec3::x(),
                v_axis: Vec3::y(),
            },
            lp,
            vec![],
            true,
            mycad_format::EntityRef::try_named("test", mycad_format::EntityKind::Face, "f").ok(),
        );
        solid.add_shell(gen.next(), vec![fi], true);
        let face = &solid.faces[fi];
        let result = validate_planar_face_outer_loop_basic(face, &solid);
        assert!(
            result.is_err(),
            "exact LENGTH_TOLERANCE radius should be rejected"
        );
        let err = format!("{}", result.unwrap_err());
        assert!(
            err.contains("analytic circle radius below LENGTH_TOLERANCE"),
            "got: {err}"
        );
    }
}
