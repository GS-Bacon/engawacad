use crate::brep::topology::{IdGenerator, Solid};
use crate::error::KernelError;
use crate::geometry::curve::Curve;
use crate::geometry::surface::Surface;
use crate::geometry::Point;
use crate::geometry::Vec3;
use engawa_format::{EntityKind, EntityRef};

/// Create a cuboid (box) B-rep solid centered at the origin.
///
/// Parameters:
/// - `dx`: width (X dimension)
/// - `dy`: height (Y dimension)
/// - `dz`: depth (Z dimension)
/// - `id_gen`: deterministic ID generator
///
/// The cuboid spans from (-dx/2, -dy/2, -dz/2) to (dx/2, dy/2, dz/2).
pub fn make_cuboid(
    dx: f64,
    dy: f64,
    dz: f64,
    id_gen: &mut IdGenerator,
) -> Result<Solid, KernelError> {
    if !dx.is_finite() || dx <= 0.0 {
        return Err(KernelError::InvalidParameter { kind: "width" });
    }
    if !dy.is_finite() || dy <= 0.0 {
        return Err(KernelError::InvalidParameter { kind: "height" });
    }
    if !dz.is_finite() || dz <= 0.0 {
        return Err(KernelError::InvalidParameter { kind: "depth" });
    }
    let mut solid = Solid::new(id_gen.next());

    let hx = dx / 2.0;
    let hy = dy / 2.0;
    let hz = dz / 2.0;

    // 8 vertices of the cuboid
    //
    //     6-------7
    //    /|      /|
    //   4-------5 |
    //   | 2-----|-3
    //   |/      |/
    //   0-------1
    //
    // v0 = (-hx, -hy, -hz), v1 = (hx, -hy, -hz), ...
    // Vertex roles: v_{xs}{ys}{zs} where xs/ys/zs ∈ {p, n}
    let v_data: [(Point, &str); 8] = [
        (Point::new(-hx, -hy, -hz), "v_nnn"), // 0
        (Point::new(hx, -hy, -hz), "v_pnn"),  // 1
        (Point::new(-hx, hy, -hz), "v_npn"),  // 2
        (Point::new(hx, hy, -hz), "v_ppn"),   // 3
        (Point::new(-hx, -hy, hz), "v_nnp"),  // 4
        (Point::new(hx, -hy, hz), "v_pnp"),   // 5
        (Point::new(-hx, hy, hz), "v_npp"),   // 6
        (Point::new(hx, hy, hz), "v_ppp"),    // 7
    ];

    // We'll use a fixed feature_id "cuboid" for the EntityRef names since the actual
    // feature_id is not available inside the kernel. The build dispatcher will need to
    // re-name these if needed. For now this gives named entities for boolean operations.
    let fid = "cuboid";

    let v: Vec<usize> = v_data
        .iter()
        .map(|(p, role)| {
            let name = EntityRef::try_named(fid, EntityKind::Vertex, *role).ok();
            solid.add_vertex(id_gen.next(), *p, name)
        })
        .collect();

    // Edge roles: connect two vertex roles with canonical sort
    let edge_defs: [(usize, usize, &str); 12] = [
        (0, 1, "e_v_nnn__v_pnn"), // e0
        (1, 3, "e_v_pnn__v_ppn"), // e1
        (3, 2, "e_v_npn__v_ppn"), // e2
        (2, 0, "e_v_nnn__v_npn"), // e3
        (4, 5, "e_v_nnp__v_pnp"), // e4
        (5, 7, "e_v_pnp__v_ppp"), // e5
        (7, 6, "e_v_npp__v_ppp"), // e6
        (6, 4, "e_v_nnp__v_npp"), // e7
        (0, 4, "e_v_nnn__v_nnp"), // e8
        (1, 5, "e_v_pnn__v_pnp"), // e9
        (3, 7, "e_v_ppn__v_ppp"), // e10
        (2, 6, "e_v_npn__v_npp"), // e11
    ];

    let e: Vec<usize> = edge_defs
        .iter()
        .map(|&(v0, v1, role)| {
            let p0 = solid.vertices[v[v0]].point;
            let p1 = solid.vertices[v[v1]].point;
            let direction = p1 - p0;
            let name = EntityRef::try_named(fid, EntityKind::Edge, role).ok();
            solid.add_edge(
                id_gen.next(),
                [v[v0], v[v1]],
                Curve::Line {
                    origin: p0,
                    direction,
                },
                [0.0, 1.0],
                name,
            )
        })
        .collect();

    // 6 faces with roles
    struct FaceDef {
        half_edges: [(usize, bool, usize); 4],
        surface: Surface,
        same_sense: bool,
        role: &'static str,
    }

    let face_defs = [
        // Front face (Z = -hz): normal -Z
        FaceDef {
            half_edges: [(0, true, 0), (1, true, 1), (2, true, 3), (3, true, 2)],
            surface: Surface::Plane {
                origin: Point::new(0.0, 0.0, -hz),
                normal: -Vec3::z(),
                u_axis: Vec3::x(),
                v_axis: Vec3::y(),
            },
            same_sense: true,
            role: "f_z_neg",
        },
        // Back face (Z = +hz): normal +Z
        FaceDef {
            half_edges: [(4, false, 5), (7, false, 4), (6, false, 6), (5, false, 7)],
            surface: Surface::Plane {
                origin: Point::new(0.0, 0.0, hz),
                normal: Vec3::z(),
                u_axis: -Vec3::x(),
                v_axis: Vec3::y(),
            },
            same_sense: true,
            role: "f_z_pos",
        },
        // Bottom face (Y = -hy): normal -Y
        FaceDef {
            half_edges: [(8, true, 0), (4, true, 4), (9, false, 5), (0, false, 1)],
            surface: Surface::Plane {
                origin: Point::new(0.0, -hy, 0.0),
                normal: -Vec3::y(),
                u_axis: Vec3::x(),
                v_axis: Vec3::z(),
            },
            same_sense: true,
            role: "f_y_neg",
        },
        // Top face (Y = +hy): normal +Y
        FaceDef {
            half_edges: [(2, false, 2), (10, true, 3), (6, true, 7), (11, false, 6)],
            surface: Surface::Plane {
                origin: Point::new(0.0, hy, 0.0),
                normal: Vec3::y(),
                u_axis: Vec3::x(),
                v_axis: Vec3::z(),
            },
            same_sense: true,
            role: "f_y_pos",
        },
        // Left face (X = -hx): normal -X
        FaceDef {
            half_edges: [(3, false, 0), (11, true, 2), (7, true, 6), (8, false, 4)],
            surface: Surface::Plane {
                origin: Point::new(-hx, 0.0, 0.0),
                normal: -Vec3::x(),
                u_axis: Vec3::y(),
                v_axis: Vec3::z(),
            },
            same_sense: true,
            role: "f_x_neg",
        },
        // Right face (X = +hx): normal +X
        FaceDef {
            half_edges: [(9, true, 1), (5, true, 5), (10, false, 7), (1, false, 3)],
            surface: Surface::Plane {
                origin: Point::new(hx, 0.0, 0.0),
                normal: Vec3::x(),
                u_axis: Vec3::y(),
                v_axis: Vec3::z(),
            },
            same_sense: true,
            role: "f_x_pos",
        },
    ];

    let mut face_indices = Vec::new();

    for face_def in &face_defs {
        let he_indices: Vec<usize> = face_def
            .half_edges
            .iter()
            .map(|&(edge_idx, forward, start_v)| {
                solid.add_half_edge(id_gen.next(), v[start_v], e[edge_idx], forward)
            })
            .collect();

        let loop_idx = solid.add_loop(id_gen.next(), he_indices);
        let name = EntityRef::try_named(fid, EntityKind::Face, face_def.role).ok();
        let face_idx = solid.add_face(
            id_gen.next(),
            face_def.surface.clone(),
            loop_idx,
            vec![],
            face_def.same_sense,
            name,
        );
        face_indices.push(face_idx);
    }

    solid.add_shell(id_gen.next(), face_indices, true);

    Ok(solid)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::brep::topology::IdGenerator;
    use crate::error::KernelError;

    #[test]
    fn test_cuboid_topology() {
        let mut id_gen = IdGenerator::new(0);
        let solid = make_cuboid(10.0, 20.0, 30.0, &mut id_gen).unwrap();

        assert_eq!(solid.vertices.len(), 8, "cuboid should have 8 vertices");
        assert_eq!(solid.edges.len(), 12, "cuboid should have 12 edges");
        assert_eq!(solid.faces.len(), 6, "cuboid should have 6 faces");
        assert_eq!(solid.shells.len(), 1, "cuboid should have 1 shell");
        assert!(solid.shells[0].closed, "cuboid shell should be closed");
        assert_eq!(
            solid.half_edges.len(),
            24,
            "cuboid should have 24 half-edges"
        );
        assert_eq!(solid.loops.len(), 6, "cuboid should have 6 loops");
    }

    #[test]
    fn test_cuboid_vertex_positions() {
        let mut id_gen = IdGenerator::new(0);
        let solid = make_cuboid(2.0, 4.0, 6.0, &mut id_gen).unwrap();

        let expected = [
            Point::new(-1.0, -2.0, -3.0),
            Point::new(1.0, -2.0, -3.0),
            Point::new(-1.0, 2.0, -3.0),
            Point::new(1.0, 2.0, -3.0),
            Point::new(-1.0, -2.0, 3.0),
            Point::new(1.0, -2.0, 3.0),
            Point::new(-1.0, 2.0, 3.0),
            Point::new(1.0, 2.0, 3.0),
        ];

        for (v, exp) in solid.vertices.iter().zip(expected.iter()) {
            assert!(
                (v.point - exp).norm() < 1e-12,
                "vertex {:?} doesn't match expected {:?}",
                v.point,
                exp
            );
        }
    }

    #[test]
    fn test_cuboid_deterministic() {
        let mut id_gen1 = IdGenerator::new(0);
        let mut id_gen2 = IdGenerator::new(0);

        let solid1 = make_cuboid(10.0, 20.0, 30.0, &mut id_gen1).unwrap();
        let solid2 = make_cuboid(10.0, 20.0, 30.0, &mut id_gen2).unwrap();

        assert_eq!(solid1.vertices.len(), solid2.vertices.len());
        for (v1, v2) in solid1.vertices.iter().zip(solid2.vertices.iter()) {
            assert_eq!(v1.id, v2.id);
            assert_eq!(v1.point, v2.point);
        }

        assert_eq!(solid1.edges.len(), solid2.edges.len());
        for (e1, e2) in solid1.edges.iter().zip(solid2.edges.iter()) {
            assert_eq!(e1.id, e2.id);
            assert_eq!(e1.vertices, e2.vertices);
        }

        assert_eq!(solid1.faces.len(), solid2.faces.len());
        for (f1, f2) in solid1.faces.iter().zip(solid2.faces.iter()) {
            assert_eq!(f1.id, f2.id);
        }
    }

    #[test]
    fn test_make_cuboid_invalid_dimensions() {
        let mut gen = IdGenerator::new(0);

        assert!(matches!(
            make_cuboid(0.0, 1.0, 1.0, &mut gen),
            Err(KernelError::InvalidParameter { kind: "width" })
        ));
        assert!(matches!(
            make_cuboid(1.0, 0.0, 1.0, &mut gen),
            Err(KernelError::InvalidParameter { kind: "height" })
        ));
        assert!(matches!(
            make_cuboid(1.0, 1.0, 0.0, &mut gen),
            Err(KernelError::InvalidParameter { kind: "depth" })
        ));

        assert!(matches!(
            make_cuboid(-1.0, 1.0, 1.0, &mut gen),
            Err(KernelError::InvalidParameter { kind: "width" })
        ));
        assert!(matches!(
            make_cuboid(1.0, -1.0, 1.0, &mut gen),
            Err(KernelError::InvalidParameter { kind: "height" })
        ));
        assert!(matches!(
            make_cuboid(1.0, 1.0, -1.0, &mut gen),
            Err(KernelError::InvalidParameter { kind: "depth" })
        ));

        assert!(matches!(
            make_cuboid(f64::NAN, 1.0, 1.0, &mut gen),
            Err(KernelError::InvalidParameter { kind: "width" })
        ));
        assert!(matches!(
            make_cuboid(1.0, f64::NAN, 1.0, &mut gen),
            Err(KernelError::InvalidParameter { kind: "height" })
        ));
        assert!(matches!(
            make_cuboid(1.0, 1.0, f64::NAN, &mut gen),
            Err(KernelError::InvalidParameter { kind: "depth" })
        ));

        assert!(matches!(
            make_cuboid(f64::INFINITY, 1.0, 1.0, &mut gen),
            Err(KernelError::InvalidParameter { kind: "width" })
        ));
        assert!(matches!(
            make_cuboid(1.0, f64::INFINITY, 1.0, &mut gen),
            Err(KernelError::InvalidParameter { kind: "height" })
        ));
        assert!(matches!(
            make_cuboid(1.0, 1.0, f64::INFINITY, &mut gen),
            Err(KernelError::InvalidParameter { kind: "depth" })
        ));

        assert!(matches!(
            make_cuboid(f64::NEG_INFINITY, 1.0, 1.0, &mut gen),
            Err(KernelError::InvalidParameter { kind: "width" })
        ));
        assert!(matches!(
            make_cuboid(1.0, f64::NEG_INFINITY, 1.0, &mut gen),
            Err(KernelError::InvalidParameter { kind: "height" })
        ));
        assert!(matches!(
            make_cuboid(1.0, 1.0, f64::NEG_INFINITY, &mut gen),
            Err(KernelError::InvalidParameter { kind: "depth" })
        ));
    }

    #[test]
    fn test_make_cuboid_near_zero_positive_ok() {
        let mut gen = IdGenerator::new(0);
        let result = make_cuboid(1e-9, 1.0, 1.0, &mut gen);
        assert!(
            result.is_ok(),
            "near-zero positive should be Ok per exact <= 0.0 policy"
        );
    }

    #[test]
    fn test_cuboid_entity_names() {
        let mut gen = IdGenerator::new(0);
        let solid = make_cuboid(2.0, 2.0, 2.0, &mut gen).unwrap();

        for v in &solid.vertices {
            assert!(v.name.is_some(), "vertex should have a name");
        }
        for e in &solid.edges {
            assert!(e.name.is_some(), "edge should have a name");
        }
        for f in &solid.faces {
            assert!(f.name.is_some(), "face should have a name");
        }
    }
}
