use crate::brep::topology::{IdGenerator, Solid};
use crate::geometry::curve::Curve;
use crate::geometry::surface::Surface;
use crate::geometry::Point;
use crate::geometry::Vec3;

/// Create a cuboid (box) B-rep solid centered at the origin.
///
/// Parameters:
/// - `dx`: width (X dimension)
/// - `dy`: height (Y dimension)
/// - `dz`: depth (Z dimension)
/// - `id_gen`: deterministic ID generator
///
/// The cuboid spans from (-dx/2, -dy/2, -dz/2) to (dx/2, dy/2, dz/2).
pub fn make_cuboid(dx: f64, dy: f64, dz: f64, id_gen: &mut IdGenerator) -> Solid {
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
    let v_positions = [
        Point::new(-hx, -hy, -hz), // 0: front-bottom-left
        Point::new(hx, -hy, -hz),  // 1: front-bottom-right
        Point::new(-hx, hy, -hz),  // 2: front-top-left
        Point::new(hx, hy, -hz),   // 3: front-top-right
        Point::new(-hx, -hy, hz),  // 4: back-bottom-left
        Point::new(hx, -hy, hz),   // 5: back-bottom-right
        Point::new(-hx, hy, hz),   // 6: back-top-left
        Point::new(hx, hy, hz),    // 7: back-top-right
    ];

    let v: Vec<usize> = v_positions
        .iter()
        .map(|p| solid.add_vertex(id_gen.next(), *p))
        .collect();

    // 12 edges of the cuboid
    // Each edge is defined by [start_vertex_index, end_vertex_index]
    let edge_defs: [(usize, usize); 12] = [
        (0, 1), // e0:  bottom-front
        (1, 3), // e1:  right-front
        (3, 2), // e2:  top-front
        (2, 0), // e3:  left-front
        (4, 5), // e4:  bottom-back
        (5, 7), // e5:  right-back
        (7, 6), // e6:  top-back
        (6, 4), // e7:  left-back
        (0, 4), // e8:  bottom-left
        (1, 5), // e9:  bottom-right
        (3, 7), // e10: top-right
        (2, 6), // e11: top-left
    ];

    let e: Vec<usize> = edge_defs
        .iter()
        .map(|&(v0, v1)| {
            let p0 = solid.vertices[v[v0]].point;
            let p1 = solid.vertices[v[v1]].point;
            let direction = p1 - p0;
            solid.add_edge(
                id_gen.next(),
                [v[v0], v[v1]],
                Curve::Line {
                    origin: p0,
                    direction,
                },
                [0.0, 1.0],
            )
        })
        .collect();

    // 6 faces of the cuboid
    // Each face is defined by its 4 edges and their orientation (forward/backward),
    // plus the surface definition.
    struct FaceDef {
        // (edge_index, forward, start_vertex_index)
        half_edges: [(usize, bool, usize); 4],
        surface: Surface,
        same_sense: bool,
    }

    let face_defs = [
        // Front face (Z = -hz): vertices 0,1,3,2 — normal -Z
        FaceDef {
            half_edges: [
                (0, true, 0),   // e0: 0->1
                (1, true, 1),   // e1: 1->3
                (2, true, 3),   // e2: 3->2
                (3, true, 2),   // e3: 2->0
            ],
            surface: Surface::Plane {
                origin: Point::new(0.0, 0.0, -hz),
                normal: -Vec3::z(),
                u_axis: Vec3::x(),
                v_axis: Vec3::y(),
            },
            same_sense: true,
        },
        // Back face (Z = +hz): vertices 5,4,6,7 — normal +Z
        FaceDef {
            half_edges: [
                (4, false, 5),  // e4: 5->4
                (7, false, 4),  // e7: 4->6
                (6, false, 6),  // e6: 6->7
                (5, false, 7),  // e5: 7->5
            ],
            surface: Surface::Plane {
                origin: Point::new(0.0, 0.0, hz),
                normal: Vec3::z(),
                u_axis: -Vec3::x(),
                v_axis: Vec3::y(),
            },
            same_sense: true,
        },
        // Bottom face (Y = -hy): vertices 0,4,5,1 — normal -Y
        FaceDef {
            half_edges: [
                (8, true, 0),   // e8: 0->4
                (4, true, 4),   // e4: 4->5
                (9, false, 5),  // e9: 5->1
                (0, false, 1),  // e0: 1->0
            ],
            surface: Surface::Plane {
                origin: Point::new(0.0, -hy, 0.0),
                normal: -Vec3::y(),
                u_axis: Vec3::x(),
                v_axis: Vec3::z(),
            },
            same_sense: true,
        },
        // Top face (Y = +hy): vertices 2,3,7,6 — normal +Y
        FaceDef {
            half_edges: [
                (2, false, 2),   // e2: 2->3
                (10, true, 3),   // e10: 3->7
                (6, true, 7),    // e6: 7->6
                (11, false, 6),  // e11: 6->2
            ],
            surface: Surface::Plane {
                origin: Point::new(0.0, hy, 0.0),
                normal: Vec3::y(),
                u_axis: Vec3::x(),
                v_axis: Vec3::z(),
            },
            same_sense: true,
        },
        // Left face (X = -hx): vertices 0,2,6,4 — normal -X
        FaceDef {
            half_edges: [
                (3, false, 0),   // e3: 0->2
                (11, true, 2),   // e11: 2->6
                (7, true, 6),    // e7: 6->4
                (8, false, 4),   // e8: 4->0
            ],
            surface: Surface::Plane {
                origin: Point::new(-hx, 0.0, 0.0),
                normal: -Vec3::x(),
                u_axis: Vec3::y(),
                v_axis: Vec3::z(),
            },
            same_sense: true,
        },
        // Right face (X = +hx): vertices 1,5,7,3 — normal +X
        FaceDef {
            half_edges: [
                (9, true, 1),    // e9: 1->5
                (5, true, 5),    // e5: 5->7
                (10, false, 7),  // e10: 7->3
                (1, false, 3),   // e1: 3->1
            ],
            surface: Surface::Plane {
                origin: Point::new(hx, 0.0, 0.0),
                normal: Vec3::x(),
                u_axis: Vec3::y(),
                v_axis: Vec3::z(),
            },
            same_sense: true,
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
        let face_idx = solid.add_face(
            id_gen.next(),
            face_def.surface.clone(),
            loop_idx,
            vec![],
            face_def.same_sense,
        );
        face_indices.push(face_idx);
    }

    solid.add_shell(id_gen.next(), face_indices, true);

    solid
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::brep::topology::IdGenerator;

    #[test]
    fn test_cuboid_topology() {
        let mut id_gen = IdGenerator::new(0);
        let solid = make_cuboid(10.0, 20.0, 30.0, &mut id_gen);

        assert_eq!(solid.vertices.len(), 8, "cuboid should have 8 vertices");
        assert_eq!(solid.edges.len(), 12, "cuboid should have 12 edges");
        assert_eq!(solid.faces.len(), 6, "cuboid should have 6 faces");
        assert_eq!(solid.shells.len(), 1, "cuboid should have 1 shell");
        assert!(solid.shells[0].closed, "cuboid shell should be closed");
        // 6 faces * 4 half-edges = 24 half-edges
        assert_eq!(
            solid.half_edges.len(),
            24,
            "cuboid should have 24 half-edges"
        );
        // 6 loops (one per face)
        assert_eq!(solid.loops.len(), 6, "cuboid should have 6 loops");
    }

    #[test]
    fn test_cuboid_vertex_positions() {
        let mut id_gen = IdGenerator::new(0);
        let solid = make_cuboid(2.0, 4.0, 6.0, &mut id_gen);

        // Check that all vertices are at the expected positions
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
        // Building the same cuboid twice with the same ID generator should
        // produce identical results.
        let mut id_gen1 = IdGenerator::new(0);
        let mut id_gen2 = IdGenerator::new(0);

        let solid1 = make_cuboid(10.0, 20.0, 30.0, &mut id_gen1);
        let solid2 = make_cuboid(10.0, 20.0, 30.0, &mut id_gen2);

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
}
