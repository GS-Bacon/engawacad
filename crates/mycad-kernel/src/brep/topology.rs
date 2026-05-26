use crate::geometry::curve::Curve;
use crate::geometry::surface::Surface;
use crate::geometry::Point;
use serde::{Deserialize, Serialize};

/// Unique identifier for topological entities.
/// Generated deterministically by creation order within a feature.
pub type EntityId = u64;

/// A vertex in B-rep — a point in 3D space.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Vertex {
    pub id: EntityId,
    pub point: Point,
}

/// A half-edge — one side of an edge, used within a loop (wire).
/// Half-edges are oriented: they go from `start_vertex` to the next
/// half-edge's start vertex.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HalfEdge {
    pub id: EntityId,
    /// Index of the start vertex in the parent solid's vertex list.
    pub start_vertex: usize,
    /// Index of the edge this half-edge belongs to.
    pub edge: usize,
    /// Whether this half-edge follows the edge's natural direction.
    pub forward: bool,
}

/// An edge — a curve segment bounded by two vertices.
/// Each edge has exactly two half-edges (one for each adjacent face).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Edge {
    pub id: EntityId,
    /// Indices of the two bounding vertices in the parent solid's vertex list.
    pub vertices: [usize; 2],
    /// The geometric curve this edge lies on.
    pub curve: Curve,
    /// Parameter range [t_start, t_end] on the curve.
    pub t_range: [f64; 2],
}

/// A loop (wire) — a closed sequence of half-edges forming a boundary.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Loop {
    pub id: EntityId,
    /// Ordered indices of half-edges in the parent solid's half-edge list.
    pub half_edges: Vec<usize>,
}

/// A face — a bounded region on a surface.
/// Has one outer loop and zero or more inner loops (holes).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Face {
    pub id: EntityId,
    /// The geometric surface this face lies on.
    pub surface: Surface,
    /// Index of the outer loop in the parent solid's loop list.
    pub outer_loop: usize,
    /// Indices of inner loops (holes) in the parent solid's loop list.
    pub inner_loops: Vec<usize>,
    /// Whether the face normal matches the surface normal.
    pub same_sense: bool,
}

/// A shell — a connected set of faces forming a closed or open surface.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Shell {
    pub id: EntityId,
    /// Indices of faces in the parent solid's face list.
    pub faces: Vec<usize>,
    /// Whether this shell is closed (watertight).
    pub closed: bool,
}

/// A solid — the top-level B-rep entity.
/// Contains all topological entities in flat arrays for deterministic ordering.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Solid {
    pub id: EntityId,
    pub vertices: Vec<Vertex>,
    pub half_edges: Vec<HalfEdge>,
    pub edges: Vec<Edge>,
    pub loops: Vec<Loop>,
    pub faces: Vec<Face>,
    pub shells: Vec<Shell>,
}

impl Solid {
    /// Create an empty solid with the given ID.
    pub fn new(id: EntityId) -> Self {
        Self {
            id,
            vertices: Vec::new(),
            half_edges: Vec::new(),
            edges: Vec::new(),
            loops: Vec::new(),
            faces: Vec::new(),
            shells: Vec::new(),
        }
    }

    /// Add a vertex and return its index.
    pub fn add_vertex(&mut self, id: EntityId, point: Point) -> usize {
        let idx = self.vertices.len();
        self.vertices.push(Vertex { id, point });
        idx
    }

    /// Add an edge and return its index.
    pub fn add_edge(
        &mut self,
        id: EntityId,
        vertices: [usize; 2],
        curve: Curve,
        t_range: [f64; 2],
    ) -> usize {
        let idx = self.edges.len();
        self.edges.push(Edge {
            id,
            vertices,
            curve,
            t_range,
        });
        idx
    }

    /// Add a half-edge and return its index.
    pub fn add_half_edge(
        &mut self,
        id: EntityId,
        start_vertex: usize,
        edge: usize,
        forward: bool,
    ) -> usize {
        let idx = self.half_edges.len();
        self.half_edges.push(HalfEdge {
            id,
            start_vertex,
            edge,
            forward,
        });
        idx
    }

