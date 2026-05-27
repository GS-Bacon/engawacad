OpenAI Codex v0.121.0 (research preview)
--------
workdir: /home/bacon/mycad
model: gpt-5.4
provider: openai
approval: never
sandbox: read-only
reasoning effort: xhigh
reasoning summaries: none
session id: 019e5d42-cfc7-7e83-a27f-041a4132c6e3
--------
user
commit 572a245
exec
/bin/bash -lc 'git show --stat --oneline 572a245 --' in /home/bacon/mycad
 succeeded in 0ms:
572a245 fix(tessellation): guard against non-canonical cylinder faces and zero-segment NaN
 crates/mycad-kernel/src/tessellation/mod.rs | 81 +++++++++++++++++++++++++++--
 1 file changed, 77 insertions(+), 4 deletions(-)

exec
/bin/bash -lc 'git show --format=medium --unified=80 572a245 -- crates/mycad-kernel/src/tessellation/mod.rs' in /home/bacon/mycad
 succeeded in 0ms:
commit 572a245caa7428946206c816c36c75258dd4c7a2
Author: GS-Bacon <bacon0817.ix@gmail.com>
Date:   Mon May 25 03:51:31 2026 +0000

    fix(tessellation): guard against non-canonical cylinder faces and zero-segment NaN
    
    - Add full-revolution check in tessellate_face_uv_grid: at least one Circle
      edge must span ≈ 2π, otherwise return TrimmedFaceUnsupported. Prevents
      silently incorrect meshes for trimmed or non-canonical cylinder faces.
    - Clamp opts.angular_segments.max(3) and opts.axial_segments.max(1) at point
      of use in both tessellate_face_uv_grid and tessellate_face_fan so that
      directly-constructed or deserialized TessellationOptions with zero fields
      cannot produce NaN vertices.
    - Add regression tests: test_zero_segments_clamped_at_use,
      test_partial_revolution_cylinder_face_errors.
    
    Co-Authored-By: Claude Sonnet 4.6 <noreply@anthropic.com>

diff --git a/crates/mycad-kernel/src/tessellation/mod.rs b/crates/mycad-kernel/src/tessellation/mod.rs
index 166f24e..11ed22c 100644
--- a/crates/mycad-kernel/src/tessellation/mod.rs
+++ b/crates/mycad-kernel/src/tessellation/mod.rs
@@ -1,288 +1,304 @@
 pub mod stl;
 pub use stl::to_ascii_stl;
 
 use crate::brep::topology::Solid;
+use crate::geometry::curve::Curve;
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
 
-    let loop_points = collect_loop_points(solid, outer_loop, opts.angular_segments)?;
+    let loop_points = collect_loop_points(solid, outer_loop, opts.angular_segments.max(3))?;
 
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
 
+    let outer_loop = &solid.loops[face.outer_loop];
+
+    // Guard: reject non-canonical cylinder faces (trimmed in u, seam not at u=0).
+    // At least one Circle edge must span ≈ 2π to qualify as a full-revolution patch.
+    let has_full_revolution = outer_loop.half_edges.iter().any(|&he_idx| {
+        let he = &solid.half_edges[he_idx];
+        let edge = &solid.edges[he.edge];
+        matches!(edge.curve, Curve::Circle { .. })
+            && (edge.t_range[1] - edge.t_range[0]).abs() >= 2.0 * PI - 0.1
+    });
+    if !has_full_revolution {
+        return Err(TessellationError::TrimmedFaceUnsupported);
+    }
+
     // Determine v range from loop corner vertices (start_vertex of each HE).
     // UvGridFullPatch assumes a full 2π revolution in u starting at u=0;
     // we do not sample the circle arcs here — the grid covers [0,2π] directly.
-    let outer_loop = &solid.loops[face.outer_loop];
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
 
-    let n_u = opts.angular_segments;
-    let n_v = opts.axial_segments;
+    // Clamp at point of use — callers may bypass TessellationOptions::new() via public fields
+    // or deserialization, so we defensively enforce minimum viable values here.
+    let n_u = opts.angular_segments.max(3);
+    let n_v = opts.axial_segments.max(1);
 
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
@@ -418,81 +434,138 @@ mod tests {
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
         let solid = make_cuboid(1.0, 1.0, 1.0, &mut id_gen);
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
 
         let solid1 = make_cuboid(2.0, 3.0, 4.0, &mut id_gen1);
         let solid2 = make_cuboid(2.0, 3.0, 4.0, &mut id_gen2);
 
         let mesh1 = tessellate_solid(&solid1).unwrap();
         let mesh2 = tessellate_solid(&solid2).unwrap();
 
         assert_eq!(mesh1.positions, mesh2.positions);
         assert_eq!(mesh1.normals, mesh2.normals);
         assert_eq!(mesh1.indices, mesh2.indices);
     }
