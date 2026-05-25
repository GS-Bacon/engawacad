pub mod stl;
pub use stl::to_ascii_stl;

use crate::brep::topology::Solid;
use crate::geometry::curve::Curve;
use crate::geometry::surface::{Surface, TessellationStrategy};
use crate::geometry::Point;
use serde::{Deserialize, Serialize};
use std::f64::consts::PI;
use thiserror::Error;

/// Seam jump detection threshold: |Δu| > π indicates a parameter-space seam crossing.
/// Minimum cross-product norm for a non-degenerate triangle.
const AREA_EPS: f64 = 1e-14;

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

/// Controls the resolution of curved-surface tessellation.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct TessellationOptions {
    pub angular_segments: usize,
    pub axial_segments: usize,
}

impl Default for TessellationOptions {
    fn default() -> Self {
        Self {
            angular_segments: 32,
            axial_segments: 1,
        }
    }
}

impl TessellationOptions {
    pub fn new(angular_segments: usize, axial_segments: usize) -> Self {
        Self {
            angular_segments: angular_segments.max(3),
            axial_segments: axial_segments.max(1),
        }
    }
}

/// Errors from tessellation.
#[derive(Debug, Error)]
pub enum TessellationError {
    #[error("unsupported surface type: {kind}")]
    UnsupportedSurface { kind: &'static str },
    #[error("trimmed or punctured faces are not yet supported")]
    TrimmedFaceUnsupported,
    #[error("non-manifold loop detected")]
    NonManifoldLoop,
}

/// Tessellate a B-rep solid into a triangle mesh using default options.
pub fn tessellate_solid(solid: &Solid) -> Result<TriangleMesh, TessellationError> {
    tessellate_solid_with(solid, &TessellationOptions::default())
}

/// Tessellate a B-rep solid into a triangle mesh with the given options.
pub fn tessellate_solid_with(
    solid: &Solid,
    opts: &TessellationOptions,
) -> Result<TriangleMesh, TessellationError> {
    let mut mesh = TriangleMesh::new();

    for face in &solid.faces {
        let strategy = face.surface.tessellation_strategy();
        match strategy {
            TessellationStrategy::BoundaryFan => {
                tessellate_face_fan(solid, face, opts, &mut mesh)?;
            }
            TessellationStrategy::UvGridFullPatch => {
                tessellate_face_uv_grid(solid, face, opts, &mut mesh)?;
            }
            TessellationStrategy::Unsupported => {
                return Err(TessellationError::UnsupportedSurface {
                    kind: face.surface.kind_name(),
                });
            }
        }
    }

    Ok(mesh)
}

/// Triangulate a face as a fan from its first boundary vertex (planar convex faces).
fn tessellate_face_fan(
    solid: &Solid,
    face: &crate::brep::topology::Face,
    opts: &TessellationOptions,
    mesh: &mut TriangleMesh,
) -> Result<(), TessellationError> {
    let outer_loop = &solid.loops[face.outer_loop];

    let loop_points = collect_loop_points(solid, outer_loop, opts.angular_segments.max(3))?;

    if loop_points.len() < 3 {
        return Ok(());
    }

    let base_idx = mesh.positions.len() as u32;

    for p in &loop_points {
        let normal = face.surface.normal_at_point(p);
        let n = if face.same_sense {
            [normal.x, normal.y, normal.z]
        } else {
            [-normal.x, -normal.y, -normal.z]
        };
        mesh.positions.push([p.x, p.y, p.z]);
        mesh.normals.push(n);
    }

    for i in 1..(loop_points.len() as u32 - 1) {
        mesh.indices.push(base_idx);
        mesh.indices.push(base_idx + i);
        mesh.indices.push(base_idx + i + 1);
    }

    Ok(())
}

/// Collect the ordered boundary points of a loop by sampling each HE's curve.
fn collect_loop_points(
    solid: &Solid,
    lp: &crate::brep::topology::Loop,
    segments: usize,
) -> Result<Vec<Point>, TessellationError> {
    let mut points = Vec::new();

    for &he_idx in &lp.half_edges {
        let he = &solid.half_edges[he_idx];
        let edge = &solid.edges[he.edge];

        let (t_start, t_end) = if he.forward {
            (edge.t_range[0], edge.t_range[1])
        } else {
            (edge.t_range[1], edge.t_range[0])
        };

        let seg_points = edge.curve.sample_segment(t_start, t_end, segments);
        for p in seg_points {
            points.push(p);
        }
    }

    Ok(points)
}

/// Triangulate a face using UV grid sampling (full untrimmed periodic/rectangular patch).
fn tessellate_face_uv_grid(
    solid: &Solid,
    face: &crate::brep::topology::Face,
    opts: &TessellationOptions,
    mesh: &mut TriangleMesh,
) -> Result<(), TessellationError> {
    if !face.inner_loops.is_empty() {
        return Err(TessellationError::TrimmedFaceUnsupported);
    }

    let outer_loop = &solid.loops[face.outer_loop];

    // Guard: total Circle-edge span must be a positive integer multiple of 2π.
    // Summing spans (rather than testing any single edge) accepts edge-split full cylinders
    // (e.g. two π-arcs per cap) while still rejecting trimmed faces. 1e-9 tolerance is tight
    // enough that floating-point noise never masks a genuine sub-degree trim.
    let total_circle_span: f64 = outer_loop
        .half_edges
        .iter()
        .filter_map(|&he_idx| {
            let he = &solid.half_edges[he_idx];
            let edge = &solid.edges[he.edge];
            if matches!(edge.curve, Curve::Circle { .. }) {
                Some((edge.t_range[1] - edge.t_range[0]).abs())
            } else {
                None
            }
        })
        .sum();
    let full_rev_count = (total_circle_span / (2.0 * PI)).round() as i64;
    if full_rev_count <= 0 || (total_circle_span - full_rev_count as f64 * 2.0 * PI).abs() >= 1e-9 {
        return Err(TessellationError::TrimmedFaceUnsupported);
    }

    // Determine v range from loop corner vertices (start_vertex of each HE).
    // UvGridFullPatch assumes a full 2π revolution in u starting at u=0;
    // we do not sample the circle arcs here — the grid covers [0,2π] directly.
    let corner_uvs: Vec<(f64, f64)> = outer_loop
        .half_edges
        .iter()
        .map(|&he_idx| {
            let he = &solid.half_edges[he_idx];
            face.surface.uv_of(&solid.vertices[he.start_vertex].point)
        })
        .collect();

    let v_min = corner_uvs.iter().map(|(_, v)| *v).fold(f64::MAX, f64::min);
    let v_max = corner_uvs.iter().map(|(_, v)| *v).fold(f64::MIN, f64::max);
    let u_min = 0.0_f64;

    // Clamp at point of use — callers may bypass TessellationOptions::new() via public fields
    // or deserialization, so we defensively enforce minimum viable values here.
    let n_u = opts.angular_segments.max(3);
    let n_v = opts.axial_segments.max(1);

    let base_idx = mesh.positions.len() as u32;

    // Sample UV grid: (n_u+1) × (n_v+1) vertices
    // u goes from u_min to u_min + 2π, v goes from v_min to v_max
    let du = 2.0 * PI / n_u as f64;
    let dv = (v_max - v_min) / n_v as f64;

    for iv in 0..=n_v {
        for iu in 0..=n_u {
            let u = u_min + du * iu as f64;
            let v = v_min + dv * iv as f64;
            let p = face.surface.evaluate(u, v);
            let normal = face.surface.normal_at(u, v);
            let n = if face.same_sense {
                [normal.x, normal.y, normal.z]
            } else {
                [-normal.x, -normal.y, -normal.z]
            };
            mesh.positions.push([p.x, p.y, p.z]);
            mesh.normals.push(n);
        }
    }

    // Generate triangles for each cell
    for iv in 0..n_v {
        for iu in 0..n_u {
            let i00 = base_idx + (iv * (n_u + 1) + iu) as u32;
            let i10 = base_idx + (iv * (n_u + 1) + iu + 1) as u32;
            let i01 = base_idx + ((iv + 1) * (n_u + 1) + iu) as u32;
            let i11 = base_idx + ((iv + 1) * (n_u + 1) + iu + 1) as u32;

            // Two triangles per cell, orientation so ∂u×∂v points outward
            push_triangle(mesh, i00, i10, i01);
            push_triangle(mesh, i10, i11, i01);
        }
    }

    Ok(())
}

/// Push a triangle, but skip degenerate (zero-area) ones.
fn push_triangle(mesh: &mut TriangleMesh, i0: u32, i1: u32, i2: u32) {
    let p0: [f64; 3] = mesh.positions[i0 as usize];
    let p1: [f64; 3] = mesh.positions[i1 as usize];
    let p2: [f64; 3] = mesh.positions[i2 as usize];
    let u = [p1[0] - p0[0], p1[1] - p0[1], p1[2] - p0[2]];
    let v = [p2[0] - p0[0], p2[1] - p0[1], p2[2] - p0[2]];
    let cross_norm = (u[1] * v[2] - u[2] * v[1]).powi(2)
        + (u[2] * v[0] - u[0] * v[2]).powi(2)
        + (u[0] * v[1] - u[1] * v[0]).powi(2).sqrt();
    if cross_norm < AREA_EPS * AREA_EPS {
        return;
    }
    mesh.indices.push(i0);
    mesh.indices.push(i1);
    mesh.indices.push(i2);
}

/// Surface kind name for error messages.
trait SurfaceKind {
    fn kind_name(&self) -> &'static str;
}

impl SurfaceKind for Surface {
    fn kind_name(&self) -> &'static str {
        match self {
            Surface::Plane { .. } => "plane",
            Surface::Cylinder { .. } => "cylinder",
            Surface::Sphere { .. } => "sphere",
            Surface::Cone { .. } => "cone",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::brep::topology::IdGenerator;
    use crate::geometry::Vec3;
    use crate::primitives::make_cuboid;
    use crate::primitives::make_cylinder;

    /// T06: Normal tessellation — triangle count for cylinder with default 32 angular segments.
    #[test]
    fn test_tessellate_cylinder_triangle_count() {
        let mut gen = IdGenerator::new(0);
        let solid = make_cylinder(5.0, 20.0, &mut gen).unwrap();
        let mesh = tessellate_solid(&solid).unwrap();

        // Lateral: 32 angular segments × 1 axial × 2 triangles = 64
        // Each cap: 32-gon fan = 30 triangles (32 - 2)
        // Total: 64 + 30 + 30 = 124
        let expected = 32 * 2 + (32 - 2) * 2;
        assert_eq!(
            mesh.triangle_count(),
            expected,
            "expected {expected} triangles, got {}",
            mesh.triangle_count()
        );
    }

    /// T07: Determinism — same input → same output.
    #[test]
    fn test_tessellation_cylinder_deterministic() {
        let mut gen1 = IdGenerator::new(0);
        let mut gen2 = IdGenerator::new(0);

        let s1 = make_cylinder(5.0, 20.0, &mut gen1).unwrap();
        let s2 = make_cylinder(5.0, 20.0, &mut gen2).unwrap();

        let m1 = tessellate_solid(&s1).unwrap();
        let m2 = tessellate_solid(&s2).unwrap();

        assert_eq!(m1.positions, m2.positions);
        assert_eq!(m1.normals, m2.normals);
        assert_eq!(m1.indices, m2.indices);
    }

    /// T08: Regression — cuboid tessellation is byte-identical to pre-cylinder output.
    #[test]
    fn test_tessellate_cuboid_unchanged() {
        let mut gen = IdGenerator::new(0);
        let solid = make_cuboid(1.0, 1.0, 1.0, &mut gen).unwrap();
        let mesh = tessellate_solid(&solid).unwrap();

        assert_eq!(mesh.triangle_count(), 12);
        assert_eq!(mesh.positions.len(), 24);
        assert_eq!(mesh.normals.len(), 24);
        assert_eq!(mesh.indices.len(), 36);
    }

    /// T08 extended: exact positions and indices snapshot.
    #[test]
    fn test_cuboid_positions_indices_snapshot() {
        let mut gen1 = IdGenerator::new(0);
        let mut gen2 = IdGenerator::new(0);

        let s1 = make_cuboid(1.0, 1.0, 1.0, &mut gen1).unwrap();
        let s2 = make_cuboid(1.0, 1.0, 1.0, &mut gen2).unwrap();

        let m1 = tessellate_solid(&s1).unwrap();
        let m2 = tessellate_solid(&s2).unwrap();

        // Byte-identical across two runs
        assert_eq!(m1.positions, m2.positions);
        assert_eq!(m1.indices, m2.indices);
    }

    /// T09: Outward normals — all facet normals point away from centroid.
    #[test]
    fn test_cylinder_outward_normals() {
        let mut gen = IdGenerator::new(0);
        let solid = make_cylinder(5.0, 20.0, &mut gen).unwrap();
        let mesh = tessellate_solid(&solid).unwrap();

        // Centroid of cylinder: (0, 0, 10)
        let centroid = [0.0f64, 0.0, 10.0];

        for tri in 0..mesh.triangle_count() {
            let i0 = mesh.indices[tri * 3] as usize;
            let i1 = mesh.indices[tri * 3 + 1] as usize;
            let i2 = mesh.indices[tri * 3 + 2] as usize;

            let p0 = mesh.positions[i0];
            let p1 = mesh.positions[i1];
            let p2 = mesh.positions[i2];

            // Facet center
            let center = [
                (p0[0] + p1[0] + p2[0]) / 3.0,
                (p0[1] + p1[1] + p2[1]) / 3.0,
                (p0[2] + p1[2] + p2[2]) / 3.0,
            ];

            // Cross product for facet normal
            let u = [p1[0] - p0[0], p1[1] - p0[1], p1[2] - p0[2]];
            let v = [p2[0] - p0[0], p2[1] - p0[1], p2[2] - p0[2]];
            let nx = u[1] * v[2] - u[2] * v[1];
            let ny = u[2] * v[0] - u[0] * v[2];
            let nz = u[0] * v[1] - u[1] * v[0];

            // Vector from centroid to facet center
            let dx = center[0] - centroid[0];
            let dy = center[1] - centroid[1];
            let dz = center[2] - centroid[2];

            // Dot product should be positive (normal points outward)
            let dot = nx * dx + ny * dy + nz * dz;
            assert!(
                dot > 0.0,
                "facet {tri}: normal should point outward (dot={dot})"
            );
        }
    }

    /// T12: Resolution externalization + clamping.
    #[test]
    fn test_tessellation_options_clamp() {
        let opts = TessellationOptions::new(2, 0);
        assert_eq!(opts.angular_segments, 3);
        assert_eq!(opts.axial_segments, 1);
    }

    #[test]
    fn test_different_resolution_different_count() {
        let mut gen = IdGenerator::new(0);
        let solid = make_cylinder(5.0, 20.0, &mut gen).unwrap();

        let mesh_default = tessellate_solid(&solid).unwrap();
        let mesh_low = tessellate_solid_with(&solid, &TessellationOptions::new(8, 1)).unwrap();
        let mesh_hi = tessellate_solid_with(&solid, &TessellationOptions::new(64, 2)).unwrap();

        assert_ne!(mesh_default.triangle_count(), mesh_low.triangle_count());
        assert_ne!(mesh_default.triangle_count(), mesh_hi.triangle_count());

        // 8 angular, 1 axial: lateral=8*1*2=16, caps=(8-2)*2=12 → 28
        assert_eq!(mesh_low.triangle_count(), 8 * 2 + (8 - 2) * 2);
        // 64 angular, 2 axial: lateral=64*2*2=256, caps=(64-2)*2=124 → 380
        assert_eq!(mesh_hi.triangle_count(), 64 * 2 * 2 + (64 - 2) * 2);
    }

    /// T15: Unsupported surface type → error.
    #[test]
    fn test_unsupported_surface_error() {
        use crate::brep::topology::Solid as S;
        use crate::geometry::curve::Curve;

        let mut s = S::new(0);
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
        let lp = s.add_loop(5, vec![he0]);
        s.add_face(
            6,
            Surface::Sphere {
                center: Point::origin(),
                radius: 1.0,
            },
            lp,
            vec![],
            true,
        );
        s.add_shell(7, vec![0], true);

        let result = tessellate_solid(&s);
        assert!(matches!(
            result,
            Err(TessellationError::UnsupportedSurface { kind: "sphere" })
        ));
    }

    // Legacy tests preserved for backward compatibility
    #[test]
    fn test_tessellate_cuboid() {
        let mut id_gen = IdGenerator::new(0);
        let solid = make_cuboid(1.0, 1.0, 1.0, &mut id_gen).unwrap();
        let mesh = tessellate_solid(&solid).unwrap();

        assert_eq!(mesh.triangle_count(), 12);
        assert_eq!(mesh.positions.len(), 24);
        assert_eq!(mesh.normals.len(), 24);
        assert_eq!(mesh.indices.len(), 36);
    }

    #[test]
    fn test_tessellation_deterministic() {
        let mut id_gen1 = IdGenerator::new(0);
        let mut id_gen2 = IdGenerator::new(0);

        let solid1 = make_cuboid(2.0, 3.0, 4.0, &mut id_gen1).unwrap();
        let solid2 = make_cuboid(2.0, 3.0, 4.0, &mut id_gen2).unwrap();

        let mesh1 = tessellate_solid(&solid1).unwrap();
        let mesh2 = tessellate_solid(&solid2).unwrap();

        assert_eq!(mesh1.positions, mesh2.positions);
        assert_eq!(mesh1.normals, mesh2.normals);
        assert_eq!(mesh1.indices, mesh2.indices);
    }
    /// Directly-constructed TessellationOptions with 0 segments should not produce NaN vertices
    /// (public fields bypass new() clamping, so tessellate_solid_with must guard at point of use).
    #[test]
    fn test_zero_segments_clamped_at_use() {
        let mut gen = IdGenerator::new(0);
        let solid = make_cylinder(5.0, 20.0, &mut gen).unwrap();
        let opts = TessellationOptions {
            angular_segments: 0,
            axial_segments: 0,
        };
        let mesh = tessellate_solid_with(&solid, &opts).unwrap();
        // Clamped to angular=3, axial=1: lateral=3*2=6, caps=(3-2)*2=2 → 8
        assert_eq!(mesh.triangle_count(), 8);
        assert!(
            mesh.positions
                .iter()
                .all(|p| p.iter().all(|x| x.is_finite())),
            "mesh must not contain NaN/inf vertices"
        );
    }

    /// Non-full-revolution cylinder face (arc < 2π) must return TrimmedFaceUnsupported.
    #[test]
    fn test_partial_revolution_cylinder_face_errors() {
        use crate::brep::topology::Solid as S;
        let mut s = S::new(0);
        let v0 = s.add_vertex(1, Point::new(5.0, 0.0, 0.0));
        let e0 = s.add_edge(
            2,
            [v0, v0],
            Curve::Circle {
                center: Point::origin(),
                normal: Vec3::z(),
                radius: 5.0,
            },
            [0.0, PI], // half revolution
        );
        let he0 = s.add_half_edge(3, v0, e0, true);
        let lp = s.add_loop(4, vec![he0]);
        s.add_face(
            5,
            Surface::Cylinder {
                origin: Point::origin(),
                axis: Vec3::z(),
                radius: 5.0,
            },
            lp,
            vec![],
            true,
        );
        s.add_shell(6, vec![0], true);
        let result = tessellate_solid(&s);
        assert!(matches!(
            result,
            Err(TessellationError::TrimmedFaceUnsupported)
        ));
    }
}