    /// Add a loop and return its index.
    pub fn add_loop(&mut self, id: EntityId, half_edges: Vec<usize>) -> usize {
        let idx = self.loops.len();
        self.loops.push(Loop { id, half_edges });
        idx
    }

    /// Add a face and return its index.
    pub fn add_face(
        &mut self,
        id: EntityId,
        surface: Surface,
        outer_loop: usize,
        inner_loops: Vec<usize>,
        same_sense: bool,
    ) -> usize {
        let idx = self.faces.len();
        self.faces.push(Face {
            id,
            surface,
            outer_loop,
            inner_loops,
            same_sense,
        });
        idx
    }

    /// Add a shell and return its index.
    pub fn add_shell(&mut self, id: EntityId, faces: Vec<usize>, closed: bool) -> usize {
        let idx = self.shells.len();
        self.shells.push(Shell { id, faces, closed });
        idx
    }

    /// Validate manifold topology: each edge has exactly 2 HEs with opposite orientation,
    /// all loops are closed, face/shell indices are in-bounds.
    /// Self-adjacent periodic faces (e.g. full sphere with 1 seam edge shared by 2 HEs)
    /// are explicitly permitted.
    pub fn validate_manifold(&self) -> Result<(), &'static str> {
        // Edge ↔ HE correspondence
        let mut edge_he_count: std::collections::HashMap<usize, Vec<bool>> =
            std::collections::HashMap::new();
        for he in &self.half_edges {
            if he.edge >= self.edges.len() {
                return Err("half-edge references out-of-bounds edge");
            }
            edge_he_count.entry(he.edge).or_default().push(he.forward);
        }

        for edge_idx in 0..self.edges.len() {
            match edge_he_count.get(&edge_idx) {
                None => return Err("edge has no half-edges"),
                Some(forwards) => {
                    if forwards.len() != 2 {
                        return Err("edge must have exactly 2 half-edges");
                    }
                    if forwards[0] == forwards[1] {
                        return Err("edge half-edges must have opposite orientation");
                    }
                }
            }
        }

        // Loop closure and face/shell bounds
        for face in &self.faces {
            if face.outer_loop >= self.loops.len() {
                return Err("face references out-of-bounds outer loop");
            }
            for &il in &face.inner_loops {
                if il >= self.loops.len() {
                    return Err("face references out-of-bounds inner loop");
                }
            }
            // Validate outer loop and all inner loops identically
            let loop_indices: Vec<usize> = std::iter::once(face.outer_loop)
                .chain(face.inner_loops.iter().copied())
                .collect();
            for loop_idx in loop_indices {
                let lp = &self.loops[loop_idx];
                if lp.half_edges.is_empty() {
                    return Err("loop must not be empty");
                }
                for i in 0..lp.half_edges.len() {
                    let he_i = lp.half_edges[i];
                    let he_next_i = lp.half_edges[(i + 1) % lp.half_edges.len()];
                    if he_i >= self.half_edges.len() || he_next_i >= self.half_edges.len() {
                        return Err("loop references out-of-bounds half-edge");
                    }
                    let he_cur = &self.half_edges[he_i];
                    let he_next = &self.half_edges[he_next_i];
                    if he_cur.edge >= self.edges.len() {
                        return Err("half-edge references out-of-bounds edge");
                    }
                    if he_next.start_vertex >= self.vertices.len() {
                        return Err("half-edge references out-of-bounds vertex");
                    }
                    let cur_edge = &self.edges[he_cur.edge];
                    let end_v = if he_cur.forward {
                        cur_edge.vertices[1]
                    } else {
                        cur_edge.vertices[0]
                    };
                    if end_v != he_next.start_vertex {
                        return Err("loop is not closed");
                    }
                }
            }
        }

        for shell in &self.shells {
            for &fi in &shell.faces {
                if fi >= self.faces.len() {
                    return Err("shell references out-of-bounds face");
                }
            }
        }

        Ok(())
    }
}

/// Counter for generating deterministic entity IDs.
/// Each feature gets its own counter starting from a feature-specific offset.
#[derive(Debug, Clone)]
pub struct IdGenerator {
    next_id: EntityId,
}

impl IdGenerator {
    pub fn new(start: EntityId) -> Self {
        Self { next_id: start }
    }