+    /// Directly-constructed TessellationOptions with 0 segments should not produce NaN vertices
+    /// (public fields bypass new() clamping, so tessellate_solid_with must guard at point of use).
+    #[test]
+    fn test_zero_segments_clamped_at_use() {
+        let mut gen = IdGenerator::new(0);
+        let solid = make_cylinder(5.0, 20.0, &mut gen).unwrap();
+        let opts = TessellationOptions {
+            angular_segments: 0,
+            axial_segments: 0,
+        };
+        let mesh = tessellate_solid_with(&solid, &opts).unwrap();
+        // Clamped to angular=3, axial=1: lateral=3*2=6, caps=(3-2)*2=2 → 8
+        assert_eq!(mesh.triangle_count(), 8);
+        assert!(
+            mesh.positions
+                .iter()
+                .all(|p| p.iter().all(|x| x.is_finite())),
+            "mesh must not contain NaN/inf vertices"
+        );
+    }
+
+    /// Non-full-revolution cylinder face (arc < 2π) must return TrimmedFaceUnsupported.
+    #[test]
+    fn test_partial_revolution_cylinder_face_errors() {
+        use crate::brep::topology::Solid as S;
+        let mut s = S::new(0);
+        let v0 = s.add_vertex(1, Point::new(5.0, 0.0, 0.0));
+        let e0 = s.add_edge(
+            2,
+            [v0, v0],
+            Curve::Circle {
+                center: Point::origin(),
+                normal: Vec3::z(),
+                radius: 5.0,
+            },
+            [0.0, PI], // half revolution
+        );
+        let he0 = s.add_half_edge(3, v0, e0, true);
+        let lp = s.add_loop(4, vec![he0]);
+        s.add_face(
+            5,
+            Surface::Cylinder {
+                origin: Point::origin(),
+                axis: Vec3::z(),
+                radius: 5.0,
+            },
+            lp,
+            vec![],
+            true,
+        );
+        s.add_shell(6, vec![0], true);
+        let result = tessellate_solid(&s);
+        assert!(matches!(
+            result,
+            Err(TessellationError::TrimmedFaceUnsupported)
+        ));
+    }
 }

exec
/bin/bash -lc "rg -n \"UvGridFullPatch|tessellation_strategy|Cylinder\" crates/mycad-kernel/src -g'*.rs'" in /home/bacon/mycad
 succeeded in 0ms:
