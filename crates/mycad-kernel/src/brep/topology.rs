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
}