    #[allow(clippy::should_implement_trait)]
    pub fn next(&mut self) -> EntityId {
        let id = self.next_id;
        self.next_id += 1;
        id
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_id_generator_is_deterministic() {
        let mut gen1 = IdGenerator::new(100);
        let mut gen2 = IdGenerator::new(100);
        for _ in 0..10 {
            assert_eq!(gen1.next(), gen2.next());
        }
    }

    #[test]
    fn test_solid_add_vertex() {
        let mut solid = Solid::new(0);
        let idx = solid.add_vertex(1, Point::new(1.0, 2.0, 3.0));
        assert_eq!(idx, 0);
        assert_eq!(solid.vertices[0].id, 1);
        assert_eq!(solid.vertices[0].point, Point::new(1.0, 2.0, 3.0));
    }

    /// F02: isolated edge (0 half-edges) must fail manifold validation.
    #[test]
    fn test_validate_manifold_isolated_edge_zero_he() {
        use crate::geometry::curve::Curve;
        use crate::geometry::Vec3;

        let mut s = Solid::new(0);
        let v0 = s.add_vertex(1, Point::new(0.0, 0.0, 0.0));
        let v1 = s.add_vertex(2, Point::new(1.0, 0.0, 0.0));
        // Edge with no half-edges at all
        let e0 = s.add_edge(
            3,
            [v0, v1],
            Curve::Line {
                origin: Point::origin(),
                direction: Vec3::x(),
            },
            [0.0, 1.0],
        );
        // Create a valid face/loop using a different edge setup
        let e1 = s.add_edge(
            4,
            [v0, v1],
            Curve::Line {
                origin: Point::origin(),
                direction: Vec3::x(),
            },
            [0.0, 1.0],
        );
        let he0 = s.add_half_edge(5, v0, e1, true);
        let he1 = s.add_half_edge(6, v1, e1, false);
        let lp = s.add_loop(7, vec![he0, he1]);
        s.add_face(
            8,
            crate::geometry::surface::Surface::Plane {
                origin: Point::origin(),
                normal: Vec3::z(),
                u_axis: Vec3::x(),
                v_axis: Vec3::y(),
            },
            lp,
            vec![],
            true,
        );
        s.add_shell(9, vec![0], true);
        let _ = e0; // e0 is the isolated edge with 0 HEs
        assert!(
            s.validate_manifold().is_err(),
            "isolated edge with 0 half-edges must fail validation"
        );
    }

    /// F02: edge with only 1 half-edge must fail manifold validation.
    #[test]
    fn test_validate_manifold_edge_with_one_he() {
        use crate::geometry::curve::Curve;
        use crate::geometry::Vec3;

        let mut s = Solid::new(0);
        let v0 = s.add_vertex(1, Point::new(0.0, 0.0, 0.0));
        let v1 = s.add_vertex(2, Point::new(1.0, 0.0, 0.0));
        let e0 = s.add_edge(
            3,
            [v0, v1],
            Curve::Line {
                origin: Point::origin(),
                direction: Vec3::x(),
            },
            [0.0, 1.0],
        );
        // Only 1 half-edge for this edge (should be 2)
        let he0 = s.add_half_edge(4, v0, e0, true);
        let lp = s.add_loop(5, vec![he0]);
        s.add_face(
            6,
            crate::geometry::surface::Surface::Plane {
                origin: Point::origin(),
                normal: Vec3::z(),
                u_axis: Vec3::x(),
                v_axis: Vec3::y(),
            },
            lp,
            vec![],
            true,
        );
        s.add_shell(7, vec![0], true);
        assert!(
            s.validate_manifold().is_err(),
            "edge with only 1 half-edge must fail validation"
        );
    }

    /// F03: coincident vertices at different indices — loop looks closed by coordinates but is not.
    #[test]
    fn test_validate_manifold_coincident_vertices_not_closed() {
        use crate::geometry::curve::Curve;
        use crate::geometry::Vec3;

        let mut s = Solid::new(0);
        // v0 and v1 have same coordinates but are different indices
        let v0 = s.add_vertex(1, Point::new(0.0, 0.0, 0.0));
        let v1 = s.add_vertex(2, Point::new(0.0, 0.0, 0.0));
        let v2 = s.add_vertex(3, Point::new(1.0, 0.0, 0.0));
        let e0 = s.add_edge(
            4,
            [v0, v2],
            Curve::Line {
                origin: Point::origin(),
                direction: Vec3::x(),
            },
            [0.0, 1.0],
        );
        let e1 = s.add_edge(
            5,
            [v2, v1],
            Curve::Line {
                origin: Point::new(1.0, 0.0, 0.0),
                direction: -Vec3::x(),
            },
            [0.0, 1.0],
        );
        let he0 = s.add_half_edge(6, v0, e0, true);
        let he1 = s.add_half_edge(7, v2, e1, true);
        let he0r = s.add_half_edge(8, v2, e0, false);
        let he1r = s.add_half_edge(9, v1, e1, false);
        // Loop: he0 ends at v2, he1 starts at v2 ✓, he1 ends at v1,
        // but loop wraps: he0 start is v0, not v1 — different index despite same coords
        let lp = s.add_loop(10, vec![he0, he1]);
        let lp2 = s.add_loop(11, vec![he0r, he1r]);
        s.add_face(
            12,
            crate::geometry::surface::Surface::Plane {
                origin: Point::origin(),
                normal: Vec3::z(),
                u_axis: Vec3::x(),
                v_axis: Vec3::y(),
            },
            lp,
            vec![],
            true,
        );
        s.add_face(
            13,
            crate::geometry::surface::Surface::Plane {
                origin: Point::origin(),
                normal: Vec3::z(),
                u_axis: Vec3::x(),
                v_axis: Vec3::y(),
            },
            lp2,
            vec![],
            true,
        );
        s.add_shell(14, vec![0, 1], true);
        assert!(
            s.validate_manifold().is_err(),
            "loop with coincident-but-different-index vertices must fail validation"
        );
    }

    /// Adversarial: out-of-bounds half-edge edge reference.
    #[test]
    fn test_validate_manifold_he_oob_edge() {
        use crate::geometry::curve::Curve;
        use crate::geometry::Vec3;

        let mut s = Solid::new(0);
        let v0 = s.add_vertex(1, Point::new(0.0, 0.0, 0.0));
        let v1 = s.add_vertex(2, Point::new(1.0, 0.0, 0.0));
        let e0 = s.add_edge(
            3,
            [v0, v1],
            Curve::Line {
                origin: Point::origin(),
                direction: Vec3::x(),
            },
            [0.0, 1.0],
        );
        // Half-edge referencing edge 99 (out-of-bounds)
        let he0 = s.add_half_edge(4, v0, 99, true);
        let he1 = s.add_half_edge(5, v1, e0, false);
        let lp = s.add_loop(6, vec![he0, he1]);
        s.add_face(
            7,
            crate::geometry::surface::Surface::Plane {
                origin: Point::origin(),
                normal: Vec3::z(),
                u_axis: Vec3::x(),
                v_axis: Vec3::y(),
            },
            lp,
            vec![],
            true,
        );
        s.add_shell(8, vec![0], true);
        assert!(
            s.validate_manifold().is_err(),
            "out-of-bounds edge reference must fail"
        );
    }

    /// Adversarial: face referencing out-of-bounds outer loop.
    #[test]
    fn test_validate_manifold_face_oob_loop() {
        use crate::geometry::surface::Surface;

        let mut s = Solid::new(0);
        let v0 = s.add_vertex(1, Point::new(0.0, 0.0, 0.0));
        let _ = v0;
        s.add_face(
            2,
            Surface::Plane {
                origin: Point::origin(),
                normal: crate::geometry::Vec3::z(),
                u_axis: crate::geometry::Vec3::x(),
                v_axis: crate::geometry::Vec3::y(),
            },
            99,
            vec![],
            true,
        );
        s.add_shell(3, vec![0], true);
        assert!(
            s.validate_manifold().is_err(),
            "out-of-bounds outer loop must fail"
        );
    }

    /// Adversarial: shell referencing out-of-bounds face.
    #[test]
    fn test_validate_manifold_shell_oob_face() {
        use crate::geometry::curve::Curve;
        use crate::geometry::surface::Surface;
        use crate::geometry::Vec3;

        let mut s = Solid::new(0);
        let v0 = s.add_vertex(1, Point::new(0.0, 0.0, 0.0));
        let v1 = s.add_vertex(2, Point::new(1.0, 0.0, 0.0));
        let e0 = s.add_edge(
            3,
            [v0, v1],
            Curve::Line {
                origin: Point::origin(),
                direction: Vec3::x(),
            },
            [0.0, 1.0],
        );
        let he0 = s.add_half_edge(4, v0, e0, true);
        let he1 = s.add_half_edge(5, v1, e0, false);
        let lp = s.add_loop(6, vec![he0, he1]);
        s.add_face(
            7,
            Surface::Plane {
                origin: Point::origin(),
                normal: Vec3::z(),
                u_axis: Vec3::x(),
                v_axis: Vec3::y(),
            },
            lp,
            vec![],
            true,
        );
        s.add_shell(8, vec![99], true);
        assert!(
            s.validate_manifold().is_err(),
            "out-of-bounds face reference in shell must fail"
        );
    }

    /// Adversarial: two half-edges with same orientation on one edge.
    #[test]
    fn test_validate_manifold_same_orientation_hes() {
        use crate::geometry::curve::Curve;
        use crate::geometry::surface::Surface;
        use crate::geometry::Vec3;

        let mut s = Solid::new(0);
        let v0 = s.add_vertex(1, Point::new(0.0, 0.0, 0.0));
        let v1 = s.add_vertex(2, Point::new(1.0, 0.0, 0.0));
        let e0 = s.add_edge(
            3,
            [v0, v1],
            Curve::Line {
                origin: Point::origin(),
                direction: Vec3::x(),
            },
            [0.0, 1.0],
        );
        // Both HEs forward — should fail
        let he0 = s.add_half_edge(4, v0, e0, true);
        let he1 = s.add_half_edge(5, v1, e0, true);
        let lp = s.add_loop(6, vec![he0, he1]);
        s.add_face(
            7,
            Surface::Plane {
                origin: Point::origin(),
                normal: Vec3::z(),
                u_axis: Vec3::x(),
                v_axis: Vec3::y(),
            },
            lp,
            vec![],
            true,
        );
        s.add_shell(8, vec![0], true);
        assert!(
            s.validate_manifold().is_err(),
            "two half-edges with same orientation must fail"
        );
    }

    /// Adversarial: empty outer loop.
    #[test]
    fn test_validate_manifold_empty_outer_loop() {
        use crate::geometry::surface::Surface;
        use crate::geometry::Vec3;

        let mut s = Solid::new(0);
        let lp = s.add_loop(1, vec![]);
        s.add_face(
            2,
            Surface::Plane {
                origin: Point::origin(),
                normal: Vec3::z(),
                u_axis: Vec3::x(),
                v_axis: Vec3::y(),
            },
            lp,
            vec![],
            true,
        );
        s.add_shell(3, vec![0], true);
        assert!(s.validate_manifold().is_err(), "empty outer loop must fail");
    }

    /// Adversarial: face with out-of-bounds inner loop.
    #[test]
    fn test_validate_manifold_face_oob_inner_loop() {
        use crate::geometry::curve::Curve;
        use crate::geometry::surface::Surface;
        use crate::geometry::Vec3;

        let mut s = Solid::new(0);
        let v0 = s.add_vertex(1, Point::new(0.0, 0.0, 0.0));
        let v1 = s.add_vertex(2, Point::new(1.0, 0.0, 0.0));
        let e0 = s.add_edge(
            3,
            [v0, v1],
            Curve::Line {
                origin: Point::origin(),
                direction: Vec3::x(),
            },
            [0.0, 1.0],
        );
        let he0 = s.add_half_edge(4, v0, e0, true);
        let he1 = s.add_half_edge(5, v1, e0, false);
        let lp = s.add_loop(6, vec![he0, he1]);
        s.add_face(
            7,
            Surface::Plane {
                origin: Point::origin(),
                normal: Vec3::z(),
                u_axis: Vec3::x(),
                v_axis: Vec3::y(),
            },
            lp,
            vec![99],
            true,
        );
        s.add_shell(8, vec![0], true);
        assert!(
            s.validate_manifold().is_err(),
            "out-of-bounds inner loop must fail"
        );
    }
}
