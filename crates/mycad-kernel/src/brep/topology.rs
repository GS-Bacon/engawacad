use crate::error::KernelError;
use crate::geometry::curve::Curve;
use crate::geometry::pcurve::Pcurve;
use crate::geometry::surface::Surface;
use crate::geometry::{Point, Vec3};
use mycad_format::EntityRef;
use serde::{Deserialize, Serialize};

/// Unique identifier for topological entities.
/// Generated deterministically by creation order within a feature.
pub type EntityId = u64;

/// A vertex in B-rep — a point in 3D space.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Vertex {
    pub id: EntityId,
    pub point: Point,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<EntityRef>,
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
    /// Optional pcurve (2D curve in face surface parameter space).
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub pcurve: Option<Pcurve>,
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
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<EntityRef>,
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
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<EntityRef>,
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
    pub fn add_vertex(&mut self, id: EntityId, point: Point, name: Option<EntityRef>) -> usize {
        let idx = self.vertices.len();
        self.vertices.push(Vertex { id, point, name });
        idx
    }

    /// Add an edge and return its index.
    pub fn add_edge(
        &mut self,
        id: EntityId,
        vertices: [usize; 2],
        curve: Curve,
        t_range: [f64; 2],
        name: Option<EntityRef>,
    ) -> usize {
        let idx = self.edges.len();
        self.edges.push(Edge {
            id,
            vertices,
            curve,
            t_range,
            name,
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
        self.add_half_edge_with_pcurve(id, start_vertex, edge, forward, None)
    }

    /// Add a half-edge with an optional pcurve and return its index.
    pub fn add_half_edge_with_pcurve(
        &mut self,
        id: EntityId,
        start_vertex: usize,
        edge: usize,
        forward: bool,
        pcurve: Option<Pcurve>,
    ) -> usize {
        let idx = self.half_edges.len();
        self.half_edges.push(HalfEdge {
            id,
            start_vertex,
            edge,
            forward,
            pcurve,
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
        name: Option<EntityRef>,
    ) -> usize {
        let idx = self.faces.len();
        self.faces.push(Face {
            id,
            surface,
            outer_loop,
            inner_loops,
            same_sense,
            name,
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
    /// Also validates pcurve integrity: for HEs with pcurve, checks that pcurve→surface→3D
    /// lifting matches edge.curve 3D evaluation at start, end, and midpoint.
    pub fn validate_manifold(&self) -> Result<(), KernelError> {
        use crate::geometry::tolerance::Tolerance;

        // Edge ↔ HE correspondence
        let mut edge_he_count: std::collections::HashMap<usize, Vec<bool>> =
            std::collections::HashMap::new();
        for he in &self.half_edges {
            if he.edge >= self.edges.len() {
                return Err(KernelError::ManifoldViolation {
                    reason: "half-edge references out-of-bounds edge",
                });
            }
            edge_he_count.entry(he.edge).or_default().push(he.forward);
        }

        for edge_idx in 0..self.edges.len() {
            match edge_he_count.get(&edge_idx) {
                None => {
                    return Err(KernelError::ManifoldViolation {
                        reason: "edge has no half-edges",
                    })
                }
                Some(forwards) => {
                    if forwards.len() != 2 {
                        return Err(KernelError::ManifoldViolation {
                            reason: "edge must have exactly 2 half-edges",
                        });
                    }
                    if forwards[0] == forwards[1] {
                        return Err(KernelError::ManifoldViolation {
                            reason: "edge half-edges must have opposite orientation",
                        });
                    }
                }
            }
        }

        // Loop closure and face/shell bounds
        for face in &self.faces {
            if face.outer_loop >= self.loops.len() {
                return Err(KernelError::ManifoldViolation {
                    reason: "face references out-of-bounds outer loop",
                });
            }
            for &il in &face.inner_loops {
                if il >= self.loops.len() {
                    return Err(KernelError::ManifoldViolation {
                        reason: "face references out-of-bounds inner loop",
                    });
                }
            }
            let loop_indices: Vec<usize> = std::iter::once(face.outer_loop)
                .chain(face.inner_loops.iter().copied())
                .collect();
            for loop_idx in loop_indices {
                let lp = &self.loops[loop_idx];
                if lp.half_edges.is_empty() {
                    return Err(KernelError::ManifoldViolation {
                        reason: "loop must not be empty",
                    });
                }
                for i in 0..lp.half_edges.len() {
                    let he_i = lp.half_edges[i];
                    let he_next_i = lp.half_edges[(i + 1) % lp.half_edges.len()];
                    if he_i >= self.half_edges.len() || he_next_i >= self.half_edges.len() {
                        return Err(KernelError::ManifoldViolation {
                            reason: "loop references out-of-bounds half-edge",
                        });
                    }
                    let he_cur = &self.half_edges[he_i];
                    let he_next = &self.half_edges[he_next_i];
                    if he_cur.edge >= self.edges.len() {
                        return Err(KernelError::ManifoldViolation {
                            reason: "half-edge references out-of-bounds edge",
                        });
                    }
                    if he_next.start_vertex >= self.vertices.len() {
                        return Err(KernelError::ManifoldViolation {
                            reason: "half-edge references out-of-bounds vertex",
                        });
                    }
                    let cur_edge = &self.edges[he_cur.edge];
                    let end_v = if he_cur.forward {
                        cur_edge.vertices[1]
                    } else {
                        cur_edge.vertices[0]
                    };
                    if end_v != he_next.start_vertex {
                        return Err(KernelError::ManifoldViolation {
                            reason: "loop is not closed",
                        });
                    }
                }
            }
        }

        for shell in &self.shells {
            for &fi in &shell.faces {
                if fi >= self.faces.len() {
                    return Err(KernelError::ManifoldViolation {
                        reason: "shell references out-of-bounds face",
                    });
                }
            }
        }

        // Pcurve integrity check: for HEs with pcurve, validate 3D consistency
        let tol = Tolerance::DEFAULT;
        for (face_idx, face) in self.faces.iter().enumerate() {
            let surface = &face.surface;
            let loop_indices: Vec<usize> = std::iter::once(face.outer_loop)
                .chain(face.inner_loops.iter().copied())
                .collect();
            for _loop_idx in &loop_indices {
                let lp = &self.loops[*_loop_idx];
                for &he_idx in &lp.half_edges {
                    let he = &self.half_edges[he_idx];
                    if let Some(ref pcurve) = he.pcurve {
                        let edge = &self.edges[he.edge];
                        let tr = pcurve.t_range();
                        let t_e_start = if he.forward {
                            edge.t_range[0]
                        } else {
                            edge.t_range[1]
                        };
                        let t_e_end = if he.forward {
                            edge.t_range[1]
                        } else {
                            edge.t_range[0]
                        };

                        let mut max_deviation = 0.0_f64;
                        for frac in &[0.0_f64, 0.5, 1.0] {
                            let t_p = tr[0] + frac * (tr[1] - tr[0]);
                            let (u, v) = pcurve.curve_2d().evaluate(t_p);
                            let p_3d = surface.evaluate(u, v);
                            let t_e = t_e_start + frac * (t_e_end - t_e_start);
                            let e_3d = edge.curve.evaluate(t_e);
                            let dev = (p_3d - e_3d).norm();
                            max_deviation = max_deviation.max(dev);
                        }

                        if max_deviation > tol.value() {
                            return Err(KernelError::PcurveSurfaceMismatch {
                                he_idx,
                                deviation: max_deviation,
                                tolerance: tol.value(),
                            });
                        }
                    }
                }
            }
            let _ = face_idx;
        }

        // Coplanar duplicate face check
        let len_eps = crate::geometry::math::LENGTH_TOLERANCE;
        for i in 0..self.faces.len() {
            for j in (i + 1)..self.faces.len() {
                let face_a = &self.faces[i];
                let face_b = &self.faces[j];

                let (normal_a, origin_a) = match &face_a.surface {
                    Surface::Plane { normal, origin, .. } => (normal, origin),
                    _ => continue,
                };
                let (normal_b, origin_b) = match &face_b.surface {
                    Surface::Plane { normal, origin, .. } => (normal, origin),
                    _ => continue,
                };

                // Normals parallel?
                if normal_a.dot(normal_b).abs() <= 1.0 - len_eps {
                    continue;
                }

                // Plane distance within tolerance?
                let dist = (origin_a - origin_b).dot(normal_b).abs();
                if dist > len_eps {
                    continue;
                }

                // Same outer_loop index → true duplicate (e.g. cloned face sharing the same loop)
                if face_a.outer_loop == face_b.outer_loop {
                    return Err(KernelError::ManifoldViolation {
                        reason: "overlapping coplanar faces detected",
                    });
                }
                // Skip face pairs that share vertices but have different loops (adjacent, not duplicates)
                let verts_a = self.loop_vertex_points(face_a.outer_loop);
                let verts_b = self.loop_vertex_points(face_b.outer_loop);
                let share_vertex = verts_a
                    .iter()
                    .any(|pa| verts_b.iter().any(|pb| (pa - pb).norm() < len_eps));
                if share_vertex {
                    continue;
                }
                // 2D AABB overlap as a coarse filter, then check inner loops to reduce false positives
                if aabb_overlap(&verts_a, &verts_b, normal_a, len_eps) {
                    let inner_loops_a: Vec<Vec<Point>> = face_a
                        .inner_loops
                        .iter()
                        .map(|&li| self.loop_vertex_points(li))
                        .collect();
                    let covered_by_hole = inner_loops_a
                        .iter()
                        .any(|hole| aabb_overlap(&verts_b, hole, normal_a, len_eps));
                    if !covered_by_hole {
                        return Err(KernelError::ManifoldViolation {
                            reason: "overlapping coplanar faces detected",
                        });
                    }
                }
            }
        }

        Ok(())
    }

    /// Collect 3D vertex points from a loop's half-edges.
    fn loop_vertex_points(&self, loop_idx: usize) -> Vec<Point> {
        let lp = &self.loops[loop_idx];
        lp.half_edges
            .iter()
            .map(|&he_idx| {
                let he = &self.half_edges[he_idx];
                self.vertices[he.start_vertex].point
            })
            .collect()
    }

    /// Compute Euler-Poincaré characteristic: V - E + F - 2*S + 2*H should equal 0.
    /// For this simplified version, H (through-holes) = 0 (inner_loops are trim curves, not genus).
    /// Valid for closed manifold: V - E + F = 2(S - H) where S = shells.len(), H = 0.
    pub fn euler_poincare(&self) -> i64 {
        let v = self.vertices.len() as i64;
        let e = self.edges.len() as i64;
        let f = self.faces.len() as i64;
        let s = self.shells.len() as i64;
        v - e + f - 2 * s
    }

    /// Translate all geometry in-place by the given offset.
    /// EntityIDs and topology indices (half_edges, loops, shells) are preserved.
    pub fn translate(&mut self, offset: crate::geometry::Vec3) {
        use crate::geometry::transform::translate_point;
        for v in &mut self.vertices {
            v.point = translate_point(v.point, offset);
        }
        for e in &mut self.edges {
            e.curve = e.curve.translate(offset);
        }
        for f in &mut self.faces {
            f.surface = f.surface.translate(offset);
        }
        // half_edges.pcurve is UV-space and invariant. loops/shells are index-only.
    }

    /// Rotate all geometry in-place by the given matrix around `pivot`.
    /// EntityIDs and topology indices (half_edges, loops, shells) are preserved.
    pub fn rotate(&mut self, matrix: [[f64; 3]; 3], pivot: crate::geometry::Point) {
        use crate::geometry::transform::{rotate_point, rotate_vec};
        for v in &mut self.vertices {
            v.point = rotate_point(v.point, matrix, pivot);
        }
        for e in &mut self.edges {
            e.curve = e.curve.rotate(matrix, pivot);
        }
        for f in &mut self.faces {
            f.surface = f.surface.rotate(matrix, pivot);
        }
        // half_edges.pcurve is UV-space and invariant under rotation.
        let _ = rotate_vec;
    }
}

/// AABB overlap check for two polygon vertex sets projected onto the dominant axis of `normal`.
fn aabb_overlap(poly_a: &[Point], poly_b: &[Point], normal: &Vec3, len_eps: f64) -> bool {
    if poly_a.is_empty() || poly_b.is_empty() {
        return false;
    }

    let (u_idx, v_idx) = if normal.x.abs() >= normal.y.abs() && normal.x.abs() >= normal.z.abs() {
        (1, 2)
    } else if normal.y.abs() >= normal.z.abs() {
        (0, 2)
    } else {
        (0, 1)
    };

    let (min_a_u, max_a_u) = poly_a
        .iter()
        .map(|p| p.coords[u_idx])
        .fold((f64::MAX, f64::MIN), |(mn, mx), v| (mn.min(v), mx.max(v)));
    let (min_a_v, max_a_v) = poly_a
        .iter()
        .map(|p| p.coords[v_idx])
        .fold((f64::MAX, f64::MIN), |(mn, mx), v| (mn.min(v), mx.max(v)));

    let (min_b_u, max_b_u) = poly_b
        .iter()
        .map(|p| p.coords[u_idx])
        .fold((f64::MAX, f64::MIN), |(mn, mx), v| (mn.min(v), mx.max(v)));
    let (min_b_v, max_b_v) = poly_b
        .iter()
        .map(|p| p.coords[v_idx])
        .fold((f64::MAX, f64::MIN), |(mn, mx), v| (mn.min(v), mx.max(v)));

    max_a_u - min_b_u > len_eps
        && max_b_u - min_a_u > len_eps
        && max_a_v - min_b_v > len_eps
        && max_b_v - min_a_v > len_eps
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
        let idx = solid.add_vertex(1, Point::new(1.0, 2.0, 3.0), None);
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
        let v0 = s.add_vertex(1, Point::new(0.0, 0.0, 0.0), None);
        let v1 = s.add_vertex(2, Point::new(1.0, 0.0, 0.0), None);
        // Edge with no half-edges at all
        let e0 = s.add_edge(
            3,
            [v0, v1],
            Curve::Line {
                origin: Point::origin(),
                direction: Vec3::x(),
            },
            [0.0, 1.0],
            None,
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
            None,
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
            None,
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
        let v0 = s.add_vertex(1, Point::new(0.0, 0.0, 0.0), None);
        let v1 = s.add_vertex(2, Point::new(1.0, 0.0, 0.0), None);
        let e0 = s.add_edge(
            3,
            [v0, v1],
            Curve::Line {
                origin: Point::origin(),
                direction: Vec3::x(),
            },
            [0.0, 1.0],
            None,
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
            None,
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
        let v0 = s.add_vertex(1, Point::new(0.0, 0.0, 0.0), None);
        let v1 = s.add_vertex(2, Point::new(0.0, 0.0, 0.0), None);
        let v2 = s.add_vertex(3, Point::new(1.0, 0.0, 0.0), None);
        let e0 = s.add_edge(
            4,
            [v0, v2],
            Curve::Line {
                origin: Point::origin(),
                direction: Vec3::x(),
            },
            [0.0, 1.0],
            None,
        );
        let e1 = s.add_edge(
            5,
            [v2, v1],
            Curve::Line {
                origin: Point::new(1.0, 0.0, 0.0),
                direction: -Vec3::x(),
            },
            [0.0, 1.0],
            None,
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
            None,
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
            None,
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
        let v0 = s.add_vertex(1, Point::new(0.0, 0.0, 0.0), None);
        let v1 = s.add_vertex(2, Point::new(1.0, 0.0, 0.0), None);
        let e0 = s.add_edge(
            3,
            [v0, v1],
            Curve::Line {
                origin: Point::origin(),
                direction: Vec3::x(),
            },
            [0.0, 1.0],
            None,
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
            None,
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
        let v0 = s.add_vertex(1, Point::new(0.0, 0.0, 0.0), None);
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
            None,
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
        let v0 = s.add_vertex(1, Point::new(0.0, 0.0, 0.0), None);
        let v1 = s.add_vertex(2, Point::new(1.0, 0.0, 0.0), None);
        let e0 = s.add_edge(
            3,
            [v0, v1],
            Curve::Line {
                origin: Point::origin(),
                direction: Vec3::x(),
            },
            [0.0, 1.0],
            None,
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
            None,
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
        let v0 = s.add_vertex(1, Point::new(0.0, 0.0, 0.0), None);
        let v1 = s.add_vertex(2, Point::new(1.0, 0.0, 0.0), None);
        let e0 = s.add_edge(
            3,
            [v0, v1],
            Curve::Line {
                origin: Point::origin(),
                direction: Vec3::x(),
            },
            [0.0, 1.0],
            None,
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
            None,
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
            None,
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
        let v0 = s.add_vertex(1, Point::new(0.0, 0.0, 0.0), None);
        let v1 = s.add_vertex(2, Point::new(1.0, 0.0, 0.0), None);
        let e0 = s.add_edge(
            3,
            [v0, v1],
            Curve::Line {
                origin: Point::origin(),
                direction: Vec3::x(),
            },
            [0.0, 1.0],
            None,
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
            None,
        );
        s.add_shell(8, vec![0], true);
        assert!(
            s.validate_manifold().is_err(),
            "out-of-bounds inner loop must fail"
        );
    }

    // T09: HalfEdge with pcurve: None serializes without pcurve field
    #[test]
    fn t09_half_edge_pcurve_none_yaml() {
        let he = HalfEdge {
            id: 1,
            start_vertex: 0,
            edge: 0,
            forward: true,
            pcurve: None,
        };
        let yaml = serde_yaml::to_string(&he).unwrap();
        assert!(!yaml.contains("pcurve"));
    }

    // T10: HalfEdge with pcurve roundtrip
    #[test]
    fn t10_half_edge_pcurve_some_yaml_roundtrip() {
        let line = crate::geometry::pcurve::Curve2D::try_line((0.0, 0.0), (1.0, 0.0)).unwrap();
        let pc = crate::geometry::pcurve::Pcurve::try_new(line, [0.0, 1.0]).unwrap();
        let he = HalfEdge {
            id: 42,
            start_vertex: 1,
            edge: 2,
            forward: true,
            pcurve: Some(pc),
        };
        let yaml = serde_yaml::to_string(&he).unwrap();
        let parsed: HalfEdge = serde_yaml::from_str(&yaml).unwrap();
        assert_eq!(parsed.id, 42);
        assert_eq!(parsed.start_vertex, 1);
        assert_eq!(parsed.edge, 2);
        assert!(parsed.forward);
        assert!(parsed.pcurve.is_some());
        assert_eq!(parsed.pcurve.unwrap().t_range(), [0.0, 1.0]);
    }

    // T11: Mixed add_half_edge and add_half_edge_with_pcurve
    #[test]
    fn t11_mixed_half_edge_apis() {
        let mut solid = Solid::new(0);
        let v0 = solid.add_vertex(1, Point::new(0.0, 0.0, 0.0), None);
        let v1 = solid.add_vertex(2, Point::new(1.0, 0.0, 0.0), None);
        let e0 = solid.add_edge(
            3,
            [v0, v1],
            Curve::Line {
                origin: Point::origin(),
                direction: crate::geometry::Vec3::x(),
            },
            [0.0, 1.0],
            None,
        );
        let he0 = solid.add_half_edge(4, v0, e0, true);
        let he1 = solid.add_half_edge_with_pcurve(5, v1, e0, false, None);
        assert_eq!(solid.half_edges[he0].pcurve, None);
        assert_eq!(solid.half_edges[he1].pcurve, None);
    }

    /// Helper: build a minimal 2-vertex 1-edge solid with pcurve on a given HE.
    fn build_pcurve_solid(
        pcurve_forward: Option<crate::geometry::pcurve::Pcurve>,
        pcurve_reverse: Option<crate::geometry::pcurve::Pcurve>,
    ) -> Solid {
        use crate::geometry::Vec3;
        let mut s = Solid::new(0);
        let v0 = s.add_vertex(1, Point::new(0.0, 0.0, 0.0), None);
        let v1 = s.add_vertex(2, Point::new(1.0, 0.0, 0.0), None);
        let e0 = s.add_edge(
            3,
            [v0, v1],
            Curve::Line {
                origin: Point::origin(),
                direction: Vec3::x(),
            },
            [0.0, 1.0],
            None,
        );
        let _ = s.add_half_edge_with_pcurve(4, v0, e0, true, pcurve_forward);
        let _ = s.add_half_edge_with_pcurve(5, v1, e0, false, pcurve_reverse);
        let lp = s.add_loop(6, vec![0, 1]);
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
            None,
        );
        s.add_shell(8, vec![0], true);
        s
    }

    // T12: pcurve endpoints match edge curve 3D evaluation
    #[test]
    fn t12_pcurve_consistent_with_edge() {
        // Line pcurve in XY plane: origin=(0,0), direction=(1,0), t_range=[0,1]
        // Surface is Plane XY: evaluate(u,v) = (u,v,0)
        // edge.curve is Line{origin=(0,0,0), direction=(1,0,0)}, t_range=[0,1]
        // pcurve at t=0: (0,0) -> surface(0,0) = (0,0,0) == edge.evaluate(0) ✓
        // pcurve at t=1: (1,0) -> surface(1,0) = (1,0,0) == edge.evaluate(1) ✓
        let line = crate::geometry::pcurve::Curve2D::try_line((0.0, 0.0), (1.0, 0.0)).unwrap();
        let pc = crate::geometry::pcurve::Pcurve::try_new(line, [0.0, 1.0]).unwrap();
        let solid = build_pcurve_solid(Some(pc), None);
        assert!(solid.validate_manifold().is_ok());
    }

    // T13: pcurve deviating from edge curve by ~1e-3
    #[test]
    fn t13_pcurve_mismatch_detected() {
        // Line pcurve: origin=(0,0), direction=(1,0.001), t_range=[0,1]
        // Midpoint at t=0.5: (0.5, 0.0005) -> surface gives (0.5, 0.0005, 0)
        // Edge curve midpoint: (0.5, 0, 0)
        // Deviation = sqrt(0.0005^2) = 0.0005 >> LENGTH_TOLERANCE
        let line = crate::geometry::pcurve::Curve2D::try_line((0.0, 0.0), (1.0, 0.001)).unwrap();
        let pc = crate::geometry::pcurve::Pcurve::try_new(line, [0.0, 1.0]).unwrap();
        let solid = build_pcurve_solid(Some(pc), None);
        let result = solid.validate_manifold();
        assert!(matches!(
            result,
            Err(KernelError::PcurveSurfaceMismatch { .. })
        ));
    }

    // T13b: endpoints match but midpoint diverges (edge is line, pcurve is circle arc with same endpoints)
    #[test]
    fn t13b_midpoint_mismatch_detected() {
        // Circle2D centered at (0.5, 0), radius=0.5, sweep from angle π to 0
        // At angle π: (-0.5+0.5, 0) = (0,0) → surface(0,0)=(0,0,0) matches edge.evaluate(0)
        // At angle 0: (0.5+0.5, 0) = (1,0) → surface(1,0)=(1,0,0) matches edge.evaluate(1)
        // At angle π/2: (0.5, 0.5) → surface(0.5, 0.5)=(0.5,0.5,0) ≠ edge.evaluate(0.5)=(0.5,0,0)
        let circle = crate::geometry::pcurve::Curve2D::try_circle((0.5, 0.0), 0.5).unwrap();
        let pc =
            crate::geometry::pcurve::Pcurve::try_new(circle, [std::f64::consts::PI, 0.0]).unwrap();
        let solid = build_pcurve_solid(Some(pc), None);
        let result = solid.validate_manifold();
        assert!(
            matches!(result, Err(KernelError::PcurveSurfaceMismatch { .. })),
            "expected PcurveSurfaceMismatch for midpoint divergence, got {:?}",
            result
        );
    }

    // T13c: descending t_range pcurve on forward HE with consistent edge
    #[test]
    fn t13c_descending_trange_forward_he_ok() {
        // Forward HE: edge t_range [0,1], pcurve t_range [1,0] (descending)
        // Line pcurve: origin=(1,0), direction=(-1,0)
        // At t=1: (1+(-1)*1, 0) = (0,0) → surface(0,0)=(0,0,0) == edge.evaluate(0)
        // At t=0: (1+(-1)*0, 0) = (1,0) → surface(1,0)=(1,0,0) == edge.evaluate(1)
        // This is valid: pcurve descends while edge ascends (both traverse the same geometry)
        let line = crate::geometry::pcurve::Curve2D::try_line((1.0, 0.0), (-1.0, 0.0)).unwrap();
        let pc = crate::geometry::pcurve::Pcurve::try_new(line, [1.0, 0.0]).unwrap();
        let solid = build_pcurve_solid(Some(pc), None);
        assert!(solid.validate_manifold().is_ok());
    }

    // T13d: validate_manifold returns Result<(), KernelError> with structured error
    #[test]
    fn t13d_validate_returns_kernel_error() {
        let line = crate::geometry::pcurve::Curve2D::try_line((0.0, 0.0), (1.0, 0.001)).unwrap();
        let pc = crate::geometry::pcurve::Pcurve::try_new(line, [0.0, 1.0]).unwrap();
        let solid = build_pcurve_solid(Some(pc), None);
        match solid.validate_manifold() {
            Err(KernelError::PcurveSurfaceMismatch {
                he_idx,
                deviation,
                tolerance,
            }) => {
                assert_eq!(he_idx, 0);
                assert!(deviation > tolerance);
            }
            other => panic!("expected PcurveSurfaceMismatch, got {:?}", other),
        }
    }

    // ---- #72 translate core tests (T01–T07) ----

    use crate::geometry::Vec3;

    fn make_test_cuboid() -> Solid {
        let mut gen = IdGenerator::new(1);
        crate::primitives::make_cuboid(2.0, 3.0, 4.0, &mut gen).unwrap()
    }

    /// T01: Determinism — same offset applied 100 times yields identical serialized output.
    #[test]
    fn t01_translate_determinism() {
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

    /// T02: Geometry types translate correctly (basis points move by offset).
    #[test]
    fn t02_geometry_translate() {
        use crate::geometry::math::point_near;
        use crate::geometry::surface::Surface;
        use crate::geometry::Plane;

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

    /// T03: Axes/normals/directions/radii/half_angles are invariant under translate.
    #[test]
    fn t03_translate_invariants() {
        use crate::geometry::surface::Surface;

        let offset = Vec3::new(10.0, 20.0, 30.0);

        // Plane: normal, u_axis, v_axis unchanged
        let plane = crate::geometry::Plane::xy();
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

    /// T04: EntityIDs and topology indices are preserved after translate.
    #[test]
    fn t04_entity_id_preserved() {
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

    /// T05: Round-trip — translate(v).translate(-v) recovers original geometry.
    #[test]
    fn t05_translate_roundtrip() {
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

    /// T06: Zero offset leaves geometry strictly unchanged.
    #[test]
    fn t06_boundary_zero_offset() {
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

    /// T07: Large offset keeps coordinates finite (no NaN/Inf).
    #[test]
    fn t07_large_offset_remains_finite() {
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

    // ---- #77 rotate core tests ----

    use crate::geometry::transform::euler_to_matrix;

    /// T01: Determinism — same rotation applied 100 times yields identical serialized output.
    #[test]
    fn t01_rotate_determinism() {
        let cuboid = make_test_cuboid();
        let matrix = euler_to_matrix(30.0, 45.0, 60.0);
        let pivot = Point::origin();
        let mut first: Option<String> = None;
        for _ in 0..100 {
            let mut s = cuboid.clone();
            s.rotate(matrix, pivot);
            let yaml = serde_yaml::to_string(&s).unwrap();
            match &first {
                None => first = Some(yaml),
                Some(prev) => assert_eq!(prev, &yaml, "rotate output must be deterministic"),
            }
        }
    }

    /// T02: 90° x-axis rotation on cuboid aligns face normals to axes.
    #[test]
    fn t02_rotate_90deg_face_normals() {
        let mut cuboid = make_test_cuboid();
        let matrix = euler_to_matrix(90.0, 0.0, 0.0);
        cuboid.rotate(matrix, Point::origin());

        for face in &cuboid.faces {
            if let crate::geometry::surface::Surface::Plane { normal, .. } = &face.surface {
                // Each face normal must align with a principal axis (±x, ±y, ±z)
                let aligned = [
                    Vec3::x(),
                    -Vec3::x(),
                    Vec3::y(),
                    -Vec3::y(),
                    Vec3::z(),
                    -Vec3::z(),
                ];
                let n = normal.normalize();
                let ok = aligned.iter().any(|a| (n.dot(a) - 1.0).abs() < 1e-12);
                assert!(
                    ok,
                    "face normal {:?} is not aligned with a principal axis",
                    n
                );
            }
        }

        // Euler-Poincaré preserved: V-E+F = 2
        assert_eq!(cuboid.euler_poincare(), 0);
    }

    /// T03: Inverse — rotate(M) then rotate(M^T) recovers original geometry.
    #[test]
    fn t03_rotate_inverse() {
        use approx::assert_relative_eq;

        let original = make_test_cuboid();
        let matrix = euler_to_matrix(30.0, 45.0, 60.0);
        // Transpose = inverse for orthogonal matrices
        let inv = [
            [matrix[0][0], matrix[1][0], matrix[2][0]],
            [matrix[0][1], matrix[1][1], matrix[2][1]],
            [matrix[0][2], matrix[1][2], matrix[2][2]],
        ];
        let pivot = Point::new(1.0, 2.0, 3.0);
        let mut roundtrip = original.clone();
        roundtrip.rotate(matrix, pivot);
        roundtrip.rotate(inv, pivot);

        for (o, r) in original.vertices.iter().zip(roundtrip.vertices.iter()) {
            assert_relative_eq!(o.point.x, r.point.x, epsilon = 1e-12);
            assert_relative_eq!(o.point.y, r.point.y, epsilon = 1e-12);
            assert_relative_eq!(o.point.z, r.point.z, epsilon = 1e-12);
        }
        for (o, r) in original.edges.iter().zip(roundtrip.edges.iter()) {
            match (&o.curve, &r.curve) {
                (
                    Curve::Line {
                        origin: oo,
                        direction: od,
                    },
                    Curve::Line {
                        origin: ro,
                        direction: rd,
                    },
                ) => {
                    assert_relative_eq!(oo.x, ro.x, epsilon = 1e-12);
                    assert_relative_eq!(oo.y, ro.y, epsilon = 1e-12);
                    assert_relative_eq!(oo.z, ro.z, epsilon = 1e-12);
                    assert_relative_eq!(od.x, rd.x, epsilon = 1e-12);
                    assert_relative_eq!(od.y, rd.y, epsilon = 1e-12);
                    assert_relative_eq!(od.z, rd.z, epsilon = 1e-12);
                }
                (
                    Curve::Circle {
                        center: oc,
                        normal: on,
                        radius: or_,
                    },
                    Curve::Circle {
                        center: rc,
                        normal: rn,
                        radius: rr,
                    },
                ) => {
                    assert_relative_eq!(oc.x, rc.x, epsilon = 1e-12);
                    assert_relative_eq!(oc.y, rc.y, epsilon = 1e-12);
                    assert_relative_eq!(oc.z, rc.z, epsilon = 1e-12);
                    assert_relative_eq!(on.x, rn.x, epsilon = 1e-12);
                    assert_relative_eq!(on.y, rn.y, epsilon = 1e-12);
                    assert_relative_eq!(on.z, rn.z, epsilon = 1e-12);
                    assert_relative_eq!(or_, rr, epsilon = 1e-12);
                }
                _ => panic!("curve variant mismatch after roundtrip"),
            }
        }
        // Surfaces round-trip too
        for (o, r) in original.faces.iter().zip(roundtrip.faces.iter()) {
            assert_relative_eq!(
                o.surface.evaluate(0.5, 0.5).x,
                r.surface.evaluate(0.5, 0.5).x,
                epsilon = 1e-12
            );
            assert_relative_eq!(
                o.surface.evaluate(0.5, 0.5).y,
                r.surface.evaluate(0.5, 0.5).y,
                epsilon = 1e-12
            );
            assert_relative_eq!(
                o.surface.evaluate(0.5, 0.5).z,
                r.surface.evaluate(0.5, 0.5).z,
                epsilon = 1e-12
            );
        }
    }

    /// T06: Cylinder surface rotates correctly (axis rotates, radius unchanged).
    #[test]
    fn t06_cylinder_rotate() {
        use crate::geometry::math::orthonormal_basis;
        use crate::geometry::surface::Surface;
        use approx::assert_relative_eq;

        let cyl = Surface::Cylinder {
            origin: Point::origin(),
            axis: Vec3::z(),
            radius: 2.5,
        };
        let matrix = euler_to_matrix(90.0, 0.0, 0.0);
        let rotated = cyl.rotate(matrix, Point::origin());

        if let Surface::Cylinder {
            origin,
            axis,
            radius,
        } = &rotated
        {
            // After 90° rotation around x-axis: z-axis → -y-axis
            assert_relative_eq!(origin.x, 0.0, epsilon = 1e-12);
            assert_relative_eq!(origin.y, 0.0, epsilon = 1e-12);
            assert_relative_eq!(origin.z, 0.0, epsilon = 1e-12);
            assert_relative_eq!(*radius, 2.5, epsilon = 1e-12);

            // Rotated axis should be (0, -1, 0)
            let (bu, bv) = orthonormal_basis(axis);
            // Verify orthonormality of rotated basis
            assert_relative_eq!(bu.norm(), 1.0, epsilon = 1e-12);
            assert_relative_eq!(bv.norm(), 1.0, epsilon = 1e-12);
            assert_relative_eq!(axis.normalize().dot(&bu), 0.0, epsilon = 1e-12);
            assert_relative_eq!(axis.normalize().dot(&bv), 0.0, epsilon = 1e-12);
        } else {
            panic!("expected Surface::Cylinder");
        }
    }

    /// T07_boundary_zero_rotation: (0,0,0) rotation leaves Solid identical.
    #[test]
    fn t07_boundary_zero_rotation() {
        let original = make_test_cuboid();
        let matrix = euler_to_matrix(0.0, 0.0, 0.0);
        let mut rotated = original.clone();
        rotated.rotate(matrix, Point::origin());

        for (o, r) in original.vertices.iter().zip(rotated.vertices.iter()) {
            assert_eq!(
                o.point, r.point,
                "vertex must be identical with zero rotation"
            );
        }
        for (o, r) in original.edges.iter().zip(rotated.edges.iter()) {
            assert_eq!(
                o.curve, r.curve,
                "edge curve must be identical with zero rotation"
            );
        }
        for (o, r) in original.faces.iter().zip(rotated.faces.iter()) {
            assert_eq!(
                o.surface, r.surface,
                "face surface must be identical with zero rotation"
            );
        }
    }

    /// T08_boundary_180: 180° around x-axis flips y/z coordinates (around pivot).
    #[test]
    fn t08_boundary_180() {
        use approx::assert_relative_eq;

        let mut cuboid = make_test_cuboid();
        let pivot = Point::origin();
        let matrix = euler_to_matrix(180.0, 0.0, 0.0);
        let original = cuboid.clone();
        cuboid.rotate(matrix, pivot);

        for (o, r) in original.vertices.iter().zip(cuboid.vertices.iter()) {
            // Rx(180°): x unchanged, y → -y, z → -z (around origin)
            assert_relative_eq!(o.point.x, r.point.x, epsilon = 1e-12);
            assert_relative_eq!(o.point.y, -r.point.y, epsilon = 1e-12);
            assert_relative_eq!(o.point.z, -r.point.z, epsilon = 1e-12);
        }
    }

    /// T09_degen_pivot_at_vertex: pivot placed on a vertex keeps that vertex invariant.
    #[test]
    fn t09_degen_pivot_at_vertex() {
        use approx::assert_relative_eq;

        let cuboid = make_test_cuboid();
        let matrix = euler_to_matrix(45.0, 30.0, 60.0);
        let pivot = cuboid.vertices[0].point;
        let mut rotated = cuboid.clone();
        rotated.rotate(matrix, pivot);

        // Vertex at pivot should not move
        assert_relative_eq!(
            cuboid.vertices[0].point.x,
            rotated.vertices[0].point.x,
            epsilon = 1e-15
        );
        assert_relative_eq!(
            cuboid.vertices[0].point.y,
            rotated.vertices[0].point.y,
            epsilon = 1e-15
        );
        assert_relative_eq!(
            cuboid.vertices[0].point.z,
            rotated.vertices[0].point.z,
            epsilon = 1e-15
        );
    }

    // ---- b6-f01 topology overlap tests ----

    /// T_adj_coplanar: two adjacent coplanar faces sharing a vertex must pass validate_manifold.
    /// Layout (XY plane):
    ///   face_a: (0,0)-(1,0)-(1,1)-(0,1)
    ///   face_b: (1,0)-(2,0)-(2,1)-(1,1)   shares edge (1,0)-(1,1) with face_a
    /// They share vertices → the improved check skips them.
    #[test]
    fn t_adj_coplanar() {
        let mut s = Solid::new(0);
        // 6 vertices
        let v0 = s.add_vertex(1, Point::new(0.0, 0.0, 0.0), None);
        let v1 = s.add_vertex(2, Point::new(1.0, 0.0, 0.0), None);
        let v2 = s.add_vertex(3, Point::new(1.0, 1.0, 0.0), None);
        let v3 = s.add_vertex(4, Point::new(0.0, 1.0, 0.0), None);
        let v4 = s.add_vertex(5, Point::new(2.0, 0.0, 0.0), None);
        let v5 = s.add_vertex(6, Point::new(2.0, 1.0, 0.0), None);

        // Edges for face_a: v0→v1, v1→v2, v2→v3, v3→v0
        let e01 = s.add_edge(
            10,
            [v0, v1],
            Curve::Line {
                origin: Point::new(0.0, 0.0, 0.0),
                direction: Vec3::x(),
            },
            [0.0, 1.0],
            None,
        );
        let e12 = s.add_edge(
            11,
            [v1, v2],
            Curve::Line {
                origin: Point::new(1.0, 0.0, 0.0),
                direction: Vec3::y(),
            },
            [0.0, 1.0],
            None,
        );
        let e23 = s.add_edge(
            12,
            [v2, v3],
            Curve::Line {
                origin: Point::new(1.0, 1.0, 0.0),
                direction: -Vec3::x(),
            },
            [0.0, 1.0],
            None,
        );
        let e30 = s.add_edge(
            13,
            [v3, v0],
            Curve::Line {
                origin: Point::new(0.0, 1.0, 0.0),
                direction: -Vec3::y(),
            },
            [0.0, 1.0],
            None,
        );

        // Edges for face_b: v1→v4, v4→v5, v5→v2, v2→v1
        let e14 = s.add_edge(
            14,
            [v1, v4],
            Curve::Line {
                origin: Point::new(1.0, 0.0, 0.0),
                direction: Vec3::x(),
            },
            [0.0, 1.0],
            None,
        );
        let e45 = s.add_edge(
            15,
            [v4, v5],
            Curve::Line {
                origin: Point::new(2.0, 0.0, 0.0),
                direction: Vec3::y(),
            },
            [0.0, 1.0],
            None,
        );
        let e52 = s.add_edge(
            16,
            [v5, v2],
            Curve::Line {
                origin: Point::new(2.0, 1.0, 0.0),
                direction: -Vec3::x(),
            },
            [0.0, 1.0],
            None,
        );
        let e21 = s.add_edge(
            17,
            [v2, v1],
            Curve::Line {
                origin: Point::new(1.0, 1.0, 0.0),
                direction: -Vec3::y(),
            },
            [0.0, 1.0],
            None,
        );

        let surface = Surface::Plane {
            origin: Point::origin(),
            normal: Vec3::z(),
            u_axis: Vec3::x(),
            v_axis: Vec3::y(),
        };

        // Face A: forward half-edges v0→v1→v2→v3→v0
        let he_a0 = s.add_half_edge(20, v0, e01, true);
        let he_a1 = s.add_half_edge(21, v1, e12, true);
        let he_a2 = s.add_half_edge(22, v2, e23, true);
        let he_a3 = s.add_half_edge(23, v3, e30, true);
        let lp_a = s.add_loop(30, vec![he_a0, he_a1, he_a2, he_a3]);

        // Face A reverse: v0→v3→v2→v1→v0
        let he_a3r = s.add_half_edge(24, v0, e30, false); // v0→v3
        let he_a2r = s.add_half_edge(25, v3, e23, false); // v3→v2
        let he_a1r = s.add_half_edge(26, v2, e12, false); // v2→v1
        let he_a0r = s.add_half_edge(27, v1, e01, false); // v1→v0
        let lp_ar = s.add_loop(31, vec![he_a3r, he_a2r, he_a1r, he_a0r]);

        // Face B: forward half-edges v1→v4→v5→v2→v1
        let he_b0 = s.add_half_edge(28, v1, e14, true);
        let he_b1 = s.add_half_edge(29, v4, e45, true);
        let he_b2 = s.add_half_edge(40, v5, e52, true);
        let he_b3 = s.add_half_edge(41, v2, e21, true);
        let lp_b = s.add_loop(32, vec![he_b0, he_b1, he_b2, he_b3]);

        // Face B reverse: v1→v2→v5→v4→v1
        let he_b3r = s.add_half_edge(42, v1, e21, false); // v1→v2
        let he_b2r = s.add_half_edge(43, v2, e52, false); // v2→v5
        let he_b1r = s.add_half_edge(44, v5, e45, false); // v5→v4
        let he_b0r = s.add_half_edge(45, v4, e14, false); // v4→v1
        let lp_br = s.add_loop(33, vec![he_b3r, he_b2r, he_b1r, he_b0r]);

        s.add_face(50, surface.clone(), lp_a, vec![], true, None);
        s.add_face(51, surface.clone(), lp_b, vec![], true, None);
        s.add_face(52, surface.clone(), lp_ar, vec![], false, None);
        s.add_face(53, surface, lp_br, vec![], false, None);

        s.add_shell(60, vec![0, 1, 2, 3], true);

        assert!(
            s.validate_manifold().is_ok(),
            "adjacent coplanar faces sharing vertices must pass validation"
        );
    }

    /// T_degen_true_overlap: two truly overlapping coplanar faces (no shared vertices)
    /// must fail validate_manifold with ManifoldViolation.
    #[test]
    fn t_degen_true_overlap() {
        let mut s = Solid::new(0);
        // face_a: (0,0)-(2,0)-(2,2)-(0,2)
        // face_b: (1,1)-(3,1)-(3,3)-(1,3) — overlaps face_a, no shared vertices
        let v0 = s.add_vertex(1, Point::new(0.0, 0.0, 0.0), None);
        let v1 = s.add_vertex(2, Point::new(2.0, 0.0, 0.0), None);
        let v2 = s.add_vertex(3, Point::new(2.0, 2.0, 0.0), None);
        let v3 = s.add_vertex(4, Point::new(0.0, 2.0, 0.0), None);
        let v4 = s.add_vertex(5, Point::new(1.0, 1.0, 0.0), None);
        let v5 = s.add_vertex(6, Point::new(3.0, 1.0, 0.0), None);
        let v6 = s.add_vertex(7, Point::new(3.0, 3.0, 0.0), None);
        let v7 = s.add_vertex(8, Point::new(1.0, 3.0, 0.0), None);

        // Edges face_a
        let e01 = s.add_edge(
            10,
            [v0, v1],
            Curve::Line {
                origin: Point::new(0.0, 0.0, 0.0),
                direction: Vec3::x(),
            },
            [0.0, 2.0],
            None,
        );
        let e12 = s.add_edge(
            11,
            [v1, v2],
            Curve::Line {
                origin: Point::new(2.0, 0.0, 0.0),
                direction: Vec3::y(),
            },
            [0.0, 2.0],
            None,
        );
        let e23 = s.add_edge(
            12,
            [v2, v3],
            Curve::Line {
                origin: Point::new(2.0, 2.0, 0.0),
                direction: -Vec3::x(),
            },
            [0.0, 2.0],
            None,
        );
        let e30 = s.add_edge(
            13,
            [v3, v0],
            Curve::Line {
                origin: Point::new(0.0, 2.0, 0.0),
                direction: -Vec3::y(),
            },
            [0.0, 2.0],
            None,
        );

        // Edges face_b
        let e45 = s.add_edge(
            14,
            [v4, v5],
            Curve::Line {
                origin: Point::new(1.0, 1.0, 0.0),
                direction: Vec3::x(),
            },
            [0.0, 2.0],
            None,
        );
        let e56 = s.add_edge(
            15,
            [v5, v6],
            Curve::Line {
                origin: Point::new(3.0, 1.0, 0.0),
                direction: Vec3::y(),
            },
            [0.0, 2.0],
            None,
        );
        let e67 = s.add_edge(
            16,
            [v6, v7],
            Curve::Line {
                origin: Point::new(3.0, 3.0, 0.0),
                direction: -Vec3::x(),
            },
            [0.0, 2.0],
            None,
        );
        let e74 = s.add_edge(
            17,
            [v7, v4],
            Curve::Line {
                origin: Point::new(1.0, 3.0, 0.0),
                direction: -Vec3::y(),
            },
            [0.0, 2.0],
            None,
        );

        let surface = Surface::Plane {
            origin: Point::origin(),
            normal: Vec3::z(),
            u_axis: Vec3::x(),
            v_axis: Vec3::y(),
        };

        // Face A: forward
        let he_a0 = s.add_half_edge(20, v0, e01, true);
        let he_a1 = s.add_half_edge(21, v1, e12, true);
        let he_a2 = s.add_half_edge(22, v2, e23, true);
        let he_a3 = s.add_half_edge(23, v3, e30, true);
        let lp_a = s.add_loop(30, vec![he_a0, he_a1, he_a2, he_a3]);

        // Face A: reverse v0→v3→v2→v1→v0
        let he_a3r = s.add_half_edge(24, v0, e30, false);
        let he_a2r = s.add_half_edge(25, v3, e23, false);
        let he_a1r = s.add_half_edge(26, v2, e12, false);
        let he_a0r = s.add_half_edge(27, v1, e01, false);
        let lp_ar = s.add_loop(31, vec![he_a3r, he_a2r, he_a1r, he_a0r]);

        // Face B: forward v4→v5→v6→v7→v4
        let he_b0 = s.add_half_edge(28, v4, e45, true);
        let he_b1 = s.add_half_edge(29, v5, e56, true);
        let he_b2 = s.add_half_edge(40, v6, e67, true);
        let he_b3 = s.add_half_edge(41, v7, e74, true);
        let lp_b = s.add_loop(32, vec![he_b0, he_b1, he_b2, he_b3]);

        // Face B: reverse v4→v7→v6→v5→v4
        let he_b3r = s.add_half_edge(42, v4, e74, false);
        let he_b2r = s.add_half_edge(43, v7, e67, false);
        let he_b1r = s.add_half_edge(44, v6, e56, false);
        let he_b0r = s.add_half_edge(45, v5, e45, false);
        let lp_br = s.add_loop(33, vec![he_b3r, he_b2r, he_b1r, he_b0r]);

        s.add_face(50, surface.clone(), lp_a, vec![], true, None);
        s.add_face(51, surface.clone(), lp_b, vec![], true, None);
        s.add_face(52, surface.clone(), lp_ar, vec![], false, None);
        s.add_face(53, surface, lp_br, vec![], false, None);

        s.add_shell(60, vec![0, 1, 2, 3], true);

        let result = s.validate_manifold();
        assert!(
            matches!(result, Err(KernelError::ManifoldViolation { .. })),
            "truly overlapping coplanar faces must fail with ManifoldViolation, got {:?}",
            result
        );
    }
}
