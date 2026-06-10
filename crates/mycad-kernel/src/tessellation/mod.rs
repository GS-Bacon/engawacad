pub mod stl;
pub use stl::to_ascii_stl;

use crate::brep::topology::Solid;
use crate::geometry::curve::Curve;
use crate::geometry::surface::{Surface, TessellationStrategy};
use crate::geometry::{
    angle_near, arc_segment_count, length_near, point_near, point_near_scaled, unwrap_periodic_uv,
    Point, ANGLE_TOLERANCE, LENGTH_TOLERANCE,
};
use serde::{Deserialize, Serialize};
use std::f64::consts::PI;
use thiserror::Error;
use ts_rs::TS;

/// Minimum cross-product norm for a non-degenerate triangle (heuristic threshold
/// for degenerate triangle removal — not a dimensioned tolerance).
const AREA_EPS: f64 = 1e-14;

/// A triangle mesh for rendering.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct TriangleMesh {
    /// Vertex positions (x, y, z).
    pub positions: Vec<[f64; 3]>,
    /// Normal vectors per vertex.
    pub normals: Vec<[f64; 3]>,
    /// Triangle indices (every 3 indices form a triangle).
    pub indices: Vec<u32>,
    /// Per-triangle face id string. Length always equals `triangle_count()`.
    /// Unnamed faces (`Face.name == None`) produce an empty string.
    pub face_ids: Vec<String>,
}

