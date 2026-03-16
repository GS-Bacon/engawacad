use crate::brep::topology::Solid;
use crate::geometry::Point;
use serde::{Deserialize, Serialize};

/// A triangle mesh for rendering.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TriangleMesh {
    /// Vertex positions (x, y, z).
    pub positions: Vec<[f64; 3]>,
    /// Normal vectors per vertex.
    pub normals: Vec<[f64; 3]>,
    /// Triangle indices (every 3 indices form a triangle).
    pub indices: Vec<u32>,
}

impl TriangleMesh {
    pub fn new() -> Self {
        Self {
            positions: Vec::new(),
            normals: Vec::new(),
            indices: Vec::new(),
        }
    }

    /// Number of triangles in the mesh.
    pub fn triangle_count(&self) -> usize {
        self.indices.len() / 3
    }
}

impl Default for TriangleMesh {
    fn default() -> Self {
        Self::new()
    }
}

/// Tessellate a B-rep solid into a triangle mesh.
///
/// For now this handles planar faces only by triangulating each face's
/// outer loop as a triangle fan from the first vertex.
pub fn tessellate_solid(solid: &Solid) -> TriangleMesh {
    let mut mesh = TriangleMesh::new();

    for face in &solid.faces {
        let outer_loop = &solid.loops[face.outer_loop];

        // Collect the vertices of this face's outer loop
        let loop_vertices: Vec<&Point> = outer_loop
            .half_edges
            .iter()
            .map(|&he_idx| {
                let he = &solid.half_edges[he_idx];
                &solid.vertices[he.start_vertex].point
            })
            .collect();

        if loop_vertices.len() < 3 {
            continue;
        }

        // Compute face normal from the surface
        let normal = face.surface.normal_at(0.0, 0.0);
        let face_normal = if face.same_sense {
            [normal.x, normal.y, normal.z]
        } else {
            [-normal.x, -normal.y, -normal.z]
        };

        // Add vertices and triangulate as a fan from vertex 0
        let base_idx = mesh.positions.len() as u32;

        for &v in &loop_vertices {
            mesh.positions.push([v.x, v.y, v.z]);
            mesh.normals.push(face_normal);
        }

        for i in 1..(loop_vertices.len() as u32 - 1) {
            mesh.indices.push(base_idx);
            mesh.indices.push(base_idx + i);
            mesh.indices.push(base_idx + i + 1);
        }
    }

    mesh
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::brep::topology::IdGenerator;
    use crate::primitives::make_cuboid;

    #[test]
    fn test_tessellate_cuboid() {
        let mut id_gen = IdGenerator::new(0);
        let solid = make_cuboid(1.0, 1.0, 1.0, &mut id_gen);
        let mesh = tessellate_solid(&solid);

        // A cuboid has 6 faces, each tessellated as 2 triangles = 12 triangles
        assert_eq!(mesh.triangle_count(), 12);
        // 6 faces * 4 vertices = 24 vertices (not shared across faces for correct normals)
        assert_eq!(mesh.positions.len(), 24);
        assert_eq!(mesh.normals.len(), 24);
        assert_eq!(mesh.indices.len(), 36);
    }

    #[test]
    fn test_tessellation_deterministic() {
        let mut id_gen1 = IdGenerator::new(0);
        let mut id_gen2 = IdGenerator::new(0);

        let solid1 = make_cuboid(2.0, 3.0, 4.0, &mut id_gen1);
        let solid2 = make_cuboid(2.0, 3.0, 4.0, &mut id_gen2);

        let mesh1 = tessellate_solid(&solid1);
        let mesh2 = tessellate_solid(&solid2);

        assert_eq!(mesh1.positions, mesh2.positions);
        assert_eq!(mesh1.normals, mesh2.normals);
        assert_eq!(mesh1.indices, mesh2.indices);
    }
}