crates/mycad-kernel/src/tessellation/mod.rs:97:        let strategy = face.surface.tessellation_strategy();
crates/mycad-kernel/src/tessellation/mod.rs:102:            TessellationStrategy::UvGridFullPatch => {
crates/mycad-kernel/src/tessellation/mod.rs:206:    // UvGridFullPatch assumes a full 2π revolution in u starting at u=0;
crates/mycad-kernel/src/tessellation/mod.rs:293:            Surface::Cylinder { .. } => "cylinder",
crates/mycad-kernel/src/tessellation/mod.rs:555:            Surface::Cylinder {
crates/mycad-kernel/src/primitives/cylinder.rs:99:    // Lateral face: Surface::Cylinder, loop = 4 HE
crates/mycad-kernel/src/primitives/cylinder.rs:112:        Surface::Cylinder {
crates/mycad-kernel/src/primitives/cylinder.rs:287:        // Lateral surface: Cylinder
crates/mycad-kernel/src/primitives/cylinder.rs:289:            Surface::Cylinder {
crates/mycad-kernel/src/primitives/cylinder.rs:295:            _ => panic!("lateral face should be Cylinder surface"),
crates/mycad-kernel/src/primitives/cylinder.rs:361:    /// Verify that cylinder surface reports UvGridFullPatch strategy.
crates/mycad-kernel/src/primitives/cylinder.rs:363:    fn test_cylinder_surface_tessellation_strategy() {
crates/mycad-kernel/src/primitives/cylinder.rs:364:        let s = Surface::Cylinder {
crates/mycad-kernel/src/primitives/cylinder.rs:370:            s.tessellation_strategy(),
crates/mycad-kernel/src/primitives/cylinder.rs:371:            TessellationStrategy::UvGridFullPatch
crates/mycad-kernel/src/geometry/surface.rs:11:    UvGridFullPatch,
crates/mycad-kernel/src/geometry/surface.rs:27:    Cylinder {
crates/mycad-kernel/src/geometry/surface.rs:53:            Surface::Cylinder {
crates/mycad-kernel/src/geometry/surface.rs:87:            Surface::Cylinder { axis, .. } => {
crates/mycad-kernel/src/geometry/surface.rs:137:            Surface::Cylinder { origin, axis, .. } => {
crates/mycad-kernel/src/geometry/surface.rs:174:    pub fn tessellation_strategy(&self) -> TessellationStrategy {
crates/mycad-kernel/src/geometry/surface.rs:177:            Surface::Cylinder { .. } => TessellationStrategy::UvGridFullPatch,
crates/mycad-kernel/src/geometry/surface.rs:208:        let surface = Surface::Cylinder {

exec
/bin/bash -lc "sed -n '1,240p' crates/mycad-kernel/src/geometry/surface.rs" in /home/bacon/mycad
 succeeded in 0ms:
use super::math::orthonormal_basis;
use super::{Point, Vec3};
use serde::{Deserialize, Serialize};

/// Strategy for tessellating a face on a given surface type.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TessellationStrategy {
    /// Fan from first boundary vertex (convex planar face).
    BoundaryFan,
    /// Full UV grid patch (untrimmed periodic/rectangular face).
    UvGridFullPatch,
    /// Not yet supported by the tessellator.
    Unsupported,
}

/// A geometric surface in 3D space.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Surface {
    /// An infinite plane.
    Plane {
        origin: Point,
        normal: Vec3,
        u_axis: Vec3,
        v_axis: Vec3,
    },
    /// A cylindrical surface.
    Cylinder {
        origin: Point,
        axis: Vec3,
        radius: f64,
    },
    /// A spherical surface.
    Sphere { center: Point, radius: f64 },
    /// A conical surface.
    Cone {
        apex: Point,
        axis: Vec3,
        half_angle: f64,
    },
}

impl Surface {
    /// Evaluate the surface at parameters (u, v).
    pub fn evaluate(&self, u: f64, v: f64) -> Point {
        match self {
            Surface::Plane {
                origin,
                u_axis,
                v_axis,
                ..
            } => origin + u * u_axis + v * v_axis,

            Surface::Cylinder {
                origin,
                axis,
                radius,
            } => {
                let (bu, bv) = orthonormal_basis(axis);
                origin + *radius * (u.cos() * bu + u.sin() * bv) + v * axis.normalize()
            }

            Surface::Sphere { center, radius } => {
                // u = longitude [0, 2π), v = latitude [-π/2, π/2]
                let x = radius * v.cos() * u.cos();
                let y = radius * v.cos() * u.sin();
                let z = radius * v.sin();
                Point::new(center.x + x, center.y + y, center.z + z)
            }

            Surface::Cone {
                apex,
                axis,
                half_angle,
            } => {
                let (bu, bv) = orthonormal_basis(axis);
                let r = v * half_angle.tan();
                apex + v * axis.normalize() + r * (u.cos() * bu + u.sin() * bv)
            }
        }
    }

    /// Get the normal vector at parameters (u, v).
    pub fn normal_at(&self, u: f64, v: f64) -> Vec3 {
        match self {
            Surface::Plane { normal, .. } => normal.normalize(),

            Surface::Cylinder { axis, .. } => {
                let (bu, bv) = orthonormal_basis(axis);
                (u.cos() * bu + u.sin() * bv).normalize()
            }

            Surface::Sphere { center, .. } => {
                let p = self.evaluate(u, v);
                (p - center).normalize()
            }

            Surface::Cone {
                apex,
                axis,
                half_angle,
            } => {
                let p = self.evaluate(u, v);
                let a = axis.normalize();
                let to_point = p - apex;
                let along_axis = to_point.dot(&a) * a;
                let radial = to_point - along_axis;
                if radial.norm() < 1e-12 {
                    return a;
                }
                let radial_n = radial.normalize();
                (radial_n * half_angle.cos() - a * half_angle.sin()).normalize()
            }
        }
    }

    /// Inverse of [`Surface::evaluate`]: find the parameters (u, v) whose
    /// evaluation lands on (or nearest to) `p`.
    ///
    /// Used so that geometry-consuming algorithms (e.g. tessellation) can ask
    /// for the surface normal at a concrete 3D point without assuming the
    /// surface is planar / has a constant normal.
    pub fn uv_of(&self, p: &Point) -> (f64, f64) {
        match self {
            Surface::Plane {
                origin,
                u_axis,
                v_axis,
                ..
            } => {
                let d = p - origin;
                (
                    d.dot(u_axis) / u_axis.dot(u_axis),
                    d.dot(v_axis) / v_axis.dot(v_axis),
                )
            }

            Surface::Cylinder { origin, axis, .. } => {
                let a = axis.normalize();
                let (bu, bv) = orthonormal_basis(axis);
                let d = p - origin;
                let v = d.dot(&a);
                let radial = d - v * a;
                (radial.dot(&bv).atan2(radial.dot(&bu)), v)
            }

            Surface::Sphere { center, radius } => {
                let d = p - center;
                let v = (d.z / radius).clamp(-1.0, 1.0).asin();
                let u = d.y.atan2(d.x);
                (u, v)
            }

            Surface::Cone { apex, axis, .. } => {
                let a = axis.normalize();
                let (bu, bv) = orthonormal_basis(axis);
                let d = p - apex;
                let v = d.dot(&a);
                let radial = d - v * a;
                (radial.dot(&bv).atan2(radial.dot(&bu)), v)
            }
        }
    }

    /// Surface normal at a concrete 3D point lying on (or near) the surface.
    ///
    /// Convenience over [`Surface::uv_of`] + [`Surface::normal_at`] so callers
    /// never have to special-case planar vs curved surfaces.
    pub fn normal_at_point(&self, p: &Point) -> Vec3 {
        let (u, v) = self.uv_of(p);
        self.normal_at(u, v)
    }

    /// Return the tessellation strategy for this surface type.
    pub fn tessellation_strategy(&self) -> TessellationStrategy {
        match self {
            Surface::Plane { .. } => TessellationStrategy::BoundaryFan,
            Surface::Cylinder { .. } => TessellationStrategy::UvGridFullPatch,
            Surface::Sphere { .. } | Surface::Cone { .. } => TessellationStrategy::Unsupported,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use approx::assert_relative_eq;

    fn assert_uv_roundtrip(surface: &Surface, u: f64, v: f64) {
        let p = surface.evaluate(u, v);
        let (u2, v2) = surface.uv_of(&p);
        assert_relative_eq!(u, u2, epsilon = 1e-9);
        assert_relative_eq!(v, v2, epsilon = 1e-9);
    }

    #[test]
    fn test_uv_of_roundtrip_plane() {
        let surface = Surface::Plane {
            origin: Point::new(1.0, 2.0, 3.0),
            normal: Vec3::z(),
            u_axis: Vec3::x(),
            v_axis: Vec3::y(),
        };
        assert_uv_roundtrip(&surface, 0.7, -1.3);
    }

    #[test]
    fn test_uv_of_roundtrip_cylinder() {
        let surface = Surface::Cylinder {
            origin: Point::new(0.0, 0.0, 0.0),
            axis: Vec3::z(),
            radius: 2.5,
        };
        assert_uv_roundtrip(&surface, 0.9, 1.7);
    }

    #[test]
    fn test_uv_of_roundtrip_sphere() {
        let surface = Surface::Sphere {
            center: Point::new(-1.0, 0.5, 2.0),
            radius: 3.0,
        };
        assert_uv_roundtrip(&surface, 1.1, 0.6);
    }

    #[test]
    fn test_uv_of_roundtrip_cone() {
        let surface = Surface::Cone {
            apex: Point::new(0.0, 0.0, 0.0),
            axis: Vec3::z(),
            half_angle: 0.5,
        };
        assert_uv_roundtrip(&surface, 0.8, 2.0);
    }

    /// A curved surface must report different normals at different points —
    /// the property the tessellator now relies on instead of a single
    /// per-face normal.
    #[test]
    fn test_sphere_normal_varies_per_point() {
        let surface = Surface::Sphere {

exec
/bin/bash -lc "sed -n '1,220p' crates/mycad-kernel/src/primitives/cylinder.rs" in /home/bacon/mycad
 succeeded in 0ms:
use crate::brep::topology::{IdGenerator, Solid};
use crate::error::KernelError;
use crate::geometry::curve::Curve;
use crate::geometry::surface::Surface;
use crate::geometry::{Point, Vec3};

/// Create a cylinder B-rep solid.
///
/// Axis = +Z, bottom center at origin, bottom z=0 / top z=height.
/// Seam vertex at (0, radius, z) — angle u=0 in the orthonormal_basis(+Z) = (+Y, -X) frame.
pub fn make_cylinder(
    radius: f64,
    height: f64,
    id_gen: &mut IdGenerator,
) -> Result<Solid, KernelError> {
    if !radius.is_finite() || radius <= 0.0 {
        return Err(KernelError::InvalidParameter { kind: "radius" });
    }
    if !height.is_finite() || height <= 0.0 {
        return Err(KernelError::InvalidParameter { kind: "height" });
    }

    let mut solid = Solid::new(id_gen.next());

    // 2 vertices: seam at bottom and top
    let v_bot = solid.add_vertex(id_gen.next(), Point::new(0.0, radius, 0.0));
    let v_top = solid.add_vertex(id_gen.next(), Point::new(0.0, radius, height));

    // 3 edges
    let e_bot = solid.add_edge(
        id_gen.next(),
        [v_bot, v_bot],
        Curve::Circle {
            center: Point::origin(),
            normal: Vec3::z(),
            radius,
        },
        [0.0, 2.0 * std::f64::consts::PI],
    );

    let e_top = solid.add_edge(
        id_gen.next(),
        [v_top, v_top],
        Curve::Circle {
            center: Point::new(0.0, 0.0, height),
            normal: Vec3::z(),
            radius,
        },
        [0.0, 2.0 * std::f64::consts::PI],
    );

    let e_seam = solid.add_edge(
        id_gen.next(),
        [v_bot, v_top],
        Curve::Line {
            origin: Point::new(0.0, radius, 0.0),
            direction: Vec3::new(0.0, 0.0, height),
        },
        [0.0, 1.0],
    );

    // 3 faces: bottom cap, top cap, lateral
    let mut face_indices = Vec::with_capacity(3);

    // Bottom cap: Surface::Plane with normal -Z, loop = e_bot reversed (CW from above = outward -Z)
    let he_bot_rev = solid.add_half_edge(id_gen.next(), v_bot, e_bot, false);
    let loop_bot = solid.add_loop(id_gen.next(), vec![he_bot_rev]);
    let f_bot = solid.add_face(
        id_gen.next(),
        Surface::Plane {
            origin: Point::origin(),
            normal: -Vec3::z(),
            u_axis: Vec3::x(),
            v_axis: Vec3::y(),
        },
        loop_bot,
        vec![],
        true,
    );
    face_indices.push(f_bot);

    // Top cap: Surface::Plane with normal +Z, loop = e_top forward (CCW from above = outward +Z)
    let he_top_fwd = solid.add_half_edge(id_gen.next(), v_top, e_top, true);
    let loop_top = solid.add_loop(id_gen.next(), vec![he_top_fwd]);
    let f_top = solid.add_face(
        id_gen.next(),
        Surface::Plane {
            origin: Point::new(0.0, 0.0, height),
            normal: Vec3::z(),
            u_axis: Vec3::x(),
            v_axis: Vec3::y(),
        },
        loop_top,
        vec![],
        true,
    );
    face_indices.push(f_top);

    // Lateral face: Surface::Cylinder, loop = 4 HE
    //   seam up (fwd, start v_bot) → top circle (rev, start v_top) →
    //   seam down (rev, start v_top) → bot circle (fwd, start v_bot)
    let he_seam_up = solid.add_half_edge(id_gen.next(), v_bot, e_seam, true);
    let he_top_rev = solid.add_half_edge(id_gen.next(), v_top, e_top, false);
    let he_seam_dn = solid.add_half_edge(id_gen.next(), v_top, e_seam, false);
    let he_bot_fwd = solid.add_half_edge(id_gen.next(), v_bot, e_bot, true);
    let loop_lat = solid.add_loop(
        id_gen.next(),
        vec![he_seam_up, he_top_rev, he_seam_dn, he_bot_fwd],
    );
    let f_lat = solid.add_face(
        id_gen.next(),
        Surface::Cylinder {
            origin: Point::origin(),
            axis: Vec3::z(),
            radius,
        },
        loop_lat,
        vec![],
        true,
    );
    face_indices.push(f_lat);

    solid.add_shell(id_gen.next(), face_indices, true);

    Ok(solid)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::geometry::surface::TessellationStrategy;

    /// T01: Determinism — identical inputs produce identical solids (field-by-field).
    #[test]
    fn test_cylinder_deterministic() {
        let mut gen1 = IdGenerator::new(0);
        let mut gen2 = IdGenerator::new(0);
        let s1 = make_cylinder(5.0, 20.0, &mut gen1).unwrap();
        let s2 = make_cylinder(5.0, 20.0, &mut gen2).unwrap();

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
        }
        for (f1, f2) in s1.faces.iter().zip(s2.faces.iter()) {
            assert_eq!(f1.id, f2.id);
        }
    }

    /// T02: Topology counts.
    #[test]
    fn test_cylinder_topology() {
        let mut gen = IdGenerator::new(0);
        let s = make_cylinder(5.0, 20.0, &mut gen).unwrap();

        assert_eq!(s.vertices.len(), 2, "2 seam vertices");
        assert_eq!(
            s.edges.len(),
            3,
            "3 edges (bot circle, top circle, seam line)"
        );
        assert_eq!(s.faces.len(), 3, "3 faces (bottom cap, top cap, lateral)");
        assert_eq!(s.shells.len(), 1);
        assert!(s.shells[0].closed);
        assert_eq!(s.half_edges.len(), 6, "6 half-edges (3 edges × 2 HE each)");
        assert_eq!(s.loops.len(), 3, "3 loops (one per face)");
    }

    /// T03: Euler–Poincaré formula V - E + F = 2(S - H), S=1, H=0 → 2.
    #[test]
    fn test_cylinder_euler() {
        let mut gen = IdGenerator::new(0);
        let s = make_cylinder(5.0, 20.0, &mut gen).unwrap();

        let v = s.vertices.len();
        let e = s.edges.len();
        let f = s.faces.len();
        let shells = s.shells.len();
        // genus=0 for simple cylinder
        let euler = (v as i64) - (e as i64) + (f as i64);
        assert_eq!(
            euler,
            2 * (shells as i64),
            "Euler-Poincaré V-E+F = 2(S-H), S={shells}, H=0"
        );
    }

    /// T04: Manifold condition — each edge referenced by exactly 2 HEs with opposite orientation,
    ///       plus loop closure (consecutive HE endpoint ≈ next HE start point).
    #[test]
    fn test_cylinder_manifold_and_loop_closure() {
        let mut gen = IdGenerator::new(0);
        let s = make_cylinder(5.0, 20.0, &mut gen).unwrap();
        let eps = 1e-10;

        // Each edge must have exactly 2 HEs with opposite forward flags
        let mut edge_he_count: std::collections::HashMap<usize, Vec<bool>> =
            std::collections::HashMap::new();
        for he in &s.half_edges {
            edge_he_count.entry(he.edge).or_default().push(he.forward);
        }
        for (edge_idx, forwards) in &edge_he_count {
            assert_eq!(forwards.len(), 2, "edge {edge_idx} must have exactly 2 HEs");
            assert_ne!(
                forwards[0], forwards[1],
                "edge {edge_idx} HEs must have opposite orientation"
            );
        }


exec
/bin/bash -lc "rg -n \"fn sample_segment|sample_segment\\(\" crates/mycad-kernel/src/geometry -g'*.rs' && sed -n '1,260p' crates/mycad-kernel/src/geometry/curve.rs" in /home/bacon/mycad
 succeeded in 0ms:
crates/mycad-kernel/src/geometry/curve.rs:42:    pub fn sample_segment(&self, t_start: f64, t_end: f64, segments: usize) -> Vec<Point> {
use super::math::orthonormal_basis;
use super::{Point, Vec3};
use serde::{Deserialize, Serialize};

/// A geometric curve in 3D space.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Curve {
    /// A line defined by an origin point and a direction vector.
    Line { origin: Point, direction: Vec3 },
    /// A circle defined by a center, normal, and radius.
    Circle {
        center: Point,
        normal: Vec3,
        radius: f64,
    },
}

impl Curve {
    /// Evaluate the curve at parameter t.
    /// For a line: origin + t * direction
    /// For a circle: center + radius * (cos(t) * u + sin(t) * v)
    pub fn evaluate(&self, t: f64) -> Point {
        match self {
            Curve::Line { origin, direction } => origin + t * direction,
            Curve::Circle {
                center,
                normal,
                radius,
            } => {
                // Build a local coordinate system from the normal
                let (u, v) = orthonormal_basis(normal);
                center + *radius * (t.cos() * u + t.sin() * v)
            }
        }
    }

    /// Sample points along the curve from `t_start` (inclusive) toward `t_end` (exclusive).
    ///
    /// Returns a non-empty point sequence starting at `evaluate(t_start)`.
    /// - `Line`: single point at `t_start` (linear interpolation between vertices handles the rest).
    /// - `Circle`: `segments` equally-spaced points from `t_start` toward `t_end`.
    pub fn sample_segment(&self, t_start: f64, t_end: f64, segments: usize) -> Vec<Point> {
        match self {
            Curve::Line { .. } => vec![self.evaluate(t_start)],
            Curve::Circle { .. } => {
                if segments == 0 {
                    return vec![self.evaluate(t_start)];
                }
                let step = (t_end - t_start) / segments as f64;
                (0..segments)
                    .map(|i| self.evaluate(t_start + step * i as f64))
                    .collect()
            }
        }
    }
}

exec
/bin/bash -lc "nl -ba crates/mycad-kernel/src/tessellation/mod.rs | sed -n '150,245p'" in /home/bacon/mycad
 succeeded in 0ms:
   150	    Ok(())
   151	}
   152	
   153	/// Collect the ordered boundary points of a loop by sampling each HE's curve.
   154	fn collect_loop_points(
   155	    solid: &Solid,
   156	    lp: &crate::brep::topology::Loop,
   157	    segments: usize,
   158	) -> Result<Vec<Point>, TessellationError> {
   159	    let mut points = Vec::new();
   160	
   161	    for &he_idx in &lp.half_edges {
   162	        let he = &solid.half_edges[he_idx];
   163	        let edge = &solid.edges[he.edge];
   164	
   165	        let (t_start, t_end) = if he.forward {
   166	            (edge.t_range[0], edge.t_range[1])
   167	        } else {
   168	            (edge.t_range[1], edge.t_range[0])
   169	        };
   170	
   171	        let seg_points = edge.curve.sample_segment(t_start, t_end, segments);
   172	        for p in seg_points {
   173	            points.push(p);
   174	        }
   175	    }
   176	
   177	    Ok(points)
   178	}
   179	
   180	/// Triangulate a face using UV grid sampling (full untrimmed periodic/rectangular patch).
   181	fn tessellate_face_uv_grid(
   182	    solid: &Solid,
   183	    face: &crate::brep::topology::Face,
   184	    opts: &TessellationOptions,
   185	    mesh: &mut TriangleMesh,
   186	) -> Result<(), TessellationError> {
   187	    if !face.inner_loops.is_empty() {
   188	        return Err(TessellationError::TrimmedFaceUnsupported);
   189	    }
   190	
   191	    let outer_loop = &solid.loops[face.outer_loop];
   192	
   193	    // Guard: reject non-canonical cylinder faces (trimmed in u, seam not at u=0).
   194	    // At least one Circle edge must span ≈ 2π to qualify as a full-revolution patch.
   195	    let has_full_revolution = outer_loop.half_edges.iter().any(|&he_idx| {
   196	        let he = &solid.half_edges[he_idx];
   197	        let edge = &solid.edges[he.edge];
   198	        matches!(edge.curve, Curve::Circle { .. })
   199	            && (edge.t_range[1] - edge.t_range[0]).abs() >= 2.0 * PI - 0.1
   200	    });
   201	    if !has_full_revolution {
   202	        return Err(TessellationError::TrimmedFaceUnsupported);
   203	    }
   204	
   205	    // Determine v range from loop corner vertices (start_vertex of each HE).
   206	    // UvGridFullPatch assumes a full 2π revolution in u starting at u=0;
   207	    // we do not sample the circle arcs here — the grid covers [0,2π] directly.
   208	    let corner_uvs: Vec<(f64, f64)> = outer_loop
   209	        .half_edges
   210	        .iter()
   211	        .map(|&he_idx| {
   212	            let he = &solid.half_edges[he_idx];
   213	            face.surface.uv_of(&solid.vertices[he.start_vertex].point)
   214	        })
   215	        .collect();
   216	
   217	    let v_min = corner_uvs.iter().map(|(_, v)| *v).fold(f64::MAX, f64::min);
   218	    let v_max = corner_uvs.iter().map(|(_, v)| *v).fold(f64::MIN, f64::max);
   219	    let u_min = 0.0_f64;
   220	
   221	    // Clamp at point of use — callers may bypass TessellationOptions::new() via public fields
   222	    // or deserialization, so we defensively enforce minimum viable values here.
   223	    let n_u = opts.angular_segments.max(3);
   224	    let n_v = opts.axial_segments.max(1);
   225	
   226	    let base_idx = mesh.positions.len() as u32;
   227	
   228	    // Sample UV grid: (n_u+1) × (n_v+1) vertices
   229	    // u goes from u_min to u_min + 2π, v goes from v_min to v_max
   230	    let du = 2.0 * PI / n_u as f64;
   231	    let dv = (v_max - v_min) / n_v as f64;
   232	
   233	    for iv in 0..=n_v {
   234	        for iu in 0..=n_u {
   235	            let u = u_min + du * iu as f64;
   236	            let v = v_min + dv * iv as f64;
   237	            let p = face.surface.evaluate(u, v);
   238	            let normal = face.surface.normal_at(u, v);
   239	            let n = if face.same_sense {
   240	                [normal.x, normal.y, normal.z]
   241	            } else {
   242	                [-normal.x, -normal.y, -normal.z]
   243	            };
   244	            mesh.positions.push([p.x, p.y, p.z]);
   245	            mesh.normals.push(n);

exec
/bin/bash -lc "sed -n '1,240p' crates/mycad-kernel/src/brep/topology.rs" in /home/bacon/mycad
 succeeded in 0ms:
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

exec
/bin/bash -lc "rg -n \"triangle_count\\(|make_cylinder|tessellate_solid_with\\(&solid, &TessellationOptions|zero_segments|partial_revolution\" crates/mycad-kernel/src/tessellation/mod.rs crates/mycad-kernel/src/primitives -g'*.rs'" in /home/bacon/mycad
 succeeded in 0ms:
crates/mycad-kernel/src/tessellation/mod.rs:37:    pub fn triangle_count(&self) -> usize {
crates/mycad-kernel/src/tessellation/mod.rs:306:    use crate::primitives::make_cylinder;
crates/mycad-kernel/src/tessellation/mod.rs:310:    fn test_tessellate_cylinder_triangle_count() {
crates/mycad-kernel/src/tessellation/mod.rs:312:        let solid = make_cylinder(5.0, 20.0, &mut gen).unwrap();
crates/mycad-kernel/src/tessellation/mod.rs:320:            mesh.triangle_count(),
crates/mycad-kernel/src/tessellation/mod.rs:323:            mesh.triangle_count()
crates/mycad-kernel/src/tessellation/mod.rs:333:        let s1 = make_cylinder(5.0, 20.0, &mut gen1).unwrap();
crates/mycad-kernel/src/tessellation/mod.rs:334:        let s2 = make_cylinder(5.0, 20.0, &mut gen2).unwrap();
crates/mycad-kernel/src/tessellation/mod.rs:351:        assert_eq!(mesh.triangle_count(), 12);
crates/mycad-kernel/src/tessellation/mod.rs:378:        let solid = make_cylinder(5.0, 20.0, &mut gen).unwrap();
crates/mycad-kernel/src/tessellation/mod.rs:384:        for tri in 0..mesh.triangle_count() {
crates/mycad-kernel/src/tessellation/mod.rs:432:        let solid = make_cylinder(5.0, 20.0, &mut gen).unwrap();
crates/mycad-kernel/src/tessellation/mod.rs:435:        let mesh_low = tessellate_solid_with(&solid, &TessellationOptions::new(8, 1)).unwrap();
crates/mycad-kernel/src/tessellation/mod.rs:436:        let mesh_hi = tessellate_solid_with(&solid, &TessellationOptions::new(64, 2)).unwrap();
crates/mycad-kernel/src/tessellation/mod.rs:438:        assert_ne!(mesh_default.triangle_count(), mesh_low.triangle_count());
crates/mycad-kernel/src/tessellation/mod.rs:439:        assert_ne!(mesh_default.triangle_count(), mesh_hi.triangle_count());
crates/mycad-kernel/src/tessellation/mod.rs:442:        assert_eq!(mesh_low.triangle_count(), 8 * 2 + (8 - 2) * 2);
crates/mycad-kernel/src/tessellation/mod.rs:444:        assert_eq!(mesh_hi.triangle_count(), 64 * 2 * 2 + (64 - 2) * 2);
crates/mycad-kernel/src/tessellation/mod.rs:493:        assert_eq!(mesh.triangle_count(), 12);
crates/mycad-kernel/src/tessellation/mod.rs:517:    fn test_zero_segments_clamped_at_use() {
crates/mycad-kernel/src/tessellation/mod.rs:519:        let solid = make_cylinder(5.0, 20.0, &mut gen).unwrap();
crates/mycad-kernel/src/tessellation/mod.rs:526:        assert_eq!(mesh.triangle_count(), 8);
crates/mycad-kernel/src/tessellation/mod.rs:537:    fn test_partial_revolution_cylinder_face_errors() {
crates/mycad-kernel/src/primitives/mod.rs:5:pub use cylinder::make_cylinder;
crates/mycad-kernel/src/primitives/cylinder.rs:11:pub fn make_cylinder(
crates/mycad-kernel/src/primitives/cylinder.rs:138:        let s1 = make_cylinder(5.0, 20.0, &mut gen1).unwrap();
crates/mycad-kernel/src/primitives/cylinder.rs:139:        let s2 = make_cylinder(5.0, 20.0, &mut gen2).unwrap();
crates/mycad-kernel/src/primitives/cylinder.rs:165:        let s = make_cylinder(5.0, 20.0, &mut gen).unwrap();
crates/mycad-kernel/src/primitives/cylinder.rs:184:        let s = make_cylinder(5.0, 20.0, &mut gen).unwrap();
crates/mycad-kernel/src/primitives/cylinder.rs:204:        let s = make_cylinder(5.0, 20.0, &mut gen).unwrap();
crates/mycad-kernel/src/primitives/cylinder.rs:258:        let s = make_cylinder(5.0, 20.0, &mut gen).unwrap();
crates/mycad-kernel/src/primitives/cylinder.rs:305:        let err = make_cylinder(0.0, 20.0, &mut gen).unwrap_err();
crates/mycad-kernel/src/primitives/cylinder.rs:312:        let err = make_cylinder(-5.0, 20.0, &mut gen).unwrap_err();
crates/mycad-kernel/src/primitives/cylinder.rs:319:        let err = make_cylinder(f64::NAN, 20.0, &mut gen).unwrap_err();
crates/mycad-kernel/src/primitives/cylinder.rs:326:        let err = make_cylinder(f64::INFINITY, 20.0, &mut gen).unwrap_err();
crates/mycad-kernel/src/primitives/cylinder.rs:333:        let err = make_cylinder(5.0, 0.0, &mut gen).unwrap_err();
crates/mycad-kernel/src/primitives/cylinder.rs:340:        let err = make_cylinder(5.0, -10.0, &mut gen).unwrap_err();
crates/mycad-kernel/src/primitives/cylinder.rs:347:        let err = make_cylinder(5.0, f64::NAN, &mut gen).unwrap_err();
crates/mycad-kernel/src/primitives/cylinder.rs:354:        let err = make_cylinder(5.0, f64::INFINITY, &mut gen).unwrap_err();

exec
/bin/bash -lc "rg -n \"Curve::Circle|t_range: \\[|add_edge\\(\" crates/mycad-kernel/src -g'*.rs' | head -n 200" in /home/bacon/mycad
 succeeded in 0ms:
crates/mycad-kernel/src/tessellation/mod.rs:198:        matches!(edge.curve, Curve::Circle { .. })
crates/mycad-kernel/src/tessellation/mod.rs:456:        let e0 = s.add_edge(
crates/mycad-kernel/src/tessellation/mod.rs:541:        let e0 = s.add_edge(
crates/mycad-kernel/src/tessellation/mod.rs:544:            Curve::Circle {
crates/mycad-kernel/src/primitives/cylinder.rs:30:    let e_bot = solid.add_edge(
crates/mycad-kernel/src/primitives/cylinder.rs:33:        Curve::Circle {
crates/mycad-kernel/src/primitives/cylinder.rs:41:    let e_top = solid.add_edge(
crates/mycad-kernel/src/primitives/cylinder.rs:44:        Curve::Circle {
crates/mycad-kernel/src/primitives/cylinder.rs:52:    let e_seam = solid.add_edge(
crates/mycad-kernel/src/geometry/curve.rs:25:            Curve::Circle {
crates/mycad-kernel/src/geometry/curve.rs:45:            Curve::Circle { .. } => {
crates/mycad-kernel/src/primitives/cuboid.rs:72:            solid.add_edge(
crates/mycad-kernel/src/brep/topology.rs:41:    pub t_range: [f64; 2],
crates/mycad-kernel/src/brep/topology.rs:112:    pub fn add_edge(
crates/mycad-kernel/src/brep/topology.rs:117:        t_range: [f64; 2],

codex
The new zero-segment clamping is fine, but the cylinder-face guard is both too permissive for near-full trimmed faces and too restrictive for valid full cylinders whose circular edges have been split. Those cases either still generate incorrect meshes or newly reject geometry that previously tessellated correctly.

Full review comments:

- [P1] Tighten the full-revolution span check — /home/bacon/mycad/crates/mycad-kernel/src/tessellation/mod.rs:199-199
  If a cylindrical face is trimmed by less than `0.1` rad (~5.7°), this condition still treats it as a full revolution and the UV-grid path emits a closed cylinder instead of rejecting it. Because `t_range` is stored explicitly on each edge, the floating-point noise here should be tiny; using a `0.1`-rad tolerance reintroduces the silent mis-meshing this change is trying to prevent for near-full partial cylinders.

- [P2] Accept full cylinders whose circular bounds are edge-split — /home/bacon/mycad/crates/mycad-kernel/src/tessellation/mod.rs:195-200
  This `any(|... span ≈ 2π|)` test rejects valid full-revolution cylinder faces when both circular boundaries have been split into multiple consecutive `Circle` edges (for example, two `π` arcs per cap after edge splitting or import). In that case no individual edge spans `2π`, so we now return `TrimmedFaceUnsupported` even though the existing grid tessellator still has enough information to mesh the full cylinder correctly from the loop vertices' `v` range.
The new zero-segment clamping is fine, but the cylinder-face guard is both too permissive for near-full trimmed faces and too restrictive for valid full cylinders whose circular edges have been split. Those cases either still generate incorrect meshes or newly reject geometry that previously tessellated correctly.

Full review comments:

- [P1] Tighten the full-revolution span check — /home/bacon/mycad/crates/mycad-kernel/src/tessellation/mod.rs:199-199
  If a cylindrical face is trimmed by less than `0.1` rad (~5.7°), this condition still treats it as a full revolution and the UV-grid path emits a closed cylinder instead of rejecting it. Because `t_range` is stored explicitly on each edge, the floating-point noise here should be tiny; using a `0.1`-rad tolerance reintroduces the silent mis-meshing this change is trying to prevent for near-full partial cylinders.

- [P2] Accept full cylinders whose circular bounds are edge-split — /home/bacon/mycad/crates/mycad-kernel/src/tessellation/mod.rs:195-200
  This `any(|... span ≈ 2π|)` test rejects valid full-revolution cylinder faces when both circular boundaries have been split into multiple consecutive `Circle` edges (for example, two `π` arcs per cap after edge splitting or import). In that case no individual edge spans `2π`, so we now return `TrimmedFaceUnsupported` even though the existing grid tessellator still has enough information to mesh the full cylinder correctly from the loop vertices' `v` range.