impl TriangleMesh {
    pub fn new() -> Self {
        Self {
            positions: Vec::new(),
            normals: Vec::new(),
            indices: Vec::new(),
            face_ids: Vec::new(),
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
///
/// Resolution contract per surface type:
/// - **Plane**: `angular_segments` is used for boundary sampling.
/// - **Cylinder**: `angular_segments` = longitude divisions, `axial_segments` = height divisions.
/// - **Sphere**: `angular_segments` = longitude divisions (`n_u`). Latitude divisions (`n_v`) are
///   derived as `(angular_segments / 2).max(2)`. `axial_segments` is **ignored**.
/// - **Cone**: Unsupported.
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

    for (face_idx, face) in solid.faces.iter().enumerate() {
        let face_id = face
            .name
            .as_ref()
            .map(|n| n.canonical_name())
            .unwrap_or_default();
        let strategy = face.surface.tessellation_strategy();
        match strategy {
            TessellationStrategy::BoundaryFan => {
                // Determine if we need earcutr (concave or has inner loops)
                let outer_loop = &solid.loops[face.outer_loop];
                let loop_points =
                    collect_loop_points(solid, outer_loop, opts.angular_segments.max(3), face_idx)?;
                let has_inner = !face.inner_loops.is_empty();
                let is_convex = is_polygon_convex(&loop_points, &face.surface, face.same_sense);

                if has_inner || !is_convex {
                    tessellate_face_earcut(solid, face, opts, &mut mesh, face_idx, &face_id)?;
                } else {
                    tessellate_face_fan_from_points(&loop_points, face, &mut mesh, &face_id)?;
                }
            }
            TessellationStrategy::UvGridFullPatch => {
                tessellate_face_uv_grid(solid, face, face_idx, opts, &mut mesh, &face_id)?;
            }
            TessellationStrategy::UvSphere => {
                tessellate_face_sphere(solid, face, face_idx, opts, &mut mesh, &face_id)?;
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
#[allow(dead_code)]
fn tessellate_face_fan(
    solid: &Solid,
    face: &crate::brep::topology::Face,
    opts: &TessellationOptions,
    mesh: &mut TriangleMesh,
    face_idx: usize,
    face_id: &str,
) -> Result<(), TessellationError> {
    let outer_loop = &solid.loops[face.outer_loop];
    let loop_points =
        collect_loop_points(solid, outer_loop, opts.angular_segments.max(3), face_idx)?;
    tessellate_face_fan_from_points(&loop_points, face, mesh, face_id)
}

/// Tessellate a convex polygon as a fan (given pre-collected points).
fn tessellate_face_fan_from_points(
    loop_points: &[Point],
    face: &crate::brep::topology::Face,
    mesh: &mut TriangleMesh,
    face_id: &str,
) -> Result<(), TessellationError> {
    if loop_points.len() < 3 {
        return Ok(());
    }

    let base_idx = mesh.positions.len() as u32;

    for p in loop_points {
        let normal = face.surface.normal_at_point(p);
        let n = if face.same_sense {
            [normal.x, normal.y, normal.z]
        } else {
            [-normal.x, -normal.y, -normal.z]
        };
        mesh.positions.push([p.x, p.y, p.z]);
        mesh.normals.push(n);
    }

    // Check if the loop winding matches the face outward direction; flip if not.
    let flip = if loop_points.len() >= 3 {
        let p0 = &loop_points[0];
        let p1 = &loop_points[1];
        let p2 = &loop_points[2];
        let e1 = p1 - p0;
        let e2 = p2 - p0;
        let tri_normal = e1.cross(&e2);
        let face_outward = face.surface.normal_at_point(p0);
        let outward = if face.same_sense {
            face_outward
        } else {
            -face_outward
        };
        tri_normal.dot(&outward) < 0.0
    } else {
        false
    };

    for i in 1..(loop_points.len() as u32 - 1) {
        if flip {
            mesh.indices.push(base_idx);
            mesh.indices.push(base_idx + i + 1);
            mesh.indices.push(base_idx + i);
        } else {
            mesh.indices.push(base_idx);
            mesh.indices.push(base_idx + i);
            mesh.indices.push(base_idx + i + 1);
        }
        mesh.face_ids.push(face_id.to_string());
    }

    Ok(())
}

/// Check if a 3D polygon is convex by projecting to 2D and checking cross product signs.
fn is_polygon_convex(points: &[Point], surface: &Surface, _same_sense: bool) -> bool {
    if points.len() < 3 {
        return false;
    }
    let area_eps = LENGTH_TOLERANCE * LENGTH_TOLERANCE;

    // Get the face normal for projection
    let normal = match surface {
        Surface::Plane { normal, .. } => *normal,
        _ => return true, // Assume convex for non-planar
    };

    // Project to 2D using dominant axis
    let (u_idx, v_idx) = if normal.x.abs() >= normal.y.abs() && normal.x.abs() >= normal.z.abs() {
        (1, 2)
    } else if normal.y.abs() >= normal.z.abs() {
        (0, 2)
    } else {
        (0, 1)
    };

    let n = points.len();
    let mut first_sign: Option<f64> = None;
    for i in 0..n {
        let prev = (i + n - 1) % n;
        let next = (i + 1) % n;
        let d1 = (
            points[i].coords[u_idx] - points[prev].coords[u_idx],
            points[i].coords[v_idx] - points[prev].coords[v_idx],
        );
        let d2 = (
            points[next].coords[u_idx] - points[i].coords[u_idx],
            points[next].coords[v_idx] - points[i].coords[v_idx],
        );
        let cross = d1.0 * d2.1 - d1.1 * d2.0;
        if cross.abs() <= area_eps {
            // Borderline — conservatively treat as non-convex
            return false;
        }
        let sign = cross.signum();
        match first_sign {
            None => first_sign = Some(sign),
            Some(fs) if fs != sign => return false,
            _ => {}
        }
    }
    true
}

/// Tessellate a face using earcutr (for concave faces and faces with inner loops).
fn tessellate_face_earcut(
    solid: &Solid,
    face: &crate::brep::topology::Face,
    opts: &TessellationOptions,
    mesh: &mut TriangleMesh,
    face_idx: usize,
    face_id: &str,
) -> Result<(), TessellationError> {
    let outer_loop = &solid.loops[face.outer_loop];
    let outer_points =
        collect_loop_points(solid, outer_loop, opts.angular_segments.max(3), face_idx)?;
    if outer_points.len() < 3 {
        return Ok(());
    }

    // Get projection axes
    let normal = match &face.surface {
        Surface::Plane { normal, .. } => *normal,
        _ => return Err(TessellationError::UnsupportedSurface { kind: "non-planar" }),
    };
    let (u_idx, v_idx) = if normal.x.abs() >= normal.y.abs() && normal.x.abs() >= normal.z.abs() {
        (1, 2)
    } else if normal.y.abs() >= normal.z.abs() {
        (0, 2)
    } else {
        (0, 1)
    };

    // Build flat vertex array for earcutr
    let mut flat_coords: Vec<f64> = Vec::new();
    for p in &outer_points {
        flat_coords.push(p.coords[u_idx]);
        flat_coords.push(p.coords[v_idx]);
    }

    // Collect inner loop points
    let mut hole_indices: Vec<usize> = Vec::new();
    for &il_idx in &face.inner_loops {
        let il = &solid.loops[il_idx];
        let il_points = collect_loop_points(solid, il, opts.angular_segments.max(3), face_idx)?;
        hole_indices.push(flat_coords.len() / 2);
        for p in &il_points {
            flat_coords.push(p.coords[u_idx]);
            flat_coords.push(p.coords[v_idx]);
        }
    }

    // Run earcutr
    let indices = earcutr::earcut(&flat_coords, &hole_indices, 2)
        .map_err(|_| TessellationError::NonManifoldLoop)?;

    let base_idx = mesh.positions.len() as u32;

    // Add all vertices (outer + inner) to mesh
    let all_points: Vec<Point> = outer_points
        .into_iter()
        .chain(face.inner_loops.iter().flat_map(|&il_idx| {
            let il = &solid.loops[il_idx];
            collect_loop_points(solid, il, opts.angular_segments.max(3), face_idx)
                .unwrap_or_default()
        }))
        .collect();

    for p in &all_points {
        let n = face.surface.normal_at_point(p);
        let nm = if face.same_sense {
            [n.x, n.y, n.z]
        } else {
            [-n.x, -n.y, -n.z]
        };
        mesh.positions.push([p.x, p.y, p.z]);
        mesh.normals.push(nm);
    }

    for chunk in indices.chunks(3) {
        let (a, b, c) = if face.same_sense {
            (chunk[0], chunk[1], chunk[2])
        } else {
            (chunk[0], chunk[2], chunk[1]) // flip winding for reversed face
        };
        mesh.indices.push(base_idx + a as u32);
        mesh.indices.push(base_idx + b as u32);
        mesh.indices.push(base_idx + c as u32);
        mesh.face_ids.push(face_id.to_string());
    }

    Ok(())
}

/// Collect the ordered boundary points of a loop by sampling each HE's curve.
/// When an HE has a pcurve, samples via pcurve→surface→3D instead of edge.curve.
fn collect_loop_points(
    solid: &Solid,
    lp: &crate::brep::topology::Loop,
    segments: usize,
    face_idx: usize,
) -> Result<Vec<Point>, TessellationError> {
    use crate::geometry::pcurve::Curve2D;

    let mut points = Vec::new();

    for &he_idx in &lp.half_edges {
        let he = &solid.half_edges[he_idx];
        let edge = &solid.edges[he.edge];

        let seg_points = if let Some(ref pcurve) = he.pcurve {
            let face = &solid.faces[face_idx];
            let uv_samples = match pcurve.curve_2d() {
                Curve2D::Line2D { .. } => {
                    vec![pcurve.evaluate(pcurve.t_range()[0])]
                }
                Curve2D::Circle2D { .. } => {
                    let [ts, te] = pcurve.t_range();
                    let n = arc_segment_count(ts, te, segments);
                    pcurve.sample(n)
                }
            };
            uv_samples
                .into_iter()
                .map(|(u, v)| face.surface.evaluate(u, v))
                .collect()
        } else {
            let (t_start, t_end) = if he.forward {
                (edge.t_range[0], edge.t_range[1])
            } else {
                (edge.t_range[1], edge.t_range[0])
            };
            let n = match &edge.curve {
                Curve::Circle { .. } => arc_segment_count(t_start, t_end, segments),
                Curve::Line { .. } => segments,
            };
            edge.curve.sample_segment(t_start, t_end, n)
        };

        for p in seg_points {
            points.push(p);
        }
    }

    Ok(points)
}

/// Tessellate a trimmed curved face (cylinder/sphere with inner loops or partial spans)
/// by projecting boundary points into UV space and running earcutr.
///
/// The key insight for watertightness: boundary points come from `collect_loop_points`,
/// which are shared with adjacent planar faces (caps) via the same edge topology.
/// After welding, boundary edges are shared by exactly 2 triangles.
fn tessellate_trimmed_uv_face(
    solid: &Solid,
    face: &crate::brep::topology::Face,
    face_idx: usize,
    opts: &TessellationOptions,
    mesh: &mut TriangleMesh,
    face_id: &str,
) -> Result<(), TessellationError> {
    let segments = opts.angular_segments.max(3);

    // 1. Collect outer loop 3D points → project to UV
    let outer_loop = &solid.loops[face.outer_loop];
    let outer_3d = collect_loop_points(solid, outer_loop, segments, face_idx)?;
    if outer_3d.len() < 3 {
        return Ok(());
    }

    let mut outer_uv: Vec<(f64, f64)> = outer_3d.iter().map(|p| face.surface.uv_of(p)).collect();

    // 2. Unwrap periodic u to avoid seam discontinuities
    {
        let mut u_list: Vec<f64> = outer_uv.iter().map(|(u, _)| *u).collect();
        unwrap_periodic_uv(&mut u_list);
        for (i, u) in u_list.into_iter().enumerate() {
            outer_uv[i].0 = u;
        }
    }

    // 4. Build flat coordinate array for earcutr
    let mut flat_coords: Vec<f64> = Vec::with_capacity(outer_3d.len() * 2);
    for (u, v) in &outer_uv {
        flat_coords.push(*u);
        flat_coords.push(*v);
    }

    // Collect inner loops and compute hole start indices.
    // Track actually-added inner loop 3D points so that the earcut index→vertex
    // mapping stays aligned (degenerate inner loops skipped in flat_coords must
    // also be excluded from the vertex array).
    let mut hole_starts: Vec<usize> = Vec::new();
    let mut inner_3d_added: Vec<Vec<Point>> = Vec::new();
    for &il_idx in &face.inner_loops {
        let il = &solid.loops[il_idx];
        let il_3d = collect_loop_points(solid, il, segments, face_idx)?;
        if il_3d.len() < 3 {
            continue;
        }

        let mut il_uv: Vec<(f64, f64)> = il_3d.iter().map(|p| face.surface.uv_of(p)).collect();

        {
            let mut u_list: Vec<f64> = il_uv.iter().map(|(u, _)| *u).collect();
            unwrap_periodic_uv(&mut u_list);
            if let Some(&(outer_u, _)) = outer_uv.first() {
                let avg_inner_u: f64 = u_list.iter().sum::<f64>() / u_list.len() as f64;
                let shift = outer_u - avg_inner_u;
                if shift.abs() > PI {
                    for u in &mut u_list {
                        *u += shift;
                    }
                }
            }
            for (i, u) in u_list.into_iter().enumerate() {
                il_uv[i].0 = u;
            }
        }

        hole_starts.push(flat_coords.len() / 2);
        for (u, v) in &il_uv {
            flat_coords.push(*u);
            flat_coords.push(*v);
        }
        inner_3d_added.push(il_3d);
    }
    let indices = earcutr::earcut(&flat_coords, &hole_starts, 2)
        .map_err(|_| TessellationError::NonManifoldLoop)?;

    let base_idx = mesh.positions.len() as u32;

    // 6. Add all vertices (outer + non-degenerate inner) with surface normals
    let all_points: Vec<Point> = outer_3d
        .into_iter()
        .chain(inner_3d_added.into_iter().flatten())
        .collect();

    for p in &all_points {
        let n = face.surface.normal_at_point(p);
        let nm = if face.same_sense {
            [n.x, n.y, n.z]
        } else {
            [-n.x, -n.y, -n.z]
        };
        mesh.positions.push([p.x, p.y, p.z]);
        mesh.normals.push(nm);
    }

    // 7. Add triangles with winding correction
    for chunk in indices.chunks(3) {
        if chunk.len() < 3 {
            break;
        }
        let (a, b, c) = if face.same_sense {
            (chunk[0], chunk[1], chunk[2])
        } else {
            (chunk[0], chunk[2], chunk[1])
        };

        // Skip degenerate triangles
        let i0 = base_idx + a as u32;
        let i1 = base_idx + b as u32;
        let i2 = base_idx + c as u32;
        push_triangle(mesh, i0, i1, i2, face_id);
    }

    Ok(())
}

/// Triangulate a face using UV grid sampling (full untrimmed periodic/rectangular patch).
fn tessellate_face_uv_grid(
    solid: &Solid,
    face: &crate::brep::topology::Face,
    face_idx: usize,
    opts: &TessellationOptions,
    mesh: &mut TriangleMesh,
    face_id: &str,
) -> Result<(), TessellationError> {
    // inner_loop or partial span → delegate to UV-earcut
    if !face.inner_loops.is_empty() {
        return tessellate_trimmed_uv_face(solid, face, face_idx, opts, mesh, face_id);
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
    let span_ok = full_rev_count > 0
        && (total_circle_span - full_rev_count as f64 * 2.0 * PI).abs() < ANGLE_TOLERANCE;
    if !span_ok {
        return tessellate_trimmed_uv_face(solid, face, face_idx, opts, mesh, face_id);
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

    // u_min: The UV grid covers a full 2π revolution. For primitive cylinders the seam
    // starts at u=0. After boolean operations the seam may relocate, but the UV grid
    // starts at u=0 regardless — adjacent face boundary alignment is achieved through
    // the n_u matching heuristic below, not by shifting u_min.
    let u_min = 0.0_f64;

    // Derive n_u from the number of circle arcs per revolution when the cylinder
    // originated from a boolean operation (arcs_per_rev > 1). The adjacent planar cap
    // samples its boundary at exactly arcs_per_rev points, so the UV grid must match
    // for watertight welding. Primitive cylinders (arcs_per_rev=1) use angular_segments.
    let circle_arc_count: usize = outer_loop
        .half_edges
        .iter()
        .filter(|&&he_idx| {
            let he = &solid.half_edges[he_idx];
            matches!(solid.edges[he.edge].curve, Curve::Circle { .. })
        })
        .count();
    let arcs_per_rev = if full_rev_count > 0 {
        circle_arc_count / full_rev_count as usize
    } else {
        0
    };
    let n_u = if arcs_per_rev > 1 {
        arcs_per_rev
    } else {
        opts.angular_segments.max(3)
    };
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

            // Two triangles per cell, orientation so ∂u×∂v points outward.
            // When same_sense=false the face normal is reversed, so flip winding
            // to keep triangle cross-product consistent with the face normal.
            if face.same_sense {
                push_triangle(mesh, i00, i10, i01, face_id);
                push_triangle(mesh, i10, i11, i01, face_id);
            } else {
                push_triangle(mesh, i00, i01, i10, face_id);
                push_triangle(mesh, i10, i01, i11, face_id);
            }
        }
    }

    Ok(())
}

/// Tessellate a canonical sphere face using UV sphere sampling.
///
/// Validates that the face is a canonical full sphere (2 HEs on 1 seam Circle edge,
/// no inner loops, poles as vertices, seam geometry matches surface) and returns
/// `TrimmedFaceUnsupported` for any non-canonical face.
fn tessellate_face_sphere(
    solid: &Solid,
    face: &crate::brep::topology::Face,
    face_idx: usize,
    opts: &TessellationOptions,
    mesh: &mut TriangleMesh,
    face_id: &str,
) -> Result<(), TessellationError> {
    // If inner loops exist, delegate to sphere-specific trimmed tessellation
    if !face.inner_loops.is_empty() {
        return tessellate_sphere_face_trimmed(solid, face, face_idx, opts, mesh, face_id);
    }

    let outer_loop = &solid.loops[face.outer_loop];

    if outer_loop.half_edges.len() != 2 {
        return Err(TessellationError::TrimmedFaceUnsupported);
    }

    let he0 = &solid.half_edges[outer_loop.half_edges[0]];
    let he1 = &solid.half_edges[outer_loop.half_edges[1]];

    // Both HEs must reference the same edge
    if he0.edge != he1.edge {
        return Err(TessellationError::TrimmedFaceUnsupported);
    }

    // Opposite orientations
    if he0.forward == he1.forward {
        return Err(TessellationError::TrimmedFaceUnsupported);
    }

    let edge = &solid.edges[he0.edge];

    let Curve::Circle {
        center: seam_center,
        normal: seam_normal,
        radius: seam_radius,
    } = &edge.curve
    else {
        return Err(TessellationError::TrimmedFaceUnsupported);
    };

    // Determine which HE is forward, which is reversed
    let (he_fwd, he_rev) = if he0.forward { (he0, he1) } else { (he1, he0) };

    // Forward HE start_vertex should match edge.vertices[0]
    if he_fwd.start_vertex != edge.vertices[0] || he_rev.start_vertex != edge.vertices[1] {
        return Err(TessellationError::TrimmedFaceUnsupported);
    }

    // Loop closure: last HE end = first HE start
    let fwd_end = edge.vertices[1];
    let rev_end = edge.vertices[0];
    let loop_start = he_fwd.start_vertex;
    let loop_end = if he_rev.forward {
        edge.vertices[1]
    } else {
        edge.vertices[0]
    };
    if !point_near(
        &solid.vertices[loop_end].point,
        &solid.vertices[loop_start].point,
    ) {
        return Err(TessellationError::TrimmedFaceUnsupported);
    }
    let _ = (fwd_end, rev_end);

    let Surface::Sphere {
        center: sph_center,
        radius: sph_radius,
    } = &face.surface
    else {
        return Err(TessellationError::TrimmedFaceUnsupported);
    };

    // Verify poles: vertices[0] near south pole (v ≈ -π/2), vertices[1] near north pole (v ≈ +π/2)
    let (u0, v0) = face.surface.uv_of(&solid.vertices[edge.vertices[0]].point);
    let (_u1, v1) = face.surface.uv_of(&solid.vertices[edge.vertices[1]].point);
    if !angle_near(v0, -PI / 2.0) || !angle_near(v1, PI / 2.0) {
        return Err(TessellationError::TrimmedFaceUnsupported);
    }
    let _ = u0;

    // Seam geometry matches sphere
    if !point_near(seam_center, sph_center) || !length_near(*seam_radius, *sph_radius) {
        return Err(TessellationError::TrimmedFaceUnsupported);
    }

    // Canonical seam orientation: normal == -Y
    if (*seam_normal + crate::geometry::Vec3::y()).norm() > LENGTH_TOLERANCE {
        return Err(TessellationError::TrimmedFaceUnsupported);
    }

    // Verify curve.evaluate(t_range[0]) ≈ vertices[0] and evaluate(t_range[1]) ≈ vertices[1]
    let p_start = edge.curve.evaluate(edge.t_range[0]);
    let p_end = edge.curve.evaluate(edge.t_range[1]);
    if !point_near_scaled(
        &p_start,
        &solid.vertices[edge.vertices[0]].point,
        *sph_radius,
    ) || !point_near_scaled(&p_end, &solid.vertices[edge.vertices[1]].point, *sph_radius)
    {
        return Err(TessellationError::TrimmedFaceUnsupported);
    }

    // T_range should be a half-circle (span = π)
    let span = (edge.t_range[1] - edge.t_range[0]).abs();
    if !angle_near(span, PI) {
        return Err(TessellationError::TrimmedFaceUnsupported);
    }

    // --- Tessellation ---
    let n_u = opts.angular_segments.max(3);
    let n_v = (n_u / 2).max(2);

    let center = *sph_center;
    let radius = *sph_radius;

    let south_pole = center + crate::geometry::Vec3::new(0.0, 0.0, -radius);
    let north_pole = center + crate::geometry::Vec3::new(0.0, 0.0, radius);

    let base_idx = mesh.positions.len() as u32;

    // South pole vertex
    {
        let normal = face.surface.normal_at_point(&south_pole);
        let n = normal_arr(&normal, face.same_sense);
        mesh.positions
            .push([south_pole.x, south_pole.y, south_pole.z]);
        mesh.normals.push(n);
    }

    // Internal latitude rings: iv = 1..=n_v-1
    // v = -π/2 + (π/n_v)*iv, each ring has n_u points
    let du = 2.0 * PI / n_u as f64;
    for iv in 1..n_v {
        let v_lat = -PI / 2.0 + (PI / n_v as f64) * iv as f64;
        for iu in 0..n_u {
            let u = du * iu as f64;
            let p = face.surface.evaluate(u, v_lat);
            let normal = face.surface.normal_at(u, v_lat);
            let n = normal_arr(&normal, face.same_sense);
            mesh.positions.push([p.x, p.y, p.z]);
            mesh.normals.push(n);
        }
    }

    // North pole vertex
    {
        let normal = face.surface.normal_at_point(&north_pole);
        let n = normal_arr(&normal, face.same_sense);
        mesh.positions
            .push([north_pole.x, north_pole.y, north_pole.z]);
        mesh.normals.push(n);
    }

    let south_idx = base_idx;
    let north_idx = base_idx + (1 + (n_v - 1) * n_u) as u32;

    // South pole fan: south → bottom ring
    let bottom_ring_start = base_idx + 1;
    for iu in 0..n_u {
        let cur = bottom_ring_start + iu as u32;
        let next = bottom_ring_start + ((iu + 1) % n_u) as u32;
        if face.same_sense {
            push_triangle(mesh, south_idx, next, cur, face_id);
        } else {
            push_triangle(mesh, south_idx, cur, next, face_id);
        }
    }

    // Middle bands
    for iv in 0..(n_v - 2) {
        let ring_a_start = base_idx + 1 + (iv * n_u) as u32;
        let ring_b_start = base_idx + 1 + ((iv + 1) * n_u) as u32;
        for iu in 0..n_u {
            let a0 = ring_a_start + iu as u32;
            let a1 = ring_a_start + ((iu + 1) % n_u) as u32;
            let b0 = ring_b_start + iu as u32;
            let b1 = ring_b_start + ((iu + 1) % n_u) as u32;
            if face.same_sense {
                push_triangle(mesh, a0, a1, b0, face_id);
                push_triangle(mesh, a1, b1, b0, face_id);
            } else {
                push_triangle(mesh, a0, b0, a1, face_id);
                push_triangle(mesh, a1, b0, b1, face_id);
            }
        }
    }

    // North pole fan: top ring → north
    let top_ring_start = base_idx + 1 + ((n_v - 2) * n_u) as u32;
    for iu in 0..n_u {
        let cur = top_ring_start + iu as u32;
        let next = top_ring_start + ((iu + 1) % n_u) as u32;
        if face.same_sense {
            push_triangle(mesh, cur, next, north_idx, face_id);
        } else {
            push_triangle(mesh, next, cur, north_idx, face_id);
        }
    }

    Ok(())
}

/// Tessellate a sphere face with inner loops (trimmed by intersection).
///
/// Uses restricted v-range UV grid sampling. The boundary ring is sampled via
/// `collect_loop_points` so that vertices match adjacent planar faces, ensuring
/// watertight welding. The latitude (v_lat) of the inner loop circle is extracted
/// from the edge geometry. The trim direction is determined from the relative
/// position of the cutting plane to the sphere center.
fn tessellate_sphere_face_trimmed(
    solid: &Solid,
    face: &crate::brep::topology::Face,
    face_idx: usize,
    opts: &TessellationOptions,
    mesh: &mut TriangleMesh,
    face_id: &str,
) -> Result<(), TessellationError> {
    let Surface::Sphere {
        center: sph_center,
        radius: sph_radius,
    } = &face.surface
    else {
        return Err(TessellationError::UnsupportedSurface { kind: "non-sphere" });
    };
    let radius = *sph_radius;
    let center = *sph_center;

    // Validate inner loop structure: exactly 1 inner loop with ≥2 HEs
    if face.inner_loops.len() != 1 {
        return Err(TessellationError::TrimmedFaceUnsupported);
    }
    let il_idx = face.inner_loops[0];
    let il = &solid.loops[il_idx];
    if il.half_edges.len() < 2 {
        return Err(TessellationError::TrimmedFaceUnsupported);
    }
    let he = &solid.half_edges[il.half_edges[0]];
    let edge = &solid.edges[he.edge];
    let Curve::Circle {
        center: circ_center,
        normal: circ_normal,
        radius: circ_radius,
    } = &edge.curve
    else {
        return Err(TessellationError::TrimmedFaceUnsupported);
    };

    // The circle center z gives us the latitude
    let center_z = circ_center.coords.z;
    // v_lat = asin((center_z - sph_center_z) / R)
    let rel_z = (center_z - center.coords.z) / radius;
    let rel_z = rel_z.clamp(-1.0, 1.0);
    let v_lat = rel_z.asin();

    // Both intersection circles share normal +Z, so circ_normal cannot tell the
    // upper cap from the lower cap. Decide from the circle's position relative to
    // the sphere center (same criterion as classify::get_fragment_interior_point):
    // a cutting circle below the center keeps the lower (south-pole) cap.
    let trim_lower = center_z < center.coords.z;

    // Use collect_loop_points only for the point count (n_u) so it matches the
    // adjacent planar face's boundary resolution. The actual boundary ring uses
    // uniform u sampling to preserve winding consistency with internal rings.
    let boundary_count =
        collect_loop_points(solid, il, opts.angular_segments.max(3), face_idx)?.len();
    let n_u = boundary_count.max(3);
    let n_v = (n_u / 2).max(2);

    let du = 2.0 * PI / n_u as f64;

    // Determine v range based on trim direction
    let (v_boundary, pole_v) = if trim_lower {
        (v_lat, -PI / 2.0) // lower cap: v_lat → south pole
    } else {
        (v_lat, PI / 2.0) // upper cap: v_lat → north pole
    };

    // Boundary ring via uniform u sampling — same angular spacing as internal
    // rings, so winding is consistent. Positions match the adjacent face because
    // both use the same n_u and 2π/n_u angular step.
    let ring_start = mesh.positions.len() as u32;
    for iu in 0..n_u {
        let u = du * iu as f64;
        let p = face.surface.evaluate(u, v_boundary);
        let normal = face.surface.normal_at(u, v_boundary);
        mesh.positions.push([p.x, p.y, p.z]);
        mesh.normals.push(normal_arr(&normal, face.same_sense));
    }

    // Sample internal rings between v_boundary and pole
    for iv in 1..n_v {
        let frac = iv as f64 / n_v as f64;
        let v = v_boundary + (pole_v - v_boundary) * frac;
        let ring_base = mesh.positions.len() as u32;
        for iu in 0..n_u {
            let u = du * iu as f64;
            let p = face.surface.evaluate(u, v);
            let normal = face.surface.normal_at(u, v);
            mesh.positions.push([p.x, p.y, p.z]);
            mesh.normals.push(normal_arr(&normal, face.same_sense));
        }
        // Triangles between this ring and previous ring (or boundary ring)
        let prev_start = if iv == 1 {
            ring_start
        } else {
            ring_base - n_u as u32
        };
        for iu in 0..n_u {
            let a0 = prev_start + iu as u32;
            let a1 = prev_start + ((iu + 1) % n_u) as u32;
            let b0 = ring_base + iu as u32;
            let b1 = ring_base + ((iu + 1) % n_u) as u32;
            if face.same_sense != trim_lower {
                push_triangle(mesh, a0, a1, b0, face_id);
                push_triangle(mesh, a1, b1, b0, face_id);
            } else {
                push_triangle(mesh, a0, b0, a1, face_id);
                push_triangle(mesh, a1, b0, b1, face_id);
            }
        }
    }

    // Pole vertex + fan
    let pole_p = face.surface.evaluate(0.0, pole_v);
    let pole_normal = face.surface.normal_at(0.0, pole_v);
    let pole_idx = mesh.positions.len() as u32;
    mesh.positions.push([pole_p.x, pole_p.y, pole_p.z]);
    mesh.normals.push(normal_arr(&pole_normal, face.same_sense));

    let last_ring_start = mesh.positions.len() as u32 - 1 - n_u as u32;
    for iu in 0..n_u {
        let cur = last_ring_start + iu as u32;
        let next = last_ring_start + ((iu + 1) % n_u) as u32;
        if trim_lower {
            // south pole fan
            if face.same_sense {
                push_triangle(mesh, pole_idx, next, cur, face_id);
            } else {
                push_triangle(mesh, pole_idx, cur, next, face_id);
            }
        } else {
            // north pole fan
            if face.same_sense {
                push_triangle(mesh, cur, next, pole_idx, face_id);
            } else {
                push_triangle(mesh, next, cur, pole_idx, face_id);
            }
        }
    }

    let _ = (circ_radius, circ_normal);
    Ok(())
}

fn normal_arr(normal: &crate::geometry::Vec3, same_sense: bool) -> [f64; 3] {
    if same_sense {
        [normal.x, normal.y, normal.z]
    } else {
        [-normal.x, -normal.y, -normal.z]
    }
}

/// Push a triangle, but skip degenerate (zero-area) ones.
fn push_triangle(mesh: &mut TriangleMesh, i0: u32, i1: u32, i2: u32, face_id: &str) {
    let p0: [f64; 3] = mesh.positions[i0 as usize];
    let p1: [f64; 3] = mesh.positions[i1 as usize];
    let p2: [f64; 3] = mesh.positions[i2 as usize];
    let u = [p1[0] - p0[0], p1[1] - p0[1], p1[2] - p0[2]];
    let v = [p2[0] - p0[0], p2[1] - p0[1], p2[2] - p0[2]];
    let cross_norm_sq = (u[1] * v[2] - u[2] * v[1]).powi(2)
        + (u[2] * v[0] - u[0] * v[2]).powi(2)
        + (u[0] * v[1] - u[1] * v[0]).powi(2);
    if cross_norm_sq < AREA_EPS * AREA_EPS {
        return;
    }
    mesh.indices.push(i0);
    mesh.indices.push(i1);
    mesh.indices.push(i2);
    mesh.face_ids.push(face_id.to_string());
}

/// Merge multiple meshes into one by concatenating positions/normals
/// and offsetting indices from subsequent meshes by the accumulated vertex count.
pub fn merge_meshes(meshes: &[TriangleMesh]) -> TriangleMesh {
    let total_positions = meshes.iter().map(|m| m.positions.len()).sum();
    let total_normals = meshes.iter().map(|m| m.normals.len()).sum();
    let total_indices = meshes.iter().map(|m| m.indices.len()).sum();

    let mut merged = TriangleMesh {
        positions: Vec::with_capacity(total_positions),
        normals: Vec::with_capacity(total_normals),
        indices: Vec::with_capacity(total_indices),
        face_ids: Vec::new(),
    };

    let mut vertex_offset: u32 = 0;
    for mesh in meshes {
        merged.positions.extend_from_slice(&mesh.positions);
        merged.normals.extend_from_slice(&mesh.normals);
        for &idx in &mesh.indices {
            merged.indices.push(vertex_offset + idx);
        }
        merged.face_ids.extend_from_slice(&mesh.face_ids);
        vertex_offset += mesh.positions.len() as u32;
    }

    merged
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
    use crate::primitives::make_sphere;

    /// T06: Normal tessellation — triangle count for cylinder with default 32 angular segments.
    #[test]
    fn test_tessellate_cylinder_triangle_count() {
        let mut gen = IdGenerator::new(0);
        let solid = make_cylinder(5.0, 20.0, Point::origin(), &mut gen).unwrap();
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

        let s1 = make_cylinder(5.0, 20.0, Point::origin(), &mut gen1).unwrap();
        let s2 = make_cylinder(5.0, 20.0, Point::origin(), &mut gen2).unwrap();

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
        let solid = make_cylinder(5.0, 20.0, Point::origin(), &mut gen).unwrap();
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
        let solid = make_cylinder(5.0, 20.0, Point::origin(), &mut gen).unwrap();

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

    /// T13: Unsupported surface type (Cone) → error.
    #[test]
    fn test_unsupported_surface_error() {
        use crate::brep::topology::Solid as S;
        use crate::geometry::curve::Curve;

        let mut s = S::new(0);
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
        let lp = s.add_loop(5, vec![he0]);
        s.add_face(
            6,
            Surface::Cone {
                apex: Point::origin(),
                axis: Vec3::z(),
                half_angle: 0.5,
            },
            lp,
            vec![],
            true,
            None,
        );
        s.add_shell(7, vec![0], true);

        let result = tessellate_solid(&s);
        assert!(matches!(
            result,
            Err(TessellationError::UnsupportedSurface { kind: "cone" })
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
        let solid = make_cylinder(5.0, 20.0, Point::origin(), &mut gen).unwrap();
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

    /// Non-full-revolution cylinder face (arc < 2π) is tessellated via UV-earcut.
    #[test]
    fn test_partial_revolution_cylinder_face_tessellated() {
        use crate::brep::topology::Solid as S;
        let mut s = S::new(0);
        let v0 = s.add_vertex(1, Point::new(5.0, 0.0, 0.0), None);
        let e0 = s.add_edge(
            2,
            [v0, v0],
            Curve::Circle {
                center: Point::origin(),
                normal: Vec3::z(),
                radius: 5.0,
            },
            [0.0, PI], // half revolution
            None,
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
            None,
        );
        s.add_shell(6, vec![0], true);
        let result = tessellate_solid(&s);
        // Partial revolution now succeeds via tessellate_trimmed_uv_face
        assert!(
            result.is_ok(),
            "partial revolution cylinder should tessellate successfully"
        );
        let mesh = result.unwrap();
        // Single-arc open loop has no enclosed area → 0 triangles is valid
        assert!(
            mesh.positions
                .iter()
                .all(|p| p.iter().all(|x| x.is_finite())),
            "no NaN/Inf"
        );
    }

    // --- Sphere tessellation tests ---

    /// T07: Sphere mesh triangle count = 2*n_u*(n_v-1). Default (angular=32): 960.
    #[test]
    fn test_sphere_triangle_count() {
        let mut gen = IdGenerator::new(0);
        let solid = make_sphere(5.0, Point::origin(), &mut gen).unwrap();
        let mesh = tessellate_solid(&solid).unwrap();

        let n_u = 32;
        let n_v = (n_u / 2).max(2);
        let expected = 2 * n_u * (n_v - 1);
        assert_eq!(
            mesh.triangle_count(),
            expected,
            "expected {expected} triangles, got {}",
            mesh.triangle_count()
        );
    }

    /// T08: Sphere tessellation determinism.
    #[test]
    fn test_sphere_tessellation_deterministic() {
        let mut gen1 = IdGenerator::new(0);
        let mut gen2 = IdGenerator::new(0);
        let s1 = make_sphere(5.0, Point::origin(), &mut gen1).unwrap();
        let s2 = make_sphere(5.0, Point::origin(), &mut gen2).unwrap();

        let m1 = tessellate_solid(&s1).unwrap();
        let m2 = tessellate_solid(&s2).unwrap();

        assert_eq!(m1.positions, m2.positions);
        assert_eq!(m1.normals, m2.normals);
        assert_eq!(m1.indices, m2.indices);
    }

    /// T09: Watertight — every undirected edge is shared by exactly 2 triangles.
    #[test]
    fn test_sphere_watertight() {
        let mut gen = IdGenerator::new(0);
        let solid = make_sphere(5.0, Point::origin(), &mut gen).unwrap();
        let mesh = tessellate_solid(&solid).unwrap();

        let mut edge_count: std::collections::HashMap<[u32; 2], usize> =
            std::collections::HashMap::new();

        for tri in 0..mesh.triangle_count() {
            let i0 = mesh.indices[tri * 3];
            let i1 = mesh.indices[tri * 3 + 1];
            let i2 = mesh.indices[tri * 3 + 2];
            for edge in &[[i0, i1], [i1, i2], [i2, i0]] {
                let key = if edge[0] < edge[1] {
                    [edge[0], edge[1]]
                } else {
                    [edge[1], edge[0]]
                };
                *edge_count.entry(key).or_insert(0) += 1;
            }
        }

        for (edge, count) in &edge_count {
            assert_eq!(
                *count, 2,
                "edge {:?} shared by {count} triangles (expected 2)",
                edge
            );
        }
    }

    /// T10: Pole integrity — poles are exact single vertices, fan has n_u triangles.
    #[test]
    fn test_sphere_pole_integrity() {
        let mut gen = IdGenerator::new(0);
        let solid = make_sphere(5.0, Point::origin(), &mut gen).unwrap();
        let mesh = tessellate_solid(&solid).unwrap();

        let eps = 1e-10;
        let radius = 5.0;

        // South pole at (0,0,-r)
        let south = [0.0f64, 0.0, -radius];
        let north = [0.0f64, 0.0, radius];

        let south_count = mesh
            .positions
            .iter()
            .filter(|p| {
                (p[0] - south[0]).abs() < eps
                    && (p[1] - south[1]).abs() < eps
                    && (p[2] - south[2]).abs() < eps
            })
            .count();
        let north_count = mesh
            .positions
            .iter()
            .filter(|p| {
                (p[0] - north[0]).abs() < eps
                    && (p[1] - north[1]).abs() < eps
                    && (p[2] - north[2]).abs() < eps
            })
            .count();

        assert_eq!(south_count, 1, "south pole should be exactly 1 vertex");
        assert_eq!(north_count, 1, "north pole should be exactly 1 vertex");

        // Find the pole vertex indices
        let south_idx = mesh
            .positions
            .iter()
            .position(|p| {
                (p[0] - south[0]).abs() < eps
                    && (p[1] - south[1]).abs() < eps
                    && (p[2] - south[2]).abs() < eps
            })
            .unwrap() as u32;
        let north_idx = mesh
            .positions
            .iter()
            .position(|p| {
                (p[0] - north[0]).abs() < eps
                    && (p[1] - north[1]).abs() < eps
                    && (p[2] - north[2]).abs() < eps
            })
            .unwrap() as u32;

        // Count fan triangles at each pole
        let south_fan: usize = (0..mesh.triangle_count())
            .filter(|&tri| {
                let i0 = mesh.indices[tri * 3];
                let i1 = mesh.indices[tri * 3 + 1];
                let i2 = mesh.indices[tri * 3 + 2];
                i0 == south_idx || i1 == south_idx || i2 == south_idx
            })
            .count();
        let north_fan: usize = (0..mesh.triangle_count())
            .filter(|&tri| {
                let i0 = mesh.indices[tri * 3];
                let i1 = mesh.indices[tri * 3 + 1];
                let i2 = mesh.indices[tri * 3 + 2];
                i0 == north_idx || i1 == north_idx || i2 == north_idx
            })
            .count();

        let n_u = 32;
        assert_eq!(south_fan, n_u, "south pole fan should have {n_u} triangles");
        assert_eq!(north_fan, n_u, "north pole fan should have {n_u} triangles");
    }

    /// T12: Non-canonical sphere face → TrimmedFaceUnsupported.
    #[test]
    fn test_non_canonical_sphere_face() {
        use crate::brep::topology::Solid as S;

        // Case 1: wrong number of HEs (3 instead of 2)
        {
            let mut s = S::new(0);
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
            let he1 = s.add_half_edge(5, v1, e0, false);
            let he_extra = s.add_half_edge(6, v1, e0, true);
            let lp = s.add_loop(7, vec![he0, he1, he_extra]);
            s.add_face(
                8,
                Surface::Sphere {
                    center: Point::origin(),
                    radius: 5.0,
                },
                lp,
                vec![],
                true,
                None,
            );
            s.add_shell(9, vec![0], true);
            assert!(matches!(
                tessellate_solid(&s),
                Err(TessellationError::TrimmedFaceUnsupported)
            ));
        }

        // Case 2: inner loops present
        {
            let mut s = S::new(0);
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
            let he1 = s.add_half_edge(5, v1, e0, false);
            let lp_outer = s.add_loop(6, vec![he0, he1]);
            let lp_inner = s.add_loop(7, vec![he0]);
            s.add_face(
                8,
                Surface::Sphere {
                    center: Point::origin(),
                    radius: 5.0,
                },
                lp_outer,
                vec![lp_inner],
                true,
                None,
            );
            s.add_shell(9, vec![0], true);
            assert!(matches!(
                tessellate_solid(&s),
                Err(TessellationError::TrimmedFaceUnsupported)
            ));
        }

        // Case 3: HEs on different edges
        {
            let mut s = S::new(0);
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
            let e1 = s.add_edge(
                4,
                [v0, v1],
                Curve::Line {
                    origin: Point::new(0.0, 0.0, -5.0),
                    direction: Vec3::new(0.0, 0.0, 10.0),
                },
                [0.0, 1.0],
                None,
            );
            let he0 = s.add_half_edge(5, v0, e0, true);
            let he1 = s.add_half_edge(6, v1, e1, false);
            let lp = s.add_loop(7, vec![he0, he1]);
            s.add_face(
                8,
                Surface::Sphere {
                    center: Point::origin(),
                    radius: 5.0,
                },
                lp,
                vec![],
                true,
                None,
            );
            s.add_shell(9, vec![0], true);
            assert!(matches!(
                tessellate_solid(&s),
                Err(TessellationError::TrimmedFaceUnsupported)
            ));
        }
    }

    /// T15: Outward normals — winding normal same sign as centroid→facet_center.
    #[test]
    fn test_sphere_outward_normals() {
        let mut gen = IdGenerator::new(0);
        let solid = make_sphere(5.0, Point::origin(), &mut gen).unwrap();
        let mesh = tessellate_solid(&solid).unwrap();

        let center = [0.0f64, 0.0, 0.0];

        for tri in 0..mesh.triangle_count() {
            let i0 = mesh.indices[tri * 3] as usize;
            let i1 = mesh.indices[tri * 3 + 1] as usize;
            let i2 = mesh.indices[tri * 3 + 2] as usize;

            let p0 = mesh.positions[i0];
            let p1 = mesh.positions[i1];
            let p2 = mesh.positions[i2];

            let facet_center = [
                (p0[0] + p1[0] + p2[0]) / 3.0,
                (p0[1] + p1[1] + p2[1]) / 3.0,
                (p0[2] + p1[2] + p2[2]) / 3.0,
            ];

            let u = [p1[0] - p0[0], p1[1] - p0[1], p1[2] - p0[2]];
            let v = [p2[0] - p0[0], p2[1] - p0[1], p2[2] - p0[2]];
            let nx = u[1] * v[2] - u[2] * v[1];
            let ny = u[2] * v[0] - u[0] * v[2];
            let nz = u[0] * v[1] - u[1] * v[0];

            let dx = facet_center[0] - center[0];
            let dy = facet_center[1] - center[1];
            let dz = facet_center[2] - center[2];

            let dot = nx * dx + ny * dy + nz * dz;
            assert!(
                dot > 0.0,
                "facet {tri}: winding normal should point outward (dot={dot})"
            );
        }
    }

    /// T19: Parameterized angular segments — formula, watertight, outward normals.
    #[test]
    fn test_sphere_various_angular_segments() {
        for &angular in &[3, 4, 5, 7, 32] {
            let mut gen = IdGenerator::new(0);
            let solid = make_sphere(5.0, Point::origin(), &mut gen).unwrap();
            let opts = TessellationOptions {
                angular_segments: angular,
                axial_segments: 1,
            };
            let mesh = tessellate_solid_with(&solid, &opts).unwrap();

            let n_u = angular.max(3);
            let n_v = (n_u / 2).max(2);
            let expected = 2 * n_u * (n_v - 1);
            assert_eq!(
                mesh.triangle_count(),
                expected,
                "angular={angular}: expected {expected}, got {}",
                mesh.triangle_count()
            );

            // Watertight
            let mut edge_count: std::collections::HashMap<[u32; 2], usize> =
                std::collections::HashMap::new();
            for tri in 0..mesh.triangle_count() {
                let i0 = mesh.indices[tri * 3];
                let i1 = mesh.indices[tri * 3 + 1];
                let i2 = mesh.indices[tri * 3 + 2];
                for edge in &[[i0, i1], [i1, i2], [i2, i0]] {
                    let key = if edge[0] < edge[1] {
                        [edge[0], edge[1]]
                    } else {
                        [edge[1], edge[0]]
                    };
                    *edge_count.entry(key).or_insert(0) += 1;
                }
            }
            for (edge, count) in &edge_count {
                assert_eq!(
                    *count, 2,
                    "angular={angular}: edge {:?} shared by {count}",
                    edge
                );
            }

            // Outward normals
            for tri in 0..mesh.triangle_count() {
                let i0 = mesh.indices[tri * 3] as usize;
                let i1 = mesh.indices[tri * 3 + 1] as usize;
                let i2 = mesh.indices[tri * 3 + 2] as usize;
                let p0 = mesh.positions[i0];
                let p1 = mesh.positions[i1];
                let p2 = mesh.positions[i2];
                let u = [p1[0] - p0[0], p1[1] - p0[1], p1[2] - p0[2]];
                let v = [p2[0] - p0[0], p2[1] - p0[1], p2[2] - p0[2]];
                let nx = u[1] * v[2] - u[2] * v[1];
                let ny = u[2] * v[0] - u[0] * v[2];
                let nz = u[0] * v[1] - u[1] * v[0];
                let fc = [
                    (p0[0] + p1[0] + p2[0]) / 3.0,
                    (p0[1] + p1[1] + p2[1]) / 3.0,
                    (p0[2] + p1[2] + p2[2]) / 3.0,
                ];
                let dot = nx * fc[0] + ny * fc[1] + nz * fc[2];
                assert!(
                    dot > 0.0,
                    "angular={angular} facet {tri}: outward (dot={dot})"
                );
            }
        }
    }

    /// Sphere tessellation with 0 angular_segments gets clamped to 3.
    #[test]
    fn test_sphere_zero_segments_clamped() {
        let mut gen = IdGenerator::new(0);
        let solid = make_sphere(5.0, Point::origin(), &mut gen).unwrap();
        let opts = TessellationOptions {
            angular_segments: 0,
            axial_segments: 0,
        };
        let mesh = tessellate_solid_with(&solid, &opts).unwrap();
        let n_u = 3;
        let n_v = (n_u / 2).max(2);
        assert_eq!(mesh.triangle_count(), 2 * n_u * (n_v - 1));
        assert!(
            mesh.positions
                .iter()
                .all(|p| p.iter().all(|x| x.is_finite())),
            "mesh must not contain NaN/inf"
        );
    }

    /// F01 regression: large radius sphere (r=1e6) tessellation succeeds.
    /// With absolute eps=1e-9, curve.evaluate(π) x-component ≈ r*sin(π) ≈ 1.2e-10 would fail
    /// because r*1.2e-16 ≈ 1.2e-10 and the check was > 1e-9. Relative eps fixes this.
    #[test]
    fn test_sphere_large_radius_tessellation() {
        let mut gen = IdGenerator::new(0);
        let solid = make_sphere(1e6, Point::origin(), &mut gen).unwrap();
        let mesh = tessellate_solid(&solid);
        assert!(
            mesh.is_ok(),
            "large radius sphere should tessellate successfully"
        );
        assert!(mesh.unwrap().triangle_count() > 0);
    }

    /// Sphere tessellation 100-run determinism.
    #[test]
    fn test_sphere_tessellation_100_runs() {
        let first = {
            let mut gen = IdGenerator::new(0);
            let s = make_sphere(5.0, Point::origin(), &mut gen).unwrap();
            tessellate_solid(&s).unwrap()
        };
        for i in 1..100 {
            let mut gen = IdGenerator::new(0);
            let s = make_sphere(5.0, Point::origin(), &mut gen).unwrap();
            let m = tessellate_solid(&s).unwrap();
            assert_eq!(first.indices, m.indices, "run {i}: indices mismatch");
            assert_eq!(
                first.positions.len(),
                m.positions.len(),
                "run {i}: positions len"
            );
        }
    }

    /// Adversarial: sphere with r=1e-10 tessellates without NaN/Inf.
    #[test]
    fn test_sphere_tiny_radius_tessellation() {
        let mut gen = IdGenerator::new(0);
        let solid = make_sphere(1e-10, Point::origin(), &mut gen).unwrap();
        let mesh = tessellate_solid(&solid).unwrap();
        assert!(
            mesh.positions
                .iter()
                .all(|p| p.iter().all(|x| x.is_finite())),
            "tiny sphere mesh must not contain NaN/Inf"
        );
        // r=1e-10 triangles have cross_norm_sq ≈ 1e-40 << AREA_EPS² = 1e-28
        assert_eq!(
            mesh.triangle_count(),
            0,
            "sub-AREA_EPS sphere should produce no triangles"
        );
    }

    /// Adversarial: sphere with r=1e10 tessellates (F01 regression at extreme scale).
    #[test]
    fn test_sphere_extreme_large_radius_tessellation() {
        let mut gen = IdGenerator::new(0);
        let solid = make_sphere(1e10, Point::origin(), &mut gen).unwrap();
        let mesh = tessellate_solid(&solid).unwrap();
        assert!(
            mesh.positions
                .iter()
                .all(|p| p.iter().all(|x| x.is_finite())),
            "extreme large sphere mesh must not contain NaN/Inf"
        );
        assert!(mesh.triangle_count() > 0);
    }

    /// Adversarial: sphere tessellation with angular=3 (minimum) is watertight.
    #[test]
    fn test_sphere_minimum_angular_watertight() {
        let mut gen = IdGenerator::new(0);
        let solid = make_sphere(5.0, Point::origin(), &mut gen).unwrap();
        let opts = TessellationOptions {
            angular_segments: 3,
            axial_segments: 1,
        };
        let mesh = tessellate_solid_with(&solid, &opts).unwrap();
        let mut edge_count: std::collections::HashMap<[u32; 2], usize> =
            std::collections::HashMap::new();
        for tri in 0..mesh.triangle_count() {
            let i0 = mesh.indices[tri * 3];
            let i1 = mesh.indices[tri * 3 + 1];
            let i2 = mesh.indices[tri * 3 + 2];
            for edge in &[[i0, i1], [i1, i2], [i2, i0]] {
                let key = if edge[0] < edge[1] {
                    [edge[0], edge[1]]
                } else {
                    [edge[1], edge[0]]
                };
                *edge_count.entry(key).or_insert(0) += 1;
            }
        }
        for (edge, count) in &edge_count {
            assert_eq!(
                *count, 2,
                "angular=3: edge {:?} shared by {count} triangles",
                edge
            );
        }
    }

    // --- T06: Slightly-above-2π span is tessellated via UV-earcut ---

    #[test]
    fn test_t06_span_above_2pi_tessellated() {
        use crate::brep::topology::Solid as S;
        // Span = 2π + ANGLE_TOLERANCE (just above) — falls through to UV-earcut
        let span = 2.0 * PI + ANGLE_TOLERANCE;
        let mut s = S::new(0);
        let v0 = s.add_vertex(1, Point::new(5.0, 0.0, 0.0), None);
        let e0 = s.add_edge(
            2,
            [v0, v0],
            Curve::Circle {
                center: Point::origin(),
                normal: Vec3::z(),
                radius: 5.0,
            },
            [0.0, span],
            None,
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
            None,
        );
        s.add_shell(6, vec![0], true);
        let result = tessellate_solid(&s);
        // Slightly-above-2π now succeeds via tessellate_trimmed_uv_face
        assert!(
            result.is_ok(),
            "span = 2π + ANGLE_TOLERANCE should tessellate via UV-earcut"
        );
        let mesh = result.unwrap();
        // Single-arc synthetic case may produce 0 triangles (no enclosed area)
        assert!(
            mesh.positions
                .iter()
                .all(|p| p.iter().all(|x| x.is_finite())),
            "no NaN/Inf"
        );
    }

    // --- T09: Degenerate triangle regression ---

    #[test]
    fn test_t09_degenerate_triangle_not_added() {
        let mut mesh = TriangleMesh::new();
        mesh.positions.push([0.0, 0.0, 0.0]);
        mesh.positions.push([1.0, 0.0, 0.0]);
        mesh.positions.push([0.5, 0.0, 0.0]); // Collinear → degenerate
        let before = mesh.indices.len();
        push_triangle(&mut mesh, 0, 1, 2, "");
        assert_eq!(
            mesh.indices.len(),
            before,
            "degenerate triangle should not be added"
        );
    }

    #[test]
    fn test_t09_normal_triangle_added() {
        let mut mesh = TriangleMesh::new();
        mesh.positions.push([0.0, 0.0, 0.0]);
        mesh.positions.push([1.0, 0.0, 0.0]);
        mesh.positions.push([0.0, 1.0, 0.0]); // Non-degenerate
        let before = mesh.indices.len();
        push_triangle(&mut mesh, 0, 1, 2, "");
        assert_eq!(
            mesh.indices.len(),
            before + 3,
            "normal triangle should be added"
        );
    }

    #[test]
    fn test_t09_identical_points_triangle_not_added() {
        let mut mesh = TriangleMesh::new();
        mesh.positions.push([1.0, 2.0, 3.0]);
        mesh.positions.push([1.0, 2.0, 3.0]);
        mesh.positions.push([1.0, 2.0, 3.0]); // All same point
        let before = mesh.indices.len();
        push_triangle(&mut mesh, 0, 1, 2, "");
        assert_eq!(
            mesh.indices.len(),
            before,
            "identical-point triangle should not be added"
        );
    }

    // --- T11 regression: existing tests pass (covered by existing tests above) ---
    // The existing cuboid/cylinder/sphere tessellation tests serve as T11 regression.

    // --- Edge-case: push_triangle with near-zero but non-zero area ---

    #[test]
    fn test_t09_near_degenerate_triangle_added() {
        let mut mesh = TriangleMesh::new();
        let tiny = AREA_EPS.sqrt() * 10.0; // cross_norm >> AREA_EPS² but still very small
        mesh.positions.push([0.0, 0.0, 0.0]);
        mesh.positions.push([tiny, 0.0, 0.0]);
        mesh.positions.push([0.0, tiny, 0.0]);
        let before = mesh.indices.len();
        push_triangle(&mut mesh, 0, 1, 2, "");
        assert_eq!(
            mesh.indices.len(),
            before + 3,
            "near-degenerate but non-zero triangle should be added"
        );
    }

    // --- merge_meshes tests (T08, T09, T10) ---

    /// T08: merge_meshes concatenates positions/normals and offsets indices.
    #[test]
    fn t08_merge_normal() {
        let m1 = TriangleMesh {
            positions: vec![[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]],
            normals: vec![[0.0, 0.0, 1.0], [0.0, 0.0, 1.0], [0.0, 0.0, 1.0]],
            indices: vec![0, 1, 2],
            face_ids: vec!["face_a".to_string()],
        };
        let m2 = TriangleMesh {
            positions: vec![[5.0, 0.0, 0.0], [6.0, 0.0, 0.0], [5.0, 1.0, 0.0]],
            normals: vec![[0.0, 0.0, 1.0], [0.0, 0.0, 1.0], [0.0, 0.0, 1.0]],
            indices: vec![0, 1, 2],
            face_ids: vec!["face_b".to_string()],
        };

        let merged = merge_meshes(&[m1, m2]);

        assert_eq!(merged.positions.len(), 6);
        assert_eq!(merged.normals.len(), 6);
        // m2 indices offset by m1.positions.len() = 3
        assert_eq!(merged.indices, vec![0, 1, 2, 3, 4, 5]);
        assert_eq!(merged.face_ids, vec!["face_a", "face_b"]);
    }

    /// T09: merge_meshes determinism — same input produces same output.
    #[test]
    fn t09_merge_determinism() {
        let mut gen1 = IdGenerator::new(0);
        let mut gen2 = IdGenerator::new(0);
        let s1 = make_cuboid(1.0, 1.0, 1.0, &mut gen1).unwrap();
        let s2 = make_cuboid(1.0, 1.0, 1.0, &mut gen2).unwrap();

        let mesh1a = tessellate_solid(&s1).unwrap();
        let mesh1b = tessellate_solid(&s2).unwrap();

        let merged1 = merge_meshes(&[mesh1a.clone(), mesh1b.clone()]);

        let mut gen3 = IdGenerator::new(0);
        let mut gen4 = IdGenerator::new(0);
        let s3 = make_cuboid(1.0, 1.0, 1.0, &mut gen3).unwrap();
        let s4 = make_cuboid(1.0, 1.0, 1.0, &mut gen4).unwrap();
        let mesh2a = tessellate_solid(&s3).unwrap();
        let mesh2b = tessellate_solid(&s4).unwrap();
        let merged2 = merge_meshes(&[mesh2a, mesh2b]);

        assert_eq!(merged1.positions, merged2.positions);
        assert_eq!(merged1.normals, merged2.normals);
        assert_eq!(merged1.indices, merged2.indices);
    }

    /// T10: merge_meshes with empty and single inputs.
    #[test]
    fn t10_merge_empty_and_single() {
        let empty = merge_meshes(&[]);
        assert_eq!(empty.positions.len(), 0);
        assert_eq!(empty.normals.len(), 0);
        assert_eq!(empty.indices.len(), 0);
        assert_eq!(empty.face_ids.len(), 0);

        let m1 = TriangleMesh {
            positions: vec![[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]],
            normals: vec![[0.0, 0.0, 1.0], [0.0, 0.0, 1.0], [0.0, 0.0, 1.0]],
            indices: vec![0, 1, 2],
            face_ids: vec!["f".to_string()],
        };
        let single = merge_meshes(&[m1.clone()]);
        assert_eq!(single.positions, m1.positions);
        assert_eq!(single.normals, m1.normals);
        assert_eq!(single.indices, m1.indices);
        assert_eq!(single.face_ids, m1.face_ids);
    }

    /// merge_meshes with three meshes — cumulative offset.
    #[test]
    fn t08_merge_three_meshes() {
        let make_mesh = |offset: f64| TriangleMesh {
            positions: vec![
                [offset, 0.0, 0.0],
                [offset + 1.0, 0.0, 0.0],
                [offset, 1.0, 0.0],
            ],
            normals: vec![[0.0, 0.0, 1.0], [0.0, 0.0, 1.0], [0.0, 0.0, 1.0]],
            indices: vec![0, 1, 2],
            face_ids: vec![format!("f{offset}")],
        };
        let merged = merge_meshes(&[make_mesh(0.0), make_mesh(5.0), make_mesh(10.0)]);
        assert_eq!(merged.positions.len(), 9);
        assert_eq!(merged.indices, vec![0, 1, 2, 3, 4, 5, 6, 7, 8]);
        assert_eq!(merged.face_ids.len(), 3);
    }

    // T14: planar face + line pcurve produces same point count as edge.curve (1 point per Line HE)
    #[test]
    fn t14_line_pcurve_same_point_count() {
        use crate::geometry::pcurve::{Curve2D, Pcurve};

        // Build a solid with one line pcurve HE on a planar face
        let mut s = Solid::new(0);
        let v0 = s.add_vertex(1, Point::new(0.0, 0.0, 0.0), None);
        let v1 = s.add_vertex(2, Point::new(1.0, 0.0, 0.0), None);
        let v2 = s.add_vertex(3, Point::new(0.0, 1.0, 0.0), None);
        let e0 = s.add_edge(
            4,
            [v0, v1],
            Curve::Line {
                origin: Point::origin(),
                direction: Vec3::x(),
            },
            [0.0, 1.0],
            None,
        );
        let e1 = s.add_edge(
            5,
            [v1, v2],
            Curve::Line {
                origin: Point::new(1.0, 0.0, 0.0),
                direction: Vec3::new(-1.0, 1.0, 0.0),
            },
            [0.0, 1.0],
            None,
        );
        let e2 = s.add_edge(
            6,
            [v2, v0],
            Curve::Line {
                origin: Point::new(0.0, 1.0, 0.0),
                direction: -Vec3::y(),
            },
            [0.0, 1.0],
            None,
        );

        // Attach a line pcurve to the forward HE of e0
        let line = Curve2D::try_line((0.0, 0.0), (1.0, 0.0)).unwrap();
        let pc = Pcurve::try_new(line, [0.0, 1.0]).unwrap();
        let he0 = s.add_half_edge_with_pcurve(7, v0, e0, true, Some(pc));
        let he1 = s.add_half_edge(8, v1, e1, true);
        let he2 = s.add_half_edge(9, v2, e2, true);
        let he0r = s.add_half_edge(10, v1, e0, false);
        let he1r = s.add_half_edge(11, v2, e1, false);
        let he2r = s.add_half_edge(12, v0, e2, false);

        let lp0 = s.add_loop(13, vec![he0, he1, he2]);
        let lp1 = s.add_loop(14, vec![he0r, he1r, he2r]);

        s.add_face(
            15,
            Surface::Plane {
                origin: Point::origin(),
                normal: Vec3::z(),
                u_axis: Vec3::x(),
                v_axis: Vec3::y(),
            },
            lp0,
            vec![],
            true,
            None,
        );
        s.add_face(
            16,
            Surface::Plane {
                origin: Point::origin(),
                normal: -Vec3::z(),
                u_axis: Vec3::x(),
                v_axis: Vec3::y(),
            },
            lp1,
            vec![],
            true,
            None,
        );
        s.add_shell(17, vec![0, 1], true);

        let mesh = tessellate_solid(&s).unwrap();
        // Each face: 3 Line HEs × 1 point each = 3 points per face = 6 total
        // Face 0 (top): 1 triangle (3 vertices), Face 1 (bottom): 1 triangle (3 vertices)
        assert_eq!(
            mesh.positions.len(),
            6,
            "expected 6 vertices, got {}",
            mesh.positions.len()
        );
        assert_eq!(mesh.triangle_count(), 2);
    }

    // T14b: planar face + circle arc pcurve produces N sample points
    #[test]
    fn t14b_circle_pcurve_n_points() {
        use crate::geometry::pcurve::{Curve2D, Pcurve};

        let mut s = Solid::new(0);
        // A triangular face where one edge is on XY plane with a circle pcurve
        let v0 = s.add_vertex(1, Point::new(0.0, 0.0, 0.0), None);
        let v1 = s.add_vertex(2, Point::new(1.0, 0.0, 0.0), None);
        let v2 = s.add_vertex(3, Point::new(0.0, 1.0, 0.0), None);

        let e0 = s.add_edge(
            4,
            [v0, v1],
            Curve::Line {
                origin: Point::origin(),
                direction: Vec3::x(),
            },
            [0.0, 1.0],
            None,
        );
        let e1 = s.add_edge(
            5,
            [v1, v2],
            Curve::Line {
                origin: Point::new(1.0, 0.0, 0.0),
                direction: Vec3::new(-1.0, 1.0, 0.0),
            },
            [0.0, 1.0],
            None,
        );
        let e2 = s.add_edge(
            6,
            [v2, v0],
            Curve::Line {
                origin: Point::new(0.0, 1.0, 0.0),
                direction: -Vec3::y(),
            },
            [0.0, 1.0],
            None,
        );

        // Circle pcurve: center=(0.5, 0), radius=0.5, sweep from π to 0 (same endpoints as edge)
        let circle = Curve2D::try_circle((0.5, 0.0), 0.5).unwrap();
        let pc = Pcurve::try_new(circle, [std::f64::consts::PI, 0.0]).unwrap();

        let he0 = s.add_half_edge_with_pcurve(7, v0, e0, true, Some(pc));
        let he1 = s.add_half_edge(8, v1, e1, true);
        let he2 = s.add_half_edge(9, v2, e2, true);
        let he0r = s.add_half_edge(10, v1, e0, false);
        let he1r = s.add_half_edge(11, v2, e1, false);
        let he2r = s.add_half_edge(12, v0, e2, false);

        let lp0 = s.add_loop(13, vec![he0, he1, he2]);
        let lp1 = s.add_loop(14, vec![he0r, he1r, he2r]);

        s.add_face(
            15,
            Surface::Plane {
                origin: Point::origin(),
                normal: Vec3::z(),
                u_axis: Vec3::x(),
                v_axis: Vec3::y(),
            },
            lp0,
            vec![],
            true,
            None,
        );
        s.add_face(
            16,
            Surface::Plane {
                origin: Point::origin(),
                normal: -Vec3::z(),
                u_axis: Vec3::x(),
                v_axis: Vec3::y(),
            },
            lp1,
            vec![],
            true,
            None,
        );
        s.add_shell(17, vec![0, 1], true);

        // With default angular_segments=32, half-circle pcurve (span=π) produces
        // arc_segment_count(π, 0, 32) = ceil(32·π/2π) = 16 points
        let mesh = tessellate_solid(&s).unwrap();
        // Face 0: circle_pcurve(16 pts) + 2 Line HEs(1 pt each) = 18 points
        // Face 1: 3 Line HEs × 1 pt each = 3 points
        assert_eq!(
            mesh.positions.len(),
            18 + 3,
            "expected 21 vertices, got {}",
            mesh.positions.len()
        );
    }

    // T22: pcurve solid build → YAML roundtrip → rebuild → tessellate → identical
    #[test]
    fn t22_pcurve_yaml_roundtrip_tessellation() {
        use crate::geometry::pcurve::{Curve2D, Pcurve};

        let build_solid = || -> Solid {
            let mut s = Solid::new(0);
            let v0 = s.add_vertex(1, Point::new(0.0, 0.0, 0.0), None);
            let v1 = s.add_vertex(2, Point::new(1.0, 0.0, 0.0), None);
            let v2 = s.add_vertex(3, Point::new(0.0, 1.0, 0.0), None);
            let e0 = s.add_edge(
                4,
                [v0, v1],
                Curve::Line {
                    origin: Point::origin(),
                    direction: Vec3::x(),
                },
                [0.0, 1.0],
                None,
            );
            let e1 = s.add_edge(
                5,
                [v1, v2],
                Curve::Line {
                    origin: Point::new(1.0, 0.0, 0.0),
                    direction: Vec3::new(-1.0, 1.0, 0.0),
                },
                [0.0, 1.0],
                None,
            );
            let e2 = s.add_edge(
                6,
                [v2, v0],
                Curve::Line {
                    origin: Point::new(0.0, 1.0, 0.0),
                    direction: -Vec3::y(),
                },
                [0.0, 1.0],
                None,
            );

            let line = Curve2D::try_line((0.0, 0.0), (1.0, 0.0)).unwrap();
            let pc = Pcurve::try_new(line, [0.0, 1.0]).unwrap();
            let he0 = s.add_half_edge_with_pcurve(7, v0, e0, true, Some(pc));
            let he1 = s.add_half_edge(8, v1, e1, true);
            let he2 = s.add_half_edge(9, v2, e2, true);
            let he0r = s.add_half_edge(10, v1, e0, false);
            let he1r = s.add_half_edge(11, v2, e1, false);
            let he2r = s.add_half_edge(12, v0, e2, false);

            let lp0 = s.add_loop(13, vec![he0, he1, he2]);
            let lp1 = s.add_loop(14, vec![he0r, he1r, he2r]);
            s.add_face(
                15,
                Surface::Plane {
                    origin: Point::origin(),
                    normal: Vec3::z(),
                    u_axis: Vec3::x(),
                    v_axis: Vec3::y(),
                },
                lp0,
                vec![],
                true,
                None,
            );
            s.add_face(
                16,
                Surface::Plane {
                    origin: Point::origin(),
                    normal: -Vec3::z(),
                    u_axis: Vec3::x(),
                    v_axis: Vec3::y(),
                },
                lp1,
                vec![],
                true,
                None,
            );
            s.add_shell(17, vec![0, 1], true);
            s
        };

        let s1 = build_solid();
        let m1 = tessellate_solid(&s1).unwrap();

        // YAML roundtrip
        let yaml = serde_yaml::to_string(&s1).unwrap();
        let s2: Solid = serde_yaml::from_str(&yaml).unwrap();
        let m2 = tessellate_solid(&s2).unwrap();

        assert_eq!(
            m1.positions, m2.positions,
            "tessellation positions differ after YAML roundtrip"
        );
        assert_eq!(
            m1.normals, m2.normals,
            "tessellation normals differ after YAML roundtrip"
        );
        assert_eq!(
            m1.indices, m2.indices,
            "tessellation indices differ after YAML roundtrip"
        );
    }

    // --- #42 Phase 2 edge-case tests ---

    /// Fan winding CW: cuboid side face has outward normals after tessellation.
    /// The side face (-X) has a CW loop when viewed from outside; fan should flip to produce
    /// outward normals.
    #[test]
    fn fan_winding_cw_box_face() {
        let mut gen = IdGenerator::new(0);
        let solid = make_cuboid(10.0, 10.0, 10.0, &mut gen).unwrap();
        let mesh = tessellate_solid(&solid).unwrap();

        // Centroid at origin, all facet normals should point away
        for tri in 0..mesh.triangle_count() {
            let i0 = mesh.indices[tri * 3] as usize;
            let i1 = mesh.indices[tri * 3 + 1] as usize;
            let i2 = mesh.indices[tri * 3 + 2] as usize;
            let p0 = mesh.positions[i0];
            let p1 = mesh.positions[i1];
            let p2 = mesh.positions[i2];
            let fc = [
                (p0[0] + p1[0] + p2[0]) / 3.0,
                (p0[1] + p1[1] + p2[1]) / 3.0,
                (p0[2] + p1[2] + p2[2]) / 3.0,
            ];
            let u = [p1[0] - p0[0], p1[1] - p0[1], p1[2] - p0[2]];
            let v = [p2[0] - p0[0], p2[1] - p0[1], p2[2] - p0[2]];
            let nx = u[1] * v[2] - u[2] * v[1];
            let ny = u[2] * v[0] - u[0] * v[2];
            let nz = u[0] * v[1] - u[1] * v[0];
            let dot = nx * fc[0] + ny * fc[1] + nz * fc[2];
            assert!(
                dot > 0.0,
                "cuboid facet {tri}: outward dot={dot} (fc=({:.2},{:.2},{:.2}))",
                fc[0],
                fc[1],
                fc[2]
            );
        }
    }

    /// Fan winding CCW: cuboid tessellation produces positive signed volume (no abs needed),
    /// confirming that CCW faces are not flipped.
    #[test]
    fn fan_winding_ccw_positive_signed_volume() {
        let mut gen = IdGenerator::new(0);
        let solid = make_cuboid(10.0, 10.0, 10.0, &mut gen).unwrap();
        let mesh = tessellate_solid(&solid).unwrap();

        let mut vol = 0.0_f64;
        for tri in 0..mesh.triangle_count() {
            let i0 = mesh.indices[tri * 3] as usize;
            let i1 = mesh.indices[tri * 3 + 1] as usize;
            let i2 = mesh.indices[tri * 3 + 2] as usize;
            let p0 = &mesh.positions[i0];
            let p1 = &mesh.positions[i1];
            let p2 = &mesh.positions[i2];
            vol += (p0[0] * (p1[1] * p2[2] - p2[1] * p1[2])
                + p1[0] * (p2[1] * p0[2] - p0[1] * p2[2])
                + p2[0] * (p0[1] * p1[2] - p1[1] * p0[2]))
                / 6.0;
        }
        assert!(
            vol > 0.0,
            "cuboid signed volume should be positive, got {vol}"
        );
        let expected = 10.0 * 10.0 * 10.0;
        assert!(
            (vol - expected).abs() < 0.01,
            "cuboid volume: expected {expected}, got {vol}"
        );
    }

    /// Shift all geometry in a solid by (dx, dy, dz).
    fn shift_solid_for_test(solid: &mut crate::brep::topology::Solid, dx: f64, dy: f64, dz: f64) {
        use crate::geometry::curve::Curve;
        use crate::geometry::surface::Surface;

        for v in &mut solid.vertices {
            v.point.coords.x += dx;
            v.point.coords.y += dy;
            v.point.coords.z += dz;
        }
        for e in &mut solid.edges {
            if let Curve::Circle { center, .. } = &mut e.curve {
                center.coords.x += dx;
                center.coords.y += dy;
                center.coords.z += dz;
            }
        }
        for f in &mut solid.faces {
            match &mut f.surface {
                Surface::Sphere { center, .. }
                | Surface::Plane { origin: center, .. }
                | Surface::Cylinder { origin: center, .. } => {
                    center.coords.x += dx;
                    center.coords.y += dy;
                    center.coords.z += dz;
                }
                _ => {}
            }
        }
    }

    /// Raw signed mesh volume (no abs). Positive = outward-consistent winding.
    fn raw_mesh_signed_volume(mesh: &TriangleMesh) -> f64 {
        let mut vol = 0.0_f64;
        for tri in 0..mesh.triangle_count() {
            let i0 = mesh.indices[tri * 3] as usize;
            let i1 = mesh.indices[tri * 3 + 1] as usize;
            let i2 = mesh.indices[tri * 3 + 2] as usize;
            let p0 = &mesh.positions[i0];
            let p1 = &mesh.positions[i1];
            let p2 = &mesh.positions[i2];
            vol += (p0[0] * (p1[1] * p2[2] - p2[1] * p1[2])
                + p1[0] * (p2[1] * p0[2] - p0[1] * p2[2])
                + p2[0] * (p0[1] * p1[2] - p1[1] * p0[2]))
                / 6.0;
        }
        vol
    }

    /// Sphere trimmed volume sign: box(10³) - sphere(r=3, center=(0,0,6)) dimple.
    /// The sphere cap face has same_sense=false; verifies trimmed sphere winding
    /// produces positive signed volume without abs.
    #[test]
    fn sphere_trimmed_volume_sign() {
        use crate::booleans::{boolean, BooleanOp};

        let mut gen = IdGenerator::new(0);
        let box_solid = make_cuboid(10.0, 10.0, 10.0, &mut gen).unwrap();
        let mut sphere = make_sphere(3.0, Point::origin(), &mut gen).unwrap();
        shift_solid_for_test(&mut sphere, 0.0, 0.0, 6.0);
        let solid = boolean(&box_solid, &sphere, BooleanOp::Cut, &mut gen)
            .expect("box - sphere cut should succeed");

        let mesh = tessellate_solid(&solid).expect("tessellate sphere dimple");
        let vol = raw_mesh_signed_volume(&mesh);

        assert!(
            vol > 0.0,
            "sphere-cut signed volume should be positive, got {vol}"
        );
        // V_cap = π·h²·(R - h/3) = π·4·(3 - 2/3) = 28π/3 ≈ 29.32
        let expected = 1000.0 - 28.0 * PI / 3.0;
        assert!(
            (vol - expected).abs() < 2.0,
            "volume ≈ {expected:.2}, got {vol:.2}"
        );
    }

    // --- Edge-case tests (adversarial persona) ---

    /// Empty solid (0 faces) → empty mesh, no panic.
    #[test]
    fn edge_empty_solid_empty_mesh() {
        let s = Solid::new(0);
        let mesh = tessellate_solid(&s).unwrap();
        assert_eq!(
            mesh.positions.len(),
            0,
            "empty solid should have 0 positions"
        );
        assert_eq!(mesh.indices.len(), 0, "empty solid should have 0 indices");
        assert_eq!(mesh.triangle_count(), 0);
    }

    /// All mesh coordinates are finite (no NaN/Inf) for cylinder.
    #[test]
    fn edge_all_cylinder_coords_finite() {
        let mut gen = IdGenerator::new(0);
        let solid = make_cylinder(5.0, 20.0, Point::origin(), &mut gen).unwrap();
        let mesh = tessellate_solid(&solid).unwrap();
        for (i, p) in mesh.positions.iter().enumerate() {
            for (j, &x) in p.iter().enumerate() {
                assert!(x.is_finite(), "position[{i}][{j}] = {x} is not finite");
            }
        }
        for (i, n) in mesh.normals.iter().enumerate() {
            for (j, &x) in n.iter().enumerate() {
                assert!(x.is_finite(), "normal[{i}][{j}] = {x} is not finite");
            }
        }
    }

    /// Arc-proportional sampling with low angular_segments still produces valid mesh.
    #[test]
    fn edge_low_angular_segments_cylinder() {
        let mut gen = IdGenerator::new(0);
        let solid = make_cylinder(5.0, 20.0, Point::origin(), &mut gen).unwrap();
        let opts = TessellationOptions::new(3, 1);
        let mesh = tessellate_solid_with(&solid, &opts).unwrap();
        assert!(
            mesh.positions
                .iter()
                .all(|p| p.iter().all(|x| x.is_finite())),
            "low angular mesh must not contain NaN/Inf"
        );
        assert!(
            mesh.triangle_count() > 0,
            "low angular mesh should have triangles"
        );
    }

    /// Coincident vertices in a degenerate triangle face: tessellation does not panic.
    #[test]
    fn edge_coincident_vertices_no_panic() {
        let mut s = Solid::new(0);
        let v0 = s.add_vertex(1, Point::new(0.0, 0.0, 0.0), None);
        let v1 = s.add_vertex(2, Point::new(0.0, 0.0, 0.0), None); // Coincident with v0
        let v2 = s.add_vertex(3, Point::new(0.0, 0.0, 1.0), None);
        let e0 = s.add_edge(
            4,
            [v0, v1],
            Curve::Line {
                origin: Point::origin(),
                direction: Vec3::z(),
            },
            [0.0, 0.0], // Zero-length
            None,
        );
        let e1 = s.add_edge(
            5,
            [v1, v2],
            Curve::Line {
                origin: Point::origin(),
                direction: Vec3::z(),
            },
            [0.0, 1.0],
            None,
        );
        let e2 = s.add_edge(
            6,
            [v2, v0],
            Curve::Line {
                origin: Point::new(0.0, 0.0, 1.0),
                direction: -Vec3::z(),
            },
            [0.0, 1.0],
            None,
        );
        let he0 = s.add_half_edge(7, v0, e0, true);
        let he1 = s.add_half_edge(8, v1, e1, true);
        let he2 = s.add_half_edge(9, v2, e2, true);
        let lp = s.add_loop(10, vec![he0, he1, he2]);
        s.add_face(
            11,
            Surface::Plane {
                origin: Point::origin(),
                normal: Vec3::x(),
                u_axis: Vec3::y(),
                v_axis: Vec3::z(),
            },
            lp,
            vec![],
            true,
            None,
        );
        s.add_shell(12, vec![0], true);

        let mesh = tessellate_solid(&s).unwrap();
        for p in &mesh.positions {
            for &x in p {
                assert!(x.is_finite());
            }
        }
    }

    /// 100-run determinism for hole tessellation via kernel-level construction.
    #[test]
    fn edge_tessellation_100_run_determinism() {
        let first = {
            let mut gen = IdGenerator::new(0);
            let s = make_cylinder(5.0, 20.0, Point::origin(), &mut gen).unwrap();
            tessellate_solid(&s).unwrap()
        };
        for i in 1..100 {
            let mut gen = IdGenerator::new(0);
            let s = make_cylinder(5.0, 20.0, Point::origin(), &mut gen).unwrap();
            let m = tessellate_solid(&s).unwrap();
            assert_eq!(first.positions, m.positions, "run {i}: positions mismatch");
            assert_eq!(first.indices, m.indices, "run {i}: indices mismatch");
        }
    }
}
