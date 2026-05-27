OpenAI Codex v0.121.0 (research preview)
--------
workdir: /home/bacon/mycad
model: gpt-5.4
provider: openai
approval: never
sandbox: read-only
reasoning effort: xhigh
reasoning summaries: none
session id: 019e5d38-7ce6-7081-874f-d1522e758fba
--------
user
commit 360f324
exec
/bin/bash -lc "git rev-parse --show-toplevel && git show --stat --oneline --decorate=short 360f324 && echo '---DIFF---' && git show --unified=80 360f324 --" in /home/bacon/mycad
 succeeded in 0ms:
/home/bacon/mycad
360f324 (HEAD -> claude/add-claude-guidelines-BKKtD, origin/claude/add-claude-guidelines-BKKtD, origin/HEAD) feat(kernel): cylinder primitive + curved surface tessellation foundation
 crates/mycad-build/src/lib.rs                  |   6 +-
 crates/mycad-cli/src/main.rs                   |  26 +-
 crates/mycad-kernel/benches/tessellation.rs    |   2 +-
 crates/mycad-kernel/src/error.rs               |   3 +
 crates/mycad-kernel/src/geometry/curve.rs      |  20 ++
 crates/mycad-kernel/src/geometry/surface.rs    |  20 ++
 crates/mycad-kernel/src/primitives/cylinder.rs | 374 ++++++++++++++++++++
 crates/mycad-kernel/src/primitives/mod.rs      |   2 +
 crates/mycad-kernel/src/tessellation/mod.rs    | 450 ++++++++++++++++++++++---
 crates/mycad-kernel/src/tessellation/stl.rs    |   4 +-
 crates/mycad-kernel/tests/cuboid_roundtrip.rs  |   4 +-
 examples/cylinder.mycad                        |   8 +
 12 files changed, 864 insertions(+), 55 deletions(-)
---DIFF---
commit 360f324d35e1753537b2c6325cb86fa5328eea78
Author: GS-Bacon <bacon0817.ix@gmail.com>
Date:   Sun May 24 07:39:53 2026 +0000

    feat(kernel): cylinder primitive + curved surface tessellation foundation
    
    Adds make_cylinder (B-rep: 2 vertices / 3 edges / 3 faces / closed shell,
    axis +Z) and extends tessellate_solid_with to handle circular-arc loops:
    BoundaryFan for plane caps, UvGridFullPatch for the cylinder side surface.
    
    Key additions:
    - make_cylinder(radius, height) -> Result<Solid, KernelError> with is_finite+positive validation
    - Curve::sample_segment(t_start, t_end, segments) for direction-aware arc polylines
    - Surface::tessellation_strategy() -> TessellationStrategy enum (Plane/Cylinder/Unsupported)
    - TessellationError propagates to CLI — no silent face-drop on unsupported surfaces
    - --segments CLI flag on mycad export
    - examples/cylinder.mycad + T01-T15 test suite (32 tests green)
    - cargo xtask ci green
    
    Closes #12
    
    Co-Authored-By: Claude Sonnet 4.6 <noreply@anthropic.com>

diff --git a/crates/mycad-build/src/lib.rs b/crates/mycad-build/src/lib.rs
index 0a57906..6e48c33 100644
--- a/crates/mycad-build/src/lib.rs
+++ b/crates/mycad-build/src/lib.rs
@@ -1,38 +1,36 @@
 use mycad_format::Feature;
 use mycad_kernel::brep::topology::{IdGenerator, Solid};
 use mycad_kernel::error::KernelError;
-use mycad_kernel::primitives::make_cuboid;
+use mycad_kernel::primitives::{make_cuboid, make_cylinder};
 
 pub fn build_solid_from_features(
     features: &[Feature],
     gen: &mut IdGenerator,
 ) -> Result<Solid, KernelError> {
     if features.is_empty() {
         return Err(KernelError::EmptyFeatureList);
     }
 
     if features.len() > 1 {
         return Err(KernelError::MultipleFeatures {
             count: features.len(),
         });
     }
 
     match &features[0] {
         Feature::CreateBox {
             width,
             height,
             depth,
             ..
         } => Ok(make_cuboid(*width, *height, *depth, gen)),
-        Feature::CreateCylinder { .. } => Err(KernelError::UnsupportedFeature {
-            kind: "create_cylinder",
-        }),
+        Feature::CreateCylinder { radius, height, .. } => make_cylinder(*radius, *height, gen),
         Feature::CreateSphere { .. } => Err(KernelError::UnsupportedFeature {
             kind: "create_sphere",
         }),
         Feature::Extrude { .. } => Err(KernelError::UnsupportedFeature { kind: "extrude" }),
         Feature::Cut { .. } => Err(KernelError::UnsupportedFeature { kind: "cut" }),
         Feature::Fuse { .. } => Err(KernelError::UnsupportedFeature { kind: "fuse" }),
         Feature::Intersect { .. } => Err(KernelError::UnsupportedFeature { kind: "intersect" }),
     }
 }
diff --git a/crates/mycad-cli/src/main.rs b/crates/mycad-cli/src/main.rs
index 09e4b0a..2c46221 100644
--- a/crates/mycad-cli/src/main.rs
+++ b/crates/mycad-cli/src/main.rs
@@ -1,51 +1,67 @@
 use clap::{Parser, Subcommand};
 use mycad_build::build_solid_from_features;
 use mycad_format::Document;
 use mycad_kernel::brep::topology::IdGenerator;
-use mycad_kernel::tessellation::{tessellate_solid, to_ascii_stl};
+use mycad_kernel::tessellation::{tessellate_solid_with, to_ascii_stl, TessellationOptions};
 use std::path::PathBuf;
 use std::process;
 
 #[derive(Parser)]
 #[command(name = "mycad", version, about = "MyCad CAD kernel CLI")]
 struct Cli {
     #[command(subcommand)]
     command: Commands,
 }
 
 #[derive(Subcommand)]
 enum Commands {
     /// Convert a .mycad file to STL
     Export {
         /// Input .mycad file
         input: PathBuf,
         /// Output STL file
         #[arg(short, long)]
         output: PathBuf,
+        /// Number of angular segments for curved surfaces (default: 32)
+        #[arg(long)]
+        segments: Option<usize>,
     },
 }
 
 fn main() {
     let cli = Cli::parse();
     match cli.command {
-        Commands::Export { input, output } => {
-            if let Err(msg) = run_export(&input, &output) {
+        Commands::Export {
+            input,
+            output,
+            segments,
+        } => {
+            if let Err(msg) = run_export(&input, &output, segments) {
                 eprintln!("error: {msg}");
                 process::exit(1);
             }
         }
     }
 }
 
-fn run_export(input: &std::path::Path, output: &std::path::Path) -> Result<(), String> {
+fn run_export(
+    input: &std::path::Path,
+    output: &std::path::Path,
+    segments: Option<usize>,
+) -> Result<(), String> {
     let doc = Document::from_path(input)
         .map_err(|e| format!("failed to read {}: {e}", input.display()))?;
     let mut gen = IdGenerator::new(0);
     let solid = build_solid_from_features(&doc.root_component.features, &mut gen)
         .map_err(|e| format!("failed to build solid: {e}"))?;
-    let mesh = tessellate_solid(&solid);
+    let opts = match segments {
+        Some(n) => TessellationOptions::new(n, 1),
+        None => TessellationOptions::default(),
+    };
+    let mesh =
+        tessellate_solid_with(&solid, &opts).map_err(|e| format!("failed to tessellate: {e}"))?;
     let stl = to_ascii_stl(&mesh, "model");
     std::fs::write(output, stl)
         .map_err(|e| format!("failed to write {}: {e}", output.display()))?;
     Ok(())
 }
diff --git a/crates/mycad-kernel/benches/tessellation.rs b/crates/mycad-kernel/benches/tessellation.rs
index 1158696..c97b61c 100644
--- a/crates/mycad-kernel/benches/tessellation.rs
+++ b/crates/mycad-kernel/benches/tessellation.rs
@@ -1,26 +1,26 @@
 use mycad_kernel::brep::topology::IdGenerator;
 use mycad_kernel::primitives::make_cuboid;
 use mycad_kernel::tessellation::tessellate_solid;
 
 fn main() {
     // Simple manual benchmark until criterion is added.
     // Replace with criterion when ready:
     //   use criterion::{criterion_group, criterion_main, Criterion};
 
     let iterations = 10_000;
     let start = std::time::Instant::now();
 
     for i in 0..iterations {
         let mut id_gen = IdGenerator::new(i as u64);
         let solid = make_cuboid(10.0, 20.0, 30.0, &mut id_gen);
-        let _mesh = tessellate_solid(&solid);
+        let _mesh = tessellate_solid(&solid).unwrap();
     }
 
     let elapsed = start.elapsed();
     println!(
         "Tessellated {} cuboids in {:?} ({:.2} us/iter)",
         iterations,
         elapsed,
         elapsed.as_micros() as f64 / iterations as f64
     );
 }
diff --git a/crates/mycad-kernel/src/error.rs b/crates/mycad-kernel/src/error.rs
index eeef025..bf248d6 100644
--- a/crates/mycad-kernel/src/error.rs
+++ b/crates/mycad-kernel/src/error.rs
@@ -1,13 +1,16 @@
 use thiserror::Error;
 
 #[derive(Debug, Error)]
 pub enum KernelError {
     #[error("empty feature list")]
     EmptyFeatureList,
 
     #[error("multiple features not yet supported (got {count})")]
     MultipleFeatures { count: usize },
 
     #[error("unsupported feature variant: {kind}")]
     UnsupportedFeature { kind: &'static str },
+
+    #[error("invalid parameter: {kind}")]
+    InvalidParameter { kind: &'static str },
 }
diff --git a/crates/mycad-kernel/src/geometry/curve.rs b/crates/mycad-kernel/src/geometry/curve.rs
index 17ef952..9512066 100644
--- a/crates/mycad-kernel/src/geometry/curve.rs
+++ b/crates/mycad-kernel/src/geometry/curve.rs
@@ -1,36 +1,56 @@
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
+
+    /// Sample points along the curve from `t_start` (inclusive) toward `t_end` (exclusive).
+    ///
+    /// Returns a non-empty point sequence starting at `evaluate(t_start)`.
+    /// - `Line`: single point at `t_start` (linear interpolation between vertices handles the rest).
+    /// - `Circle`: `segments` equally-spaced points from `t_start` toward `t_end`.
+    pub fn sample_segment(&self, t_start: f64, t_end: f64, segments: usize) -> Vec<Point> {
+        match self {
+            Curve::Line { .. } => vec![self.evaluate(t_start)],
+            Curve::Circle { .. } => {
+                if segments == 0 {
+                    return vec![self.evaluate(t_start)];
+                }
+                let step = (t_end - t_start) / segments as f64;
+                (0..segments)
+                    .map(|i| self.evaluate(t_start + step * i as f64))
+                    .collect()
+            }
+        }
+    }
 }
diff --git a/crates/mycad-kernel/src/geometry/surface.rs b/crates/mycad-kernel/src/geometry/surface.rs
index 727c630..bec65f4 100644
--- a/crates/mycad-kernel/src/geometry/surface.rs
+++ b/crates/mycad-kernel/src/geometry/surface.rs
@@ -1,240 +1,260 @@
 use super::math::orthonormal_basis;
 use super::{Point, Vec3};
 use serde::{Deserialize, Serialize};
 
+/// Strategy for tessellating a face on a given surface type.
+#[derive(Debug, Clone, Copy, PartialEq, Eq)]
+pub enum TessellationStrategy {
+    /// Fan from first boundary vertex (convex planar face).
+    BoundaryFan,
+    /// Full UV grid patch (untrimmed periodic/rectangular face).
+    UvGridFullPatch,
+    /// Not yet supported by the tessellator.
+    Unsupported,
+}
+
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
+
+    /// Return the tessellation strategy for this surface type.
+    pub fn tessellation_strategy(&self) -> TessellationStrategy {
+        match self {
+            Surface::Plane { .. } => TessellationStrategy::BoundaryFan,
+            Surface::Cylinder { .. } => TessellationStrategy::UvGridFullPatch,
+            Surface::Sphere { .. } | Surface::Cone { .. } => TessellationStrategy::Unsupported,
+        }
+    }
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
             center: Point::origin(),
             radius: 1.0,
         };
         let p1 = surface.evaluate(0.0, 0.0);
         let p2 = surface.evaluate(std::f64::consts::FRAC_PI_2, 0.3);
         let n1 = surface.normal_at_point(&p1);
         let n2 = surface.normal_at_point(&p2);
         // Distinct surface points yield non-parallel outward normals.
         assert!((n1.dot(&n2)).abs() < 0.99);
     }
 
     #[test]
     fn test_plane_normal_constant_per_point() {
         let surface = Surface::Plane {
             origin: Point::origin(),
             normal: Vec3::z(),
             u_axis: Vec3::x(),
             v_axis: Vec3::y(),
         };
         let n1 = surface.normal_at_point(&Point::new(5.0, -2.0, 0.0));
diff --git a/crates/mycad-kernel/src/primitives/cylinder.rs b/crates/mycad-kernel/src/primitives/cylinder.rs
new file mode 100644
index 0000000..af32de8
--- /dev/null
+++ b/crates/mycad-kernel/src/primitives/cylinder.rs
@@ -0,0 +1,374 @@
+use crate::brep::topology::{IdGenerator, Solid};
+use crate::error::KernelError;
+use crate::geometry::curve::Curve;
+use crate::geometry::surface::Surface;
+use crate::geometry::{Point, Vec3};
+
+/// Create a cylinder B-rep solid.
+///
+/// Axis = +Z, bottom center at origin, bottom z=0 / top z=height.
+/// Seam vertex at (0, radius, z) — angle u=0 in the orthonormal_basis(+Z) = (+Y, -X) frame.
+pub fn make_cylinder(
+    radius: f64,
+    height: f64,
+    id_gen: &mut IdGenerator,
+) -> Result<Solid, KernelError> {
+    if !radius.is_finite() || radius <= 0.0 {
+        return Err(KernelError::InvalidParameter { kind: "radius" });
+    }
+    if !height.is_finite() || height <= 0.0 {
+        return Err(KernelError::InvalidParameter { kind: "height" });
+    }
+
+    let mut solid = Solid::new(id_gen.next());
+
+    // 2 vertices: seam at bottom and top
+    let v_bot = solid.add_vertex(id_gen.next(), Point::new(0.0, radius, 0.0));
+    let v_top = solid.add_vertex(id_gen.next(), Point::new(0.0, radius, height));
+
+    // 3 edges
+    let e_bot = solid.add_edge(
+        id_gen.next(),
+        [v_bot, v_bot],
+        Curve::Circle {
+            center: Point::origin(),
+            normal: Vec3::z(),
+            radius,
+        },
+        [0.0, 2.0 * std::f64::consts::PI],
+    );
+
+    let e_top = solid.add_edge(
+        id_gen.next(),
+        [v_top, v_top],
+        Curve::Circle {
+            center: Point::new(0.0, 0.0, height),
+            normal: Vec3::z(),
+            radius,
+        },
+        [0.0, 2.0 * std::f64::consts::PI],
+    );
+
+    let e_seam = solid.add_edge(
+        id_gen.next(),
+        [v_bot, v_top],
+        Curve::Line {
+            origin: Point::new(0.0, radius, 0.0),
+            direction: Vec3::new(0.0, 0.0, height),
+        },
+        [0.0, 1.0],
+    );
+
+    // 3 faces: bottom cap, top cap, lateral
+    let mut face_indices = Vec::with_capacity(3);
+
+    // Bottom cap: Surface::Plane with normal -Z, loop = e_bot reversed (CW from above = outward -Z)
+    let he_bot_rev = solid.add_half_edge(id_gen.next(), v_bot, e_bot, false);
+    let loop_bot = solid.add_loop(id_gen.next(), vec![he_bot_rev]);
+    let f_bot = solid.add_face(
+        id_gen.next(),
+        Surface::Plane {
+            origin: Point::origin(),
+            normal: -Vec3::z(),
+            u_axis: Vec3::x(),
+            v_axis: Vec3::y(),
+        },
+        loop_bot,
+        vec![],
+        true,
+    );
+    face_indices.push(f_bot);
+
+    // Top cap: Surface::Plane with normal +Z, loop = e_top forward (CCW from above = outward +Z)
+    let he_top_fwd = solid.add_half_edge(id_gen.next(), v_top, e_top, true);
+    let loop_top = solid.add_loop(id_gen.next(), vec![he_top_fwd]);
+    let f_top = solid.add_face(
+        id_gen.next(),
+        Surface::Plane {
+            origin: Point::new(0.0, 0.0, height),
+            normal: Vec3::z(),
+            u_axis: Vec3::x(),
+            v_axis: Vec3::y(),
+        },
+        loop_top,
+        vec![],
+        true,
+    );
+    face_indices.push(f_top);
+
+    // Lateral face: Surface::Cylinder, loop = 4 HE
+    //   seam up (fwd, start v_bot) → top circle (rev, start v_top) →
+    //   seam down (rev, start v_top) → bot circle (fwd, start v_bot)
+    let he_seam_up = solid.add_half_edge(id_gen.next(), v_bot, e_seam, true);
+    let he_top_rev = solid.add_half_edge(id_gen.next(), v_top, e_top, false);
+    let he_seam_dn = solid.add_half_edge(id_gen.next(), v_top, e_seam, false);
+    let he_bot_fwd = solid.add_half_edge(id_gen.next(), v_bot, e_bot, true);
+    let loop_lat = solid.add_loop(
+        id_gen.next(),
+        vec![he_seam_up, he_top_rev, he_seam_dn, he_bot_fwd],
+    );
+    let f_lat = solid.add_face(
+        id_gen.next(),
+        Surface::Cylinder {
+            origin: Point::origin(),
+            axis: Vec3::z(),
+            radius,
+        },
+        loop_lat,
+        vec![],
+        true,
+    );
+    face_indices.push(f_lat);
+
+    solid.add_shell(id_gen.next(), face_indices, true);
+
+    Ok(solid)
+}
+
+#[cfg(test)]
+mod tests {
+    use super::*;
+    use crate::geometry::surface::TessellationStrategy;
+
+    /// T01: Determinism — identical inputs produce identical solids (field-by-field).
+    #[test]
+    fn test_cylinder_deterministic() {
+        let mut gen1 = IdGenerator::new(0);
+        let mut gen2 = IdGenerator::new(0);
+        let s1 = make_cylinder(5.0, 20.0, &mut gen1).unwrap();
+        let s2 = make_cylinder(5.0, 20.0, &mut gen2).unwrap();
+
+        assert_eq!(s1.vertices.len(), s2.vertices.len());
+        assert_eq!(s1.edges.len(), s2.edges.len());
+        assert_eq!(s1.faces.len(), s2.faces.len());
+        assert_eq!(s1.half_edges.len(), s2.half_edges.len());
+        assert_eq!(s1.loops.len(), s2.loops.len());
+        assert_eq!(s1.shells.len(), s2.shells.len());
+
+        for (v1, v2) in s1.vertices.iter().zip(s2.vertices.iter()) {
+            assert_eq!(v1.id, v2.id);
+            assert_eq!(v1.point, v2.point);
+        }
+        for (e1, e2) in s1.edges.iter().zip(s2.edges.iter()) {
+            assert_eq!(e1.id, e2.id);
+            assert_eq!(e1.vertices, e2.vertices);
+        }
+        for (f1, f2) in s1.faces.iter().zip(s2.faces.iter()) {
+            assert_eq!(f1.id, f2.id);
+        }
+    }
+
+    /// T02: Topology counts.
+    #[test]
+    fn test_cylinder_topology() {
+        let mut gen = IdGenerator::new(0);
+        let s = make_cylinder(5.0, 20.0, &mut gen).unwrap();
+
+        assert_eq!(s.vertices.len(), 2, "2 seam vertices");
+        assert_eq!(
+            s.edges.len(),
+            3,
+            "3 edges (bot circle, top circle, seam line)"
+        );
+        assert_eq!(s.faces.len(), 3, "3 faces (bottom cap, top cap, lateral)");
+        assert_eq!(s.shells.len(), 1);
+        assert!(s.shells[0].closed);
+        assert_eq!(s.half_edges.len(), 6, "6 half-edges (3 edges × 2 HE each)");
+        assert_eq!(s.loops.len(), 3, "3 loops (one per face)");
+    }
+
+    /// T03: Euler–Poincaré formula V - E + F = 2(S - H), S=1, H=0 → 2.
+    #[test]
+    fn test_cylinder_euler() {
+        let mut gen = IdGenerator::new(0);
+        let s = make_cylinder(5.0, 20.0, &mut gen).unwrap();
+
+        let v = s.vertices.len();
+        let e = s.edges.len();
+        let f = s.faces.len();
+        let shells = s.shells.len();
+        // genus=0 for simple cylinder
+        let euler = (v as i64) - (e as i64) + (f as i64);
+        assert_eq!(
+            euler,
+            2 * (shells as i64),
+            "Euler-Poincaré V-E+F = 2(S-H), S={shells}, H=0"
+        );
+    }
+
+    /// T04: Manifold condition — each edge referenced by exactly 2 HEs with opposite orientation,
+    ///       plus loop closure (consecutive HE endpoint ≈ next HE start point).
+    #[test]
+    fn test_cylinder_manifold_and_loop_closure() {
+        let mut gen = IdGenerator::new(0);
+        let s = make_cylinder(5.0, 20.0, &mut gen).unwrap();
+        let eps = 1e-10;
+
+        // Each edge must have exactly 2 HEs with opposite forward flags
+        let mut edge_he_count: std::collections::HashMap<usize, Vec<bool>> =
+            std::collections::HashMap::new();
+        for he in &s.half_edges {
+            edge_he_count.entry(he.edge).or_default().push(he.forward);
+        }
+        for (edge_idx, forwards) in &edge_he_count {
+            assert_eq!(forwards.len(), 2, "edge {edge_idx} must have exactly 2 HEs");
+            assert_ne!(
+                forwards[0], forwards[1],
+                "edge {edge_idx} HEs must have opposite orientation"
+            );
+        }
+
+        // Loop closure: each loop's consecutive HEs must connect geometrically
+        for (loop_idx, lp) in s.loops.iter().enumerate() {
+            assert!(
+                !lp.half_edges.is_empty(),
+                "loop {loop_idx} must not be empty"
+            );
+            for i in 0..lp.half_edges.len() {
+                let he_cur = &s.half_edges[lp.half_edges[i]];
+                let he_next = &s.half_edges[lp.half_edges[(i + 1) % lp.half_edges.len()]];
+
+                let cur_edge = &s.edges[he_cur.edge];
+                let end_v = if he_cur.forward {
+                    cur_edge.vertices[1]
+                } else {
+                    cur_edge.vertices[0]
+                };
+
+                let next_start = he_next.start_vertex;
+
+                assert!(
+                    (s.vertices[end_v].point - s.vertices[next_start].point).norm() < eps,
+                    "loop {loop_idx}: HE {} end vertex {} ({:?}) != HE {} start vertex {} ({:?})",
+                    i,
+                    end_v,
+                    s.vertices[end_v].point,
+                    (i + 1) % lp.half_edges.len(),
+                    next_start,
+                    s.vertices[next_start].point,
+                );
+            }
+        }
+    }
+
+    /// T05: Geometry — seam vertices and surface coefficients.
+    #[test]
+    fn test_cylinder_geometry() {
+        let mut gen = IdGenerator::new(0);
+        let s = make_cylinder(5.0, 20.0, &mut gen).unwrap();
+        let eps = 1e-10;
+
+        // Seam vertices
+        assert!((s.vertices[0].point - Point::new(0.0, 5.0, 0.0)).norm() < eps);
+        assert!((s.vertices[1].point - Point::new(0.0, 5.0, 20.0)).norm() < eps);
+
+        // Bottom cap surface: Plane with normal -Z
+        match &s.faces[0].surface {
+            Surface::Plane { normal, .. } => {
+                assert!(
+                    (*normal + Vec3::z()).norm() < eps,
+                    "bottom cap normal should be -Z"
+                );
+            }
+            _ => panic!("bottom cap should be a Plane"),
+        }
+
+        // Top cap surface: Plane with normal +Z
+        match &s.faces[1].surface {
+            Surface::Plane { normal, .. } => {
+                assert!(
+                    (*normal - Vec3::z()).norm() < eps,
+                    "top cap normal should be +Z"
+                );
+            }
+            _ => panic!("top cap should be a Plane"),
+        }
+
+        // Lateral surface: Cylinder
+        match &s.faces[2].surface {
+            Surface::Cylinder {
+                radius: r, axis, ..
+            } => {
+                assert!((r - 5.0).abs() < eps);
+                assert!((*axis - Vec3::z()).norm() < eps);
+            }
+            _ => panic!("lateral face should be Cylinder surface"),
+        }
+    }
+
+    /// T14: Degenerate inputs — radius<=0, height<=0, non-finite → InvalidParameter.
+    #[test]
+    fn test_cylinder_degenerate_inputs() {
+        let mut gen = IdGenerator::new(0);
+
+        // Zero radius
+        let err = make_cylinder(0.0, 20.0, &mut gen).unwrap_err();
+        assert!(matches!(
+            err,
+            KernelError::InvalidParameter { kind: "radius" }
+        ));
+
+        // Negative radius
+        let err = make_cylinder(-5.0, 20.0, &mut gen).unwrap_err();
+        assert!(matches!(
+            err,
+            KernelError::InvalidParameter { kind: "radius" }
+        ));
+
+        // NaN radius
+        let err = make_cylinder(f64::NAN, 20.0, &mut gen).unwrap_err();
+        assert!(matches!(
+            err,
+            KernelError::InvalidParameter { kind: "radius" }
+        ));
+
+        // Inf radius
+        let err = make_cylinder(f64::INFINITY, 20.0, &mut gen).unwrap_err();
+        assert!(matches!(
+            err,
+            KernelError::InvalidParameter { kind: "radius" }
+        ));
+
+        // Zero height
+        let err = make_cylinder(5.0, 0.0, &mut gen).unwrap_err();
+        assert!(matches!(
+            err,
+            KernelError::InvalidParameter { kind: "height" }
+        ));
+
+        // Negative height
+        let err = make_cylinder(5.0, -10.0, &mut gen).unwrap_err();
+        assert!(matches!(
+            err,
+            KernelError::InvalidParameter { kind: "height" }
+        ));
+
+        // NaN height
+        let err = make_cylinder(5.0, f64::NAN, &mut gen).unwrap_err();
+        assert!(matches!(
+            err,
+            KernelError::InvalidParameter { kind: "height" }
+        ));
+
+        // Inf height
+        let err = make_cylinder(5.0, f64::INFINITY, &mut gen).unwrap_err();
+        assert!(matches!(
+            err,
+            KernelError::InvalidParameter { kind: "height" }
+        ));
+    }
+
+    /// Verify that cylinder surface reports UvGridFullPatch strategy.
+    #[test]
+    fn test_cylinder_surface_tessellation_strategy() {
+        let s = Surface::Cylinder {
+            origin: Point::origin(),
+            axis: Vec3::z(),
+            radius: 5.0,
+        };
+        assert_eq!(
+            s.tessellation_strategy(),
+            TessellationStrategy::UvGridFullPatch
+        );
+    }
+}
diff --git a/crates/mycad-kernel/src/primitives/mod.rs b/crates/mycad-kernel/src/primitives/mod.rs
index f2ba81c..a2e33b5 100644
--- a/crates/mycad-kernel/src/primitives/mod.rs
+++ b/crates/mycad-kernel/src/primitives/mod.rs
@@ -1,3 +1,5 @@
 mod cuboid;
+mod cylinder;
 
 pub use cuboid::make_cuboid;
+pub use cylinder::make_cylinder;
diff --git a/crates/mycad-kernel/src/tessellation/mod.rs b/crates/mycad-kernel/src/tessellation/mod.rs
index 3537e8f..166f24e 100644
--- a/crates/mycad-kernel/src/tessellation/mod.rs
+++ b/crates/mycad-kernel/src/tessellation/mod.rs
@@ -1,130 +1,498 @@
 pub mod stl;
 pub use stl::to_ascii_stl;
 
 use crate::brep::topology::Solid;
+use crate::geometry::surface::{Surface, TessellationStrategy};
 use crate::geometry::Point;
 use serde::{Deserialize, Serialize};
+use std::f64::consts::PI;
+use thiserror::Error;
+
+/// Seam jump detection threshold: |Δu| > π indicates a parameter-space seam crossing.
+/// Minimum cross-product norm for a non-degenerate triangle.
+const AREA_EPS: f64 = 1e-14;
 
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
 
-/// Tessellate a B-rep solid into a triangle mesh.
-///
-/// Each face's outer loop is triangulated as a fan from its first vertex.
-/// Normals are evaluated per vertex from the underlying surface, so curved
-/// faces get correct shading rather than a single per-face normal.
-///
-/// Not yet handled: interior subdivision of curved faces (uv sampling) — a
-/// curved face is still approximated by its boundary fan — and inner loops
-/// (holes). Both are deferred to the curved-primitive work (Phase 3).
-pub fn tessellate_solid(solid: &Solid) -> TriangleMesh {
+/// Controls the resolution of curved-surface tessellation.
+#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
+pub struct TessellationOptions {
+    pub angular_segments: usize,
+    pub axial_segments: usize,
+}
+
+impl Default for TessellationOptions {
+    fn default() -> Self {
+        Self {
+            angular_segments: 32,
+            axial_segments: 1,
+        }
+    }
+}
+
+impl TessellationOptions {
+    pub fn new(angular_segments: usize, axial_segments: usize) -> Self {
+        Self {
+            angular_segments: angular_segments.max(3),
+            axial_segments: axial_segments.max(1),
+        }
+    }
+}
+
+/// Errors from tessellation.
+#[derive(Debug, Error)]
+pub enum TessellationError {
+    #[error("unsupported surface type: {kind}")]
+    UnsupportedSurface { kind: &'static str },
+    #[error("trimmed or punctured faces are not yet supported")]
+    TrimmedFaceUnsupported,
+    #[error("non-manifold loop detected")]
+    NonManifoldLoop,
+}
+
+/// Tessellate a B-rep solid into a triangle mesh using default options.
+pub fn tessellate_solid(solid: &Solid) -> Result<TriangleMesh, TessellationError> {
+    tessellate_solid_with(solid, &TessellationOptions::default())
+}
+
+/// Tessellate a B-rep solid into a triangle mesh with the given options.
+pub fn tessellate_solid_with(
+    solid: &Solid,
+    opts: &TessellationOptions,
+) -> Result<TriangleMesh, TessellationError> {
     let mut mesh = TriangleMesh::new();
 
     for face in &solid.faces {
-        let outer_loop = &solid.loops[face.outer_loop];
-
-        // Collect the vertices of this face's outer loop
-        let loop_vertices: Vec<&Point> = outer_loop
-            .half_edges
-            .iter()
-            .map(|&he_idx| {
-                let he = &solid.half_edges[he_idx];
-                &solid.vertices[he.start_vertex].point
-            })
-            .collect();
-
-        if loop_vertices.len() < 3 {
-            continue;
+        let strategy = face.surface.tessellation_strategy();
+        match strategy {
+            TessellationStrategy::BoundaryFan => {
+                tessellate_face_fan(solid, face, opts, &mut mesh)?;
+            }
+            TessellationStrategy::UvGridFullPatch => {
+                tessellate_face_uv_grid(solid, face, opts, &mut mesh)?;
+            }
+            TessellationStrategy::Unsupported => {
+                return Err(TessellationError::UnsupportedSurface {
+                    kind: face.surface.kind_name(),
+                });
+            }
         }
+    }
+
+    Ok(mesh)
+}
 
-        // Add vertices and triangulate as a fan from vertex 0.
-        // Normals are evaluated per vertex from the surface so curved faces
-        // are shaded correctly (planar faces yield a constant normal).
-        let base_idx = mesh.positions.len() as u32;
+/// Triangulate a face as a fan from its first boundary vertex (planar convex faces).
+fn tessellate_face_fan(
+    solid: &Solid,
+    face: &crate::brep::topology::Face,
+    opts: &TessellationOptions,
+    mesh: &mut TriangleMesh,
+) -> Result<(), TessellationError> {
+    let outer_loop = &solid.loops[face.outer_loop];
+
+    let loop_points = collect_loop_points(solid, outer_loop, opts.angular_segments)?;
+
+    if loop_points.len() < 3 {
+        return Ok(());
+    }
+
+    let base_idx = mesh.positions.len() as u32;
+
+    for p in &loop_points {
+        let normal = face.surface.normal_at_point(p);
+        let n = if face.same_sense {
+            [normal.x, normal.y, normal.z]
+        } else {
+            [-normal.x, -normal.y, -normal.z]
+        };
+        mesh.positions.push([p.x, p.y, p.z]);
+        mesh.normals.push(n);
+    }
 
-        for &v in &loop_vertices {
-            let normal = face.surface.normal_at_point(v);
+    for i in 1..(loop_points.len() as u32 - 1) {
+        mesh.indices.push(base_idx);
+        mesh.indices.push(base_idx + i);
+        mesh.indices.push(base_idx + i + 1);
+    }
+
+    Ok(())
+}
+
+/// Collect the ordered boundary points of a loop by sampling each HE's curve.
+fn collect_loop_points(
+    solid: &Solid,
+    lp: &crate::brep::topology::Loop,
+    segments: usize,
+) -> Result<Vec<Point>, TessellationError> {
+    let mut points = Vec::new();
+
+    for &he_idx in &lp.half_edges {
+        let he = &solid.half_edges[he_idx];
+        let edge = &solid.edges[he.edge];
+
+        let (t_start, t_end) = if he.forward {
+            (edge.t_range[0], edge.t_range[1])
+        } else {
+            (edge.t_range[1], edge.t_range[0])
+        };
+
+        let seg_points = edge.curve.sample_segment(t_start, t_end, segments);
+        for p in seg_points {
+            points.push(p);
+        }
+    }
+
+    Ok(points)
+}
+
+/// Triangulate a face using UV grid sampling (full untrimmed periodic/rectangular patch).
+fn tessellate_face_uv_grid(
+    solid: &Solid,
+    face: &crate::brep::topology::Face,
+    opts: &TessellationOptions,
+    mesh: &mut TriangleMesh,
+) -> Result<(), TessellationError> {
+    if !face.inner_loops.is_empty() {
+        return Err(TessellationError::TrimmedFaceUnsupported);
+    }
+
+    // Determine v range from loop corner vertices (start_vertex of each HE).
+    // UvGridFullPatch assumes a full 2π revolution in u starting at u=0;
+    // we do not sample the circle arcs here — the grid covers [0,2π] directly.
+    let outer_loop = &solid.loops[face.outer_loop];
+    let corner_uvs: Vec<(f64, f64)> = outer_loop
+        .half_edges
+        .iter()
+        .map(|&he_idx| {
+            let he = &solid.half_edges[he_idx];
+            face.surface.uv_of(&solid.vertices[he.start_vertex].point)
+        })
+        .collect();
+
+    let v_min = corner_uvs.iter().map(|(_, v)| *v).fold(f64::MAX, f64::min);
+    let v_max = corner_uvs.iter().map(|(_, v)| *v).fold(f64::MIN, f64::max);
+    let u_min = 0.0_f64;
+
+    let n_u = opts.angular_segments;
+    let n_v = opts.axial_segments;
+
+    let base_idx = mesh.positions.len() as u32;
+
+    // Sample UV grid: (n_u+1) × (n_v+1) vertices
+    // u goes from u_min to u_min + 2π, v goes from v_min to v_max
+    let du = 2.0 * PI / n_u as f64;
+    let dv = (v_max - v_min) / n_v as f64;
+
+    for iv in 0..=n_v {
+        for iu in 0..=n_u {
+            let u = u_min + du * iu as f64;
+            let v = v_min + dv * iv as f64;
+            let p = face.surface.evaluate(u, v);
+            let normal = face.surface.normal_at(u, v);
             let n = if face.same_sense {
                 [normal.x, normal.y, normal.z]
             } else {
                 [-normal.x, -normal.y, -normal.z]
             };
-            mesh.positions.push([v.x, v.y, v.z]);
+            mesh.positions.push([p.x, p.y, p.z]);
             mesh.normals.push(n);
         }
+    }
 
-        for i in 1..(loop_vertices.len() as u32 - 1) {
-            mesh.indices.push(base_idx);
-            mesh.indices.push(base_idx + i);
-            mesh.indices.push(base_idx + i + 1);
+    // Generate triangles for each cell
+    for iv in 0..n_v {
+        for iu in 0..n_u {
+            let i00 = base_idx + (iv * (n_u + 1) + iu) as u32;
+            let i10 = base_idx + (iv * (n_u + 1) + iu + 1) as u32;
+            let i01 = base_idx + ((iv + 1) * (n_u + 1) + iu) as u32;
+            let i11 = base_idx + ((iv + 1) * (n_u + 1) + iu + 1) as u32;
+
+            // Two triangles per cell, orientation so ∂u×∂v points outward
+            push_triangle(mesh, i00, i10, i01);
+            push_triangle(mesh, i10, i11, i01);
         }
     }
 
-    mesh
+    Ok(())
+}
+
+/// Push a triangle, but skip degenerate (zero-area) ones.
+fn push_triangle(mesh: &mut TriangleMesh, i0: u32, i1: u32, i2: u32) {
+    let p0: [f64; 3] = mesh.positions[i0 as usize];
+    let p1: [f64; 3] = mesh.positions[i1 as usize];
+    let p2: [f64; 3] = mesh.positions[i2 as usize];
+    let u = [p1[0] - p0[0], p1[1] - p0[1], p1[2] - p0[2]];
+    let v = [p2[0] - p0[0], p2[1] - p0[1], p2[2] - p0[2]];
+    let cross_norm = (u[1] * v[2] - u[2] * v[1]).powi(2)
+        + (u[2] * v[0] - u[0] * v[2]).powi(2)
+        + (u[0] * v[1] - u[1] * v[0]).powi(2).sqrt();
+    if cross_norm < AREA_EPS * AREA_EPS {
+        return;
+    }
+    mesh.indices.push(i0);
+    mesh.indices.push(i1);
+    mesh.indices.push(i2);
+}
+
+/// Surface kind name for error messages.
+trait SurfaceKind {
+    fn kind_name(&self) -> &'static str;
+}
+
+impl SurfaceKind for Surface {
+    fn kind_name(&self) -> &'static str {
+        match self {
+            Surface::Plane { .. } => "plane",
+            Surface::Cylinder { .. } => "cylinder",
+            Surface::Sphere { .. } => "sphere",
+            Surface::Cone { .. } => "cone",
+        }
+    }
 }
 
 #[cfg(test)]
 mod tests {
     use super::*;
     use crate::brep::topology::IdGenerator;
+    use crate::geometry::Vec3;
     use crate::primitives::make_cuboid;
+    use crate::primitives::make_cylinder;
+
+    /// T06: Normal tessellation — triangle count for cylinder with default 32 angular segments.
+    #[test]
+    fn test_tessellate_cylinder_triangle_count() {
+        let mut gen = IdGenerator::new(0);
+        let solid = make_cylinder(5.0, 20.0, &mut gen).unwrap();
+        let mesh = tessellate_solid(&solid).unwrap();
+
+        // Lateral: 32 angular segments × 1 axial × 2 triangles = 64
+        // Each cap: 32-gon fan = 30 triangles (32 - 2)
+        // Total: 64 + 30 + 30 = 124
+        let expected = 32 * 2 + (32 - 2) * 2;
+        assert_eq!(
+            mesh.triangle_count(),
+            expected,
+            "expected {expected} triangles, got {}",
+            mesh.triangle_count()
+        );
+    }
+
+    /// T07: Determinism — same input → same output.
+    #[test]
+    fn test_tessellation_cylinder_deterministic() {
+        let mut gen1 = IdGenerator::new(0);
+        let mut gen2 = IdGenerator::new(0);
+
+        let s1 = make_cylinder(5.0, 20.0, &mut gen1).unwrap();
+        let s2 = make_cylinder(5.0, 20.0, &mut gen2).unwrap();
+
+        let m1 = tessellate_solid(&s1).unwrap();
+        let m2 = tessellate_solid(&s2).unwrap();
+
+        assert_eq!(m1.positions, m2.positions);
+        assert_eq!(m1.normals, m2.normals);
+        assert_eq!(m1.indices, m2.indices);
+    }
+
+    /// T08: Regression — cuboid tessellation is byte-identical to pre-cylinder output.
+    #[test]
+    fn test_tessellate_cuboid_unchanged() {
+        let mut gen = IdGenerator::new(0);
+        let solid = make_cuboid(1.0, 1.0, 1.0, &mut gen);
+        let mesh = tessellate_solid(&solid).unwrap();
+
+        assert_eq!(mesh.triangle_count(), 12);
+        assert_eq!(mesh.positions.len(), 24);
+        assert_eq!(mesh.normals.len(), 24);
+        assert_eq!(mesh.indices.len(), 36);
+    }
+
+    /// T08 extended: exact positions and indices snapshot.
+    #[test]
+    fn test_cuboid_positions_indices_snapshot() {
+        let mut gen1 = IdGenerator::new(0);
+        let mut gen2 = IdGenerator::new(0);
+
+        let s1 = make_cuboid(1.0, 1.0, 1.0, &mut gen1);
+        let s2 = make_cuboid(1.0, 1.0, 1.0, &mut gen2);
+
+        let m1 = tessellate_solid(&s1).unwrap();
+        let m2 = tessellate_solid(&s2).unwrap();
+
+        // Byte-identical across two runs
+        assert_eq!(m1.positions, m2.positions);
+        assert_eq!(m1.indices, m2.indices);
+    }
+
+    /// T09: Outward normals — all facet normals point away from centroid.
+    #[test]
+    fn test_cylinder_outward_normals() {
+        let mut gen = IdGenerator::new(0);
+        let solid = make_cylinder(5.0, 20.0, &mut gen).unwrap();
+        let mesh = tessellate_solid(&solid).unwrap();
+
+        // Centroid of cylinder: (0, 0, 10)
+        let centroid = [0.0f64, 0.0, 10.0];
+
+        for tri in 0..mesh.triangle_count() {
+            let i0 = mesh.indices[tri * 3] as usize;
+            let i1 = mesh.indices[tri * 3 + 1] as usize;
+            let i2 = mesh.indices[tri * 3 + 2] as usize;
+
+            let p0 = mesh.positions[i0];
+            let p1 = mesh.positions[i1];
+            let p2 = mesh.positions[i2];
+
+            // Facet center
+            let center = [
+                (p0[0] + p1[0] + p2[0]) / 3.0,
+                (p0[1] + p1[1] + p2[1]) / 3.0,
+                (p0[2] + p1[2] + p2[2]) / 3.0,
+            ];
+
+            // Cross product for facet normal
+            let u = [p1[0] - p0[0], p1[1] - p0[1], p1[2] - p0[2]];
+            let v = [p2[0] - p0[0], p2[1] - p0[1], p2[2] - p0[2]];
+            let nx = u[1] * v[2] - u[2] * v[1];
+            let ny = u[2] * v[0] - u[0] * v[2];
+            let nz = u[0] * v[1] - u[1] * v[0];
+
+            // Vector from centroid to facet center
+            let dx = center[0] - centroid[0];
+            let dy = center[1] - centroid[1];
+            let dz = center[2] - centroid[2];
+
+            // Dot product should be positive (normal points outward)
+            let dot = nx * dx + ny * dy + nz * dz;
+            assert!(
+                dot > 0.0,
+                "facet {tri}: normal should point outward (dot={dot})"
+            );
+        }
+    }
+
+    /// T12: Resolution externalization + clamping.
+    #[test]
+    fn test_tessellation_options_clamp() {
+        let opts = TessellationOptions::new(2, 0);
+        assert_eq!(opts.angular_segments, 3);
+        assert_eq!(opts.axial_segments, 1);
+    }
+
+    #[test]
+    fn test_different_resolution_different_count() {
+        let mut gen = IdGenerator::new(0);
+        let solid = make_cylinder(5.0, 20.0, &mut gen).unwrap();
+
+        let mesh_default = tessellate_solid(&solid).unwrap();
+        let mesh_low = tessellate_solid_with(&solid, &TessellationOptions::new(8, 1)).unwrap();
+        let mesh_hi = tessellate_solid_with(&solid, &TessellationOptions::new(64, 2)).unwrap();
+
+        assert_ne!(mesh_default.triangle_count(), mesh_low.triangle_count());
+        assert_ne!(mesh_default.triangle_count(), mesh_hi.triangle_count());
+
+        // 8 angular, 1 axial: lateral=8*1*2=16, caps=(8-2)*2=12 → 28
+        assert_eq!(mesh_low.triangle_count(), 8 * 2 + (8 - 2) * 2);
+        // 64 angular, 2 axial: lateral=64*2*2=256, caps=(64-2)*2=124 → 380
+        assert_eq!(mesh_hi.triangle_count(), 64 * 2 * 2 + (64 - 2) * 2);
+    }
+
+    /// T15: Unsupported surface type → error.
+    #[test]
+    fn test_unsupported_surface_error() {
+        use crate::brep::topology::Solid as S;
+        use crate::geometry::curve::Curve;
+
+        let mut s = S::new(0);
+        let v0 = s.add_vertex(1, Point::new(0.0, 0.0, 0.0));
+        let v1 = s.add_vertex(2, Point::new(1.0, 0.0, 0.0));
+        let e0 = s.add_edge(
+            3,
+            [v0, v1],
+            Curve::Line {
+                origin: Point::origin(),
+                direction: Vec3::x(),
+            },
+            [0.0, 1.0],
+        );
+        let he0 = s.add_half_edge(4, v0, e0, true);
+        let lp = s.add_loop(5, vec![he0]);
+        s.add_face(
+            6,
+            Surface::Sphere {
+                center: Point::origin(),
+                radius: 1.0,
+            },
+            lp,
+            vec![],
+            true,
+        );
+        s.add_shell(7, vec![0], true);
+
+        let result = tessellate_solid(&s);
+        assert!(matches!(
+            result,
+            Err(TessellationError::UnsupportedSurface { kind: "sphere" })
+        ));
+    }
 
+    // Legacy tests preserved for backward compatibility
     #[test]
     fn test_tessellate_cuboid() {
         let mut id_gen = IdGenerator::new(0);
         let solid = make_cuboid(1.0, 1.0, 1.0, &mut id_gen);
-        let mesh = tessellate_solid(&solid);
+        let mesh = tessellate_solid(&solid).unwrap();
 
-        // A cuboid has 6 faces, each tessellated as 2 triangles = 12 triangles
         assert_eq!(mesh.triangle_count(), 12);
-        // 6 faces * 4 vertices = 24 vertices (not shared across faces for correct normals)
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
 
-        let mesh1 = tessellate_solid(&solid1);
-        let mesh2 = tessellate_solid(&solid2);
+        let mesh1 = tessellate_solid(&solid1).unwrap();
+        let mesh2 = tessellate_solid(&solid2).unwrap();
 
         assert_eq!(mesh1.positions, mesh2.positions);
         assert_eq!(mesh1.normals, mesh2.normals);
         assert_eq!(mesh1.indices, mesh2.indices);
     }
 }
diff --git a/crates/mycad-kernel/src/tessellation/stl.rs b/crates/mycad-kernel/src/tessellation/stl.rs
index 6edf172..5f201cf 100644
--- a/crates/mycad-kernel/src/tessellation/stl.rs
+++ b/crates/mycad-kernel/src/tessellation/stl.rs
@@ -1,80 +1,80 @@
 use super::TriangleMesh;
 use std::fmt::Write;
 
 pub fn to_ascii_stl(mesh: &TriangleMesh, solid_name: &str) -> String {
     let tri_count = mesh.triangle_count();
     let mut out = String::with_capacity(64 + tri_count * 200);
 
     writeln!(out, "solid {}", solid_name).unwrap();
 
     for tri in 0..tri_count {
         let i0 = mesh.indices[tri * 3] as usize;
         let i1 = mesh.indices[tri * 3 + 1] as usize;
         let i2 = mesh.indices[tri * 3 + 2] as usize;
 
         let p0 = mesh.positions[i0];
         let p1 = mesh.positions[i1];
         let p2 = mesh.positions[i2];
 
         let n = face_normal(p0, p1, p2);
 
         writeln!(out, "  facet normal {} {} {}", n[0], n[1], n[2]).unwrap();
         writeln!(out, "    outer loop").unwrap();
         writeln!(out, "      vertex {} {} {}", p0[0], p0[1], p0[2]).unwrap();
         writeln!(out, "      vertex {} {} {}", p1[0], p1[1], p1[2]).unwrap();
         writeln!(out, "      vertex {} {} {}", p2[0], p2[1], p2[2]).unwrap();
         writeln!(out, "    endloop").unwrap();
         writeln!(out, "  endfacet").unwrap();
     }
 
     writeln!(out, "endsolid {}", solid_name).unwrap();
     out
 }
 
 fn face_normal(p0: [f64; 3], p1: [f64; 3], p2: [f64; 3]) -> [f64; 3] {
     let u = [p1[0] - p0[0], p1[1] - p0[1], p1[2] - p0[2]];
     let v = [p2[0] - p0[0], p2[1] - p0[1], p2[2] - p0[2]];
     let nx = u[1] * v[2] - u[2] * v[1];
     let ny = u[2] * v[0] - u[0] * v[2];
     let nz = u[0] * v[1] - u[1] * v[0];
     let len = (nx * nx + ny * ny + nz * nz).sqrt();
     if len > 0.0 {
         [nx / len, ny / len, nz / len]
     } else {
         [0.0, 0.0, 0.0]
     }
 }
 
 #[cfg(test)]
 mod tests {
     use super::*;
     use crate::brep::topology::IdGenerator;
     use crate::primitives::make_cuboid;
     use crate::tessellation::tessellate_solid;
 
     #[test]
     fn ascii_stl_cuboid_structure() {
         let mut g = IdGenerator::new(0);
         let solid = make_cuboid(1.0, 1.0, 1.0, &mut g);
-        let mesh = tessellate_solid(&solid);
+        let mesh = tessellate_solid(&solid).unwrap();
         let stl = to_ascii_stl(&mesh, "test_cube");
 
         assert!(stl.starts_with("solid test_cube\n"));
         assert!(stl.trim_end().ends_with("endsolid test_cube"));
         assert_eq!(stl.matches("facet normal").count(), 12);
         assert_eq!(stl.matches("endfacet").count(), 12);
         assert_eq!(stl.matches("outer loop").count(), 12);
         assert_eq!(stl.matches("endloop").count(), 12);
         assert_eq!(stl.matches("\n      vertex ").count(), 36);
     }
 
     #[test]
     fn ascii_stl_deterministic() {
         let run = || {
             let mut g = IdGenerator::new(0);
             let s = make_cuboid(1.0, 1.0, 1.0, &mut g);
-            to_ascii_stl(&tessellate_solid(&s), "cube")
+            to_ascii_stl(&tessellate_solid(&s).unwrap(), "cube")
         };
         assert_eq!(run(), run());
     }
 }
diff --git a/crates/mycad-kernel/tests/cuboid_roundtrip.rs b/crates/mycad-kernel/tests/cuboid_roundtrip.rs
index 3b76f70..29e77e4 100644
--- a/crates/mycad-kernel/tests/cuboid_roundtrip.rs
+++ b/crates/mycad-kernel/tests/cuboid_roundtrip.rs
@@ -1,41 +1,41 @@
 use mycad_kernel::brep::topology::IdGenerator;
 use mycad_kernel::primitives::make_cuboid;
 use mycad_kernel::tessellation::tessellate_solid;
 
 /// Integration test: create a cuboid, tessellate it, and verify the pipeline.
 #[test]
 fn cuboid_to_mesh_pipeline() {
     let mut id_gen = IdGenerator::new(0);
     let solid = make_cuboid(10.0, 20.0, 30.0, &mut id_gen);
 
     // Verify topology
     assert_eq!(solid.vertices.len(), 8);
     assert_eq!(solid.edges.len(), 12);
     assert_eq!(solid.faces.len(), 6);
 
     // Tessellate
-    let mesh = tessellate_solid(&solid);
+    let mesh = tessellate_solid(&solid).unwrap();
 
     // Verify mesh
     assert_eq!(mesh.triangle_count(), 12); // 6 faces * 2 triangles
     assert_eq!(mesh.positions.len(), 24); // 6 faces * 4 vertices
     assert_eq!(mesh.normals.len(), 24);
     assert_eq!(mesh.indices.len(), 36); // 12 triangles * 3 indices
 }
 
 /// Integration test: deterministic pipeline produces identical results.
 #[test]
 fn cuboid_pipeline_is_deterministic() {
     let run = || {
         let mut id_gen = IdGenerator::new(42);
         let solid = make_cuboid(5.0, 10.0, 15.0, &mut id_gen);
-        tessellate_solid(&solid)
+        tessellate_solid(&solid).unwrap()
     };
 
     let mesh1 = run();
     let mesh2 = run();
 
     assert_eq!(mesh1.positions, mesh2.positions);
     assert_eq!(mesh1.normals, mesh2.normals);
     assert_eq!(mesh1.indices, mesh2.indices);
 }
diff --git a/examples/cylinder.mycad b/examples/cylinder.mycad
new file mode 100644
index 0000000..0bad825
--- /dev/null
+++ b/examples/cylinder.mycad
@@ -0,0 +1,8 @@
+version: "0.1.0"
+root_component:
+  name: "Simple Cylinder"
+  features:
+    - type: create_cylinder
+      id: cyl_1
+      radius: 5.0
+      height: 20.0

exec
/bin/bash -lc "sed -n '1,260p' crates/mycad-kernel/src/brep/topology.rs" in /home/bacon/mycad
exec
/bin/bash -lc "sed -n '1,260p' crates/mycad-kernel/src/primitives/cylinder.rs" in /home/bacon/mycad
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

        // Loop closure: each loop's consecutive HEs must connect geometrically
        for (loop_idx, lp) in s.loops.iter().enumerate() {
            assert!(
                !lp.half_edges.is_empty(),
                "loop {loop_idx} must not be empty"
            );
            for i in 0..lp.half_edges.len() {
                let he_cur = &s.half_edges[lp.half_edges[i]];
                let he_next = &s.half_edges[lp.half_edges[(i + 1) % lp.half_edges.len()]];

                let cur_edge = &s.edges[he_cur.edge];
                let end_v = if he_cur.forward {
                    cur_edge.vertices[1]
                } else {
                    cur_edge.vertices[0]
                };

                let next_start = he_next.start_vertex;

                assert!(
                    (s.vertices[end_v].point - s.vertices[next_start].point).norm() < eps,
                    "loop {loop_idx}: HE {} end vertex {} ({:?}) != HE {} start vertex {} ({:?})",
                    i,
                    end_v,
                    s.vertices[end_v].point,
                    (i + 1) % lp.half_edges.len(),
                    next_start,
                    s.vertices[next_start].point,
                );
            }
        }
    }

    /// T05: Geometry — seam vertices and surface coefficients.
    #[test]
    fn test_cylinder_geometry() {
        let mut gen = IdGenerator::new(0);
        let s = make_cylinder(5.0, 20.0, &mut gen).unwrap();
        let eps = 1e-10;


exec
/bin/bash -lc "sed -n '1,260p' crates/mycad-kernel/src/geometry/math.rs" in /home/bacon/mycad
 succeeded in 0ms:
use super::Vec3;

/// Build an orthonormal basis (u, v) from a normal vector.
///
/// Given a normal vector, computes two perpendicular unit vectors
/// that together with the normal form a right-handed coordinate system.
pub fn orthonormal_basis(normal: &Vec3) -> (Vec3, Vec3) {
    let n = normal.normalize();
    // Choose a vector not parallel to n
    let not_parallel = if n.x.abs() < 0.9 {
        Vec3::x()
    } else {
        Vec3::y()
    };
    let u = n.cross(&not_parallel).normalize();
    let v = n.cross(&u);
    (u, v)
}

#[cfg(test)]
mod tests {
    use super::*;
    use approx::assert_relative_eq;

    #[test]
    fn test_orthonormal_basis_z_axis() {
        let (u, v) = orthonormal_basis(&Vec3::z());
        assert_relative_eq!(u.norm(), 1.0, epsilon = 1e-12);
        assert_relative_eq!(v.norm(), 1.0, epsilon = 1e-12);
        assert_relative_eq!(u.dot(&v), 0.0, epsilon = 1e-12);
        assert_relative_eq!(u.dot(&Vec3::z()), 0.0, epsilon = 1e-12);
    }

    #[test]
    fn test_orthonormal_basis_x_axis() {
        let (u, v) = orthonormal_basis(&Vec3::x());
        assert_relative_eq!(u.norm(), 1.0, epsilon = 1e-12);
        assert_relative_eq!(v.norm(), 1.0, epsilon = 1e-12);
        assert_relative_eq!(u.dot(&v), 0.0, epsilon = 1e-12);
        assert_relative_eq!(u.dot(&Vec3::x()), 0.0, epsilon = 1e-12);
    }
}

exec
/bin/bash -lc "sed -n '1,260p' crates/mycad-kernel/src/primitives/cuboid.rs" in /home/bacon/mycad
 succeeded in 0ms:
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
                (0, true, 0), // e0: 0->1
                (1, true, 1), // e1: 1->3
                (2, true, 3), // e2: 3->2
                (3, true, 2), // e3: 2->0
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
                (4, false, 5), // e4: 5->4
                (7, false, 4), // e7: 4->6
                (6, false, 6), // e6: 6->7
                (5, false, 7), // e5: 7->5
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
                (8, true, 0),  // e8: 0->4
                (4, true, 4),  // e4: 4->5
                (9, false, 5), // e9: 5->1
                (0, false, 1), // e0: 1->0
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
                (2, false, 2),  // e2: 2->3
                (10, true, 3),  // e10: 3->7
                (6, true, 7),   // e6: 7->6
                (11, false, 6), // e11: 6->2
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
                (3, false, 0), // e3: 0->2
                (11, true, 2), // e11: 2->6
                (7, true, 6),  // e7: 6->4
                (8, false, 4), // e8: 4->0
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
                (9, true, 1),   // e9: 1->5
                (5, true, 5),   // e5: 5->7
                (10, false, 7), // e10: 7->3
                (1, false, 3),  // e1: 3->1
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

exec
/bin/bash -lc "sed -n '1,260p' crates/mycad-kernel/src/lib.rs && echo '---' && sed -n '1,200p' crates/mycad-kernel/src/primitives/mod.rs" in /home/bacon/mycad
 succeeded in 0ms:
pub mod brep;
pub mod error;
pub mod geometry;
pub mod primitives;
pub mod tessellation;

pub use brep::topology;
pub use error::KernelError;
---
mod cuboid;
mod cylinder;

pub use cuboid::make_cuboid;
pub use cylinder::make_cylinder;

exec
/bin/bash -lc 'grep -R "enum Feature" -n crates | head' in /home/bacon/mycad
 succeeded in 0ms:
crates/mycad-format/src/feature.rs:16:pub enum Feature {

exec
/bin/bash -lc "sed -n '1,220p' crates/mycad-format/src/lib.rs" in /home/bacon/mycad
 succeeded in 0ms:
pub mod component;
pub mod document;
pub mod error;
pub mod feature;

pub use component::Component;
pub use document::Document;
pub use error::FormatError;
pub use feature::Feature;

exec
/bin/bash -lc 'grep -R "CreateCylinder" -n crates | head -20' in /home/bacon/mycad
 succeeded in 0ms:
crates/mycad-build/src/lib.rs:27:        Feature::CreateCylinder { radius, height, .. } => make_cylinder(*radius, *height, gen),
crates/mycad-format/CLAUDE.md:34:| `CreateCylinder` | (未実装)    |
crates/mycad-format/src/feature.rs:28:    CreateCylinder {
crates/mycad-format/src/feature.rs:76:            | Feature::CreateCylinder { id, .. }

exec
/bin/bash -lc "sed -n '1,180p' crates/mycad-format/src/feature.rs" in /home/bacon/mycad
 succeeded in 0ms:
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// A reference to a topological entity (face, edge, vertex) on a feature's result.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct EntityRef {
    /// The feature that created this entity.
    pub feature_id: String,
    /// The role of this entity in the feature (e.g., "top_face", "side_edge_0").
    pub role: String,
}

/// A feature — one step in the operation history.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "type")]
pub enum Feature {
    /// Create a box primitive.
    #[serde(rename = "create_box")]
    CreateBox {
        id: String,
        width: f64,
        height: f64,
        depth: f64,
    },

    /// Create a cylinder primitive.
    #[serde(rename = "create_cylinder")]
    CreateCylinder {
        id: String,
        radius: f64,
        height: f64,
    },

    /// Create a sphere primitive.
    #[serde(rename = "create_sphere")]
    CreateSphere { id: String, radius: f64 },

    /// Extrude a sketch profile.
    #[serde(rename = "extrude")]
    Extrude {
        id: String,
        sketch: String,
        depth: f64,
    },

    /// Cut (boolean subtract) one body from another.
    #[serde(rename = "cut")]
    Cut {
        id: String,
        target: String,
        tool: String,
    },

    /// Fuse (boolean union) two bodies.
    #[serde(rename = "fuse")]
    Fuse {
        id: String,
        target: String,
        tool: String,
    },

    /// Intersect two bodies.
    #[serde(rename = "intersect")]
    Intersect {
        id: String,
        target: String,
        tool: String,
    },
}

impl Feature {
    /// Get the feature ID.
    pub fn id(&self) -> &str {
        match self {
            Feature::CreateBox { id, .. }
            | Feature::CreateCylinder { id, .. }
            | Feature::CreateSphere { id, .. }
            | Feature::Extrude { id, .. }
            | Feature::Cut { id, .. }
            | Feature::Fuse { id, .. }
            | Feature::Intersect { id, .. } => id,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_feature_serialization() {
        let feature = Feature::CreateBox {
            id: "box_1".to_string(),
            width: 10.0,
            height: 20.0,
            depth: 30.0,
        };

        let yaml = serde_yaml::to_string(&feature).unwrap();
        assert!(yaml.contains("create_box"));
        assert!(yaml.contains("box_1"));

        let deserialized: Feature = serde_yaml::from_str(&yaml).unwrap();
        assert_eq!(deserialized.id(), "box_1");
    }

    #[test]
    fn test_entity_ref_serialization() {
        let entity_ref = EntityRef {
            feature_id: "box_1".to_string(),
            role: "top_face".to_string(),
        };

        let yaml = serde_yaml::to_string(&entity_ref).unwrap();
        let deserialized: EntityRef = serde_yaml::from_str(&yaml).unwrap();
        assert_eq!(deserialized.feature_id, "box_1");
        assert_eq!(deserialized.role, "top_face");
    }
}

exec
/bin/bash -lc "sed -n '1,260p' crates/mycad-kernel/src/tessellation/mod.rs && echo '---PART2---' && sed -n '260,520p' crates/mycad-kernel/src/tessellation/mod.rs" in /home/bacon/mycad
 succeeded in 0ms:
pub mod stl;
pub use stl::to_ascii_stl;

use crate::brep::topology::Solid;
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

    let loop_points = collect_loop_points(solid, outer_loop, opts.angular_segments)?;

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

    // Determine v range from loop corner vertices (start_vertex of each HE).
    // UvGridFullPatch assumes a full 2π revolution in u starting at u=0;
    // we do not sample the circle arcs here — the grid covers [0,2π] directly.
    let outer_loop = &solid.loops[face.outer_loop];
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

    let n_u = opts.angular_segments;
    let n_v = opts.axial_segments;

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
---PART2---
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
        let solid = make_cuboid(1.0, 1.0, 1.0, &mut gen);
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

        let s1 = make_cuboid(1.0, 1.0, 1.0, &mut gen1);
        let s2 = make_cuboid(1.0, 1.0, 1.0, &mut gen2);

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
}

exec
/bin/bash -lc "sed -n '260,520p' crates/mycad-kernel/src/primitives/cylinder.rs" in /home/bacon/mycad
 succeeded in 0ms:

        // Seam vertices
        assert!((s.vertices[0].point - Point::new(0.0, 5.0, 0.0)).norm() < eps);
        assert!((s.vertices[1].point - Point::new(0.0, 5.0, 20.0)).norm() < eps);

        // Bottom cap surface: Plane with normal -Z
        match &s.faces[0].surface {
            Surface::Plane { normal, .. } => {
                assert!(
                    (*normal + Vec3::z()).norm() < eps,
                    "bottom cap normal should be -Z"
                );
            }
            _ => panic!("bottom cap should be a Plane"),
        }

        // Top cap surface: Plane with normal +Z
        match &s.faces[1].surface {
            Surface::Plane { normal, .. } => {
                assert!(
                    (*normal - Vec3::z()).norm() < eps,
                    "top cap normal should be +Z"
                );
            }
            _ => panic!("top cap should be a Plane"),
        }

        // Lateral surface: Cylinder
        match &s.faces[2].surface {
            Surface::Cylinder {
                radius: r, axis, ..
            } => {
                assert!((r - 5.0).abs() < eps);
                assert!((*axis - Vec3::z()).norm() < eps);
            }
            _ => panic!("lateral face should be Cylinder surface"),
        }
    }

    /// T14: Degenerate inputs — radius<=0, height<=0, non-finite → InvalidParameter.
    #[test]
    fn test_cylinder_degenerate_inputs() {
        let mut gen = IdGenerator::new(0);

        // Zero radius
        let err = make_cylinder(0.0, 20.0, &mut gen).unwrap_err();
        assert!(matches!(
            err,
            KernelError::InvalidParameter { kind: "radius" }
        ));

        // Negative radius
        let err = make_cylinder(-5.0, 20.0, &mut gen).unwrap_err();
        assert!(matches!(
            err,
            KernelError::InvalidParameter { kind: "radius" }
        ));

        // NaN radius
        let err = make_cylinder(f64::NAN, 20.0, &mut gen).unwrap_err();
        assert!(matches!(
            err,
            KernelError::InvalidParameter { kind: "radius" }
        ));

        // Inf radius
        let err = make_cylinder(f64::INFINITY, 20.0, &mut gen).unwrap_err();
        assert!(matches!(
            err,
            KernelError::InvalidParameter { kind: "radius" }
        ));

        // Zero height
        let err = make_cylinder(5.0, 0.0, &mut gen).unwrap_err();
        assert!(matches!(
            err,
            KernelError::InvalidParameter { kind: "height" }
        ));

        // Negative height
        let err = make_cylinder(5.0, -10.0, &mut gen).unwrap_err();
        assert!(matches!(
            err,
            KernelError::InvalidParameter { kind: "height" }
        ));

        // NaN height
        let err = make_cylinder(5.0, f64::NAN, &mut gen).unwrap_err();
        assert!(matches!(
            err,
            KernelError::InvalidParameter { kind: "height" }
        ));

        // Inf height
        let err = make_cylinder(5.0, f64::INFINITY, &mut gen).unwrap_err();
        assert!(matches!(
            err,
            KernelError::InvalidParameter { kind: "height" }
        ));
    }

    /// Verify that cylinder surface reports UvGridFullPatch strategy.
    #[test]
    fn test_cylinder_surface_tessellation_strategy() {
        let s = Surface::Cylinder {
            origin: Point::origin(),
            axis: Vec3::z(),
            radius: 5.0,
        };
        assert_eq!(
            s.tessellation_strategy(),
            TessellationStrategy::UvGridFullPatch
        );
    }
}

exec
/bin/bash -lc "grep -R \"tessellate_solid(\" -n . | sed -n '1,120p'" in /home/bacon/mycad
 succeeded in 0ms:
./features/12-cylinder/design-review-3.md:138:  pub fn tessellate_solid(solid) -> TriangleMesh           // = _with(solid, &default())  ※既存呼び出し・既存テスト不変
./features/12-cylinder/design-review-3.md:199:| T06 | テッセレーション正常系 | `tessellate_solid(cyl)` の三角形数 = angular*2(側面) + (angular-2)*2(キャップ) 等、既定 angular=32 で算出した期待値 | `triangle_count` 一致 |
./features/12-cylinder/design-review-3.md:228:crates/mycad-cli/src/main.rs:46:    let mesh = tessellate_solid(&solid);
./features/12-cylinder/design-review-3.md:252:crates/mycad-kernel/benches/tessellation.rs:16:        let _mesh = tessellate_solid(&solid);
./features/12-cylinder/design-review-3.md:257:crates/mycad-kernel/src/tessellation/mod.rs:49:pub fn tessellate_solid(solid: &Solid) -> TriangleMesh {
./features/12-cylinder/design-review-3.md:263:crates/mycad-kernel/src/tessellation/mod.rs:105:        let mesh = tessellate_solid(&solid);
./features/12-cylinder/design-review-3.md:268:crates/mycad-kernel/src/tessellation/mod.rs:123:        let mesh1 = tessellate_solid(&solid1);
./features/12-cylinder/design-review-3.md:269:crates/mycad-kernel/src/tessellation/mod.rs:124:        let mesh2 = tessellate_solid(&solid2);
./features/12-cylinder/design-review-3.md:284:crates/mycad-kernel/src/tessellation/stl.rs:59:        let mesh = tessellate_solid(&solid);
./features/12-cylinder/design-review-3.md:287:crates/mycad-kernel/src/tessellation/stl.rs:76:            to_ascii_stl(&tessellate_solid(&s), "cube")
./features/12-cylinder/design-review-3.md:295:crates/mycad-kernel/tests/cuboid_roundtrip.rs:17:    let mesh = tessellate_solid(&solid);
./features/12-cylinder/design-review-3.md:298:crates/mycad-kernel/tests/cuboid_roundtrip.rs:32:        tessellate_solid(&solid)
./features/12-cylinder/design-review-3.md:918:pub fn tessellate_solid(solid: &Solid) -> TriangleMesh {
./features/12-cylinder/design-review-3.md:974:        let mesh = tessellate_solid(&solid);
./features/12-cylinder/design-review-3.md:992:        let mesh1 = tessellate_solid(&solid1);
./features/12-cylinder/design-review-3.md:993:        let mesh2 = tessellate_solid(&solid2);
./features/12-cylinder/design-review-3.md:1392:    let mesh = tessellate_solid(&solid);
./features/12-cylinder/design-review-3.md:1478:    let mesh = tessellate_solid(&solid);
./features/12-cylinder/design-review-3.md:1493:        tessellate_solid(&solid)
./features/12-cylinder/design-review-3.md:2119:        let mesh = tessellate_solid(&solid);
./features/12-cylinder/design-review-3.md:2136:            to_ascii_stl(&tessellate_solid(&s), "cube")
./features/12-cylinder/final-review.md:166:-    let mesh = tessellate_solid(&solid);
./features/12-cylinder/final-review.md:198:-        let _mesh = tessellate_solid(&solid);
./features/12-cylinder/final-review.md:199:+        let _mesh = tessellate_solid(&solid).unwrap();
./features/12-cylinder/final-review.md:1007:-pub fn tessellate_solid(solid: &Solid) -> TriangleMesh {
./features/12-cylinder/final-review.md:1045:+pub fn tessellate_solid(solid: &Solid) -> Result<TriangleMesh, TessellationError> {
./features/12-cylinder/final-review.md:1284:+        let mesh = tessellate_solid(&solid).unwrap();
./features/12-cylinder/final-review.md:1307:+        let m1 = tessellate_solid(&s1).unwrap();
./features/12-cylinder/final-review.md:1308:+        let m2 = tessellate_solid(&s2).unwrap();
./features/12-cylinder/final-review.md:1320:+        let mesh = tessellate_solid(&solid).unwrap();
./features/12-cylinder/final-review.md:1337:+        let m1 = tessellate_solid(&s1).unwrap();
./features/12-cylinder/final-review.md:1338:+        let m2 = tessellate_solid(&s2).unwrap();
./features/12-cylinder/final-review.md:1350:+        let mesh = tessellate_solid(&solid).unwrap();
./features/12-cylinder/final-review.md:1405:+        let mesh_default = tessellate_solid(&solid).unwrap();
./features/12-cylinder/final-review.md:1450:+        let result = tessellate_solid(&s);
./features/12-cylinder/final-review.md:1462:-        let mesh = tessellate_solid(&solid);
./features/12-cylinder/final-review.md:1463:+        let mesh = tessellate_solid(&solid).unwrap();
./features/12-cylinder/final-review.md:1481:-        let mesh1 = tessellate_solid(&solid1);
./features/12-cylinder/final-review.md:1482:-        let mesh2 = tessellate_solid(&solid2);
./features/12-cylinder/final-review.md:1483:+        let mesh1 = tessellate_solid(&solid1).unwrap();
./features/12-cylinder/final-review.md:1484:+        let mesh2 = tessellate_solid(&solid2).unwrap();
./features/12-cylinder/final-review.md:1554:-        let mesh = tessellate_solid(&solid);
./features/12-cylinder/final-review.md:1555:+        let mesh = tessellate_solid(&solid).unwrap();
./features/12-cylinder/final-review.md:1572:-            to_ascii_stl(&tessellate_solid(&s), "cube")
./features/12-cylinder/final-review.md:1573:+            to_ascii_stl(&tessellate_solid(&s).unwrap(), "cube")
./features/12-cylinder/final-review.md:1599:-    let mesh = tessellate_solid(&solid);
./features/12-cylinder/final-review.md:1600:+    let mesh = tessellate_solid(&solid).unwrap();
./features/12-cylinder/final-review.md:1615:-        tessellate_solid(&solid)
./features/12-cylinder/final-review.md:1616:+        tessellate_solid(&solid).unwrap()
./features/12-cylinder/final-review.md:2695:pub fn tessellate_solid(solid: &Solid) -> Result<TriangleMesh, TessellationError> {
./features/12-cylinder/final-review.md:2910:        let mesh = tessellate_solid(&solid).unwrap();
./features/12-cylinder/final-review.md:2933:        let m1 = tessellate_solid(&s1).unwrap();
./features/12-cylinder/final-review.md:2934:        let m2 = tessellate_solid(&s2).unwrap();
./features/12-cylinder/final-review.md:2946:        let mesh = tessellate_solid(&solid).unwrap();
./features/12-cylinder/final-review.md:2963:        let m1 = tessellate_solid(&s1).unwrap();
./features/12-cylinder/final-review.md:2964:        let m2 = tessellate_solid(&s2).unwrap();
./features/12-cylinder/final-review.md:2976:        let mesh = tessellate_solid(&solid).unwrap();
./features/12-cylinder/final-review.md:3031:        let mesh_default = tessellate_solid(&solid).unwrap();
./features/12-cylinder/final-review.md:3076:        let result = tessellate_solid(&s);
./features/12-cylinder/final-review.md:3088:        let mesh = tessellate_solid(&solid).unwrap();
./features/12-cylinder/final-review.md:3104:        let mesh1 = tessellate_solid(&solid1).unwrap();
./features/12-cylinder/final-review.md:3105:        let mesh2 = tessellate_solid(&solid2).unwrap();
./features/12-cylinder/design-review-2.md:138:  pub fn tessellate_solid(solid) -> TriangleMesh           // = _with(solid, &default())  ※既存呼び出し・既存テスト不変
./features/12-cylinder/design-review-2.md:192:| T06 | テッセレーション正常系 | `tessellate_solid(cyl)` の三角形数 = angular*2(側面) + (angular-2)*2(キャップ) 等、既定 angular=32 で算出した期待値 | `triangle_count` 一致 |
./features/12-cylinder/design-review-2.md:607:pub fn tessellate_solid(solid: &Solid) -> TriangleMesh {
./features/12-cylinder/design-review-2.md:663:        let mesh = tessellate_solid(&solid);
./features/12-cylinder/design-review-2.md:681:        let mesh1 = tessellate_solid(&solid1);
./features/12-cylinder/design-review-2.md:682:        let mesh2 = tessellate_solid(&solid2);
./features/12-cylinder/design-review-2.md:1030:    let mesh = tessellate_solid(&solid);
./features/12-cylinder/design-review-2.md:1888:./features/12-cylinder/design-review.md:528:crates/mycad-kernel/src/tessellation/mod.rs:49:pub fn tessellate_solid(solid: &Solid) -> TriangleMesh {
./features/12-cylinder/design-review-2.md:1894:./features/12-cylinder/design-review.md:534:crates/mycad-kernel/src/tessellation/mod.rs:105:        let mesh = tessellate_solid(&solid);
./features/12-cylinder/design-review-2.md:1899:./features/12-cylinder/design-review.md:539:crates/mycad-kernel/src/tessellation/mod.rs:123:        let mesh1 = tessellate_solid(&solid1);
./features/12-cylinder/design-review-2.md:1900:./features/12-cylinder/design-review.md:540:crates/mycad-kernel/src/tessellation/mod.rs:124:        let mesh2 = tessellate_solid(&solid2);
./features/12-cylinder/design-review-2.md:2230:        let mesh = tessellate_solid(&solid);
./features/12-cylinder/design-review-2.md:2247:            to_ascii_stl(&tessellate_solid(&s), "cube")
./features/12-cylinder/design-review.md:137:  pub fn tessellate_solid(solid) -> TriangleMesh           // = _with(solid, &default())  ※既存呼び出し・既存テスト不変
./features/12-cylinder/design-review.md:181:| T06 | テッセレーション正常系 | `tessellate_solid(cyl)` の三角形数 = angular*2(側面) + (angular-2)*2(キャップ) 等、既定 angular=32 で算出した期待値 | `triangle_count` 一致 |
./features/12-cylinder/design-review.md:458:features/12-cylinder/design-review.md:137:  pub fn tessellate_solid(solid) -> TriangleMesh           // = _with(solid, &default())  ※既存呼び出し・既存テスト不変
./features/12-cylinder/design-review.md:464:features/12-cylinder/design-review.md:181:| T06 | テッセレーション正常系 | `tessellate_solid(cyl)` の三角形数 = angular*2(側面) + (angular-2)*2(キャップ) 等、既定 angular=32 で算出した期待値 | `triangle_count` 一致 |
./features/12-cylinder/design-review.md:507:crates/mycad-cli/src/main.rs:46:    let mesh = tessellate_solid(&solid);
./features/12-cylinder/design-review.md:528:crates/mycad-kernel/src/tessellation/mod.rs:49:pub fn tessellate_solid(solid: &Solid) -> TriangleMesh {
./features/12-cylinder/design-review.md:534:crates/mycad-kernel/src/tessellation/mod.rs:105:        let mesh = tessellate_solid(&solid);
./features/12-cylinder/design-review.md:539:crates/mycad-kernel/src/tessellation/mod.rs:123:        let mesh1 = tessellate_solid(&solid1);
./features/12-cylinder/design-review.md:540:crates/mycad-kernel/src/tessellation/mod.rs:124:        let mesh2 = tessellate_solid(&solid2);
./features/12-cylinder/design-review.md:549:crates/mycad-kernel/src/tessellation/stl.rs:59:        let mesh = tessellate_solid(&solid);
./features/12-cylinder/design-review.md:552:crates/mycad-kernel/src/tessellation/stl.rs:76:            to_ascii_stl(&tessellate_solid(&s), "cube")
./features/12-cylinder/design-review.md:579:crates/mycad-kernel/benches/tessellation.rs:16:        let _mesh = tessellate_solid(&solid);
./features/12-cylinder/design-review.md:585:crates/mycad-kernel/tests/cuboid_roundtrip.rs:17:    let mesh = tessellate_solid(&solid);
./features/12-cylinder/design-review.md:588:crates/mycad-kernel/tests/cuboid_roundtrip.rs:32:        tessellate_solid(&solid)
./features/12-cylinder/design-review.md:1220:pub fn tessellate_solid(solid: &Solid) -> TriangleMesh {
./features/12-cylinder/design-review.md:1276:        let mesh = tessellate_solid(&solid);
./features/12-cylinder/design-review.md:1294:        let mesh1 = tessellate_solid(&solid1);
./features/12-cylinder/design-review.md:1295:        let mesh2 = tessellate_solid(&solid2);
./features/12-cylinder/design-review.md:1647:    let mesh = tessellate_solid(&solid);
./features/12-cylinder/design-review.md:2277:    let mesh = tessellate_solid(&solid);
./features/12-cylinder/design-review.md:2292:        tessellate_solid(&solid)
./crates/mycad-kernel/tests/cuboid_roundtrip.rs:17:    let mesh = tessellate_solid(&solid).unwrap();
./crates/mycad-kernel/tests/cuboid_roundtrip.rs:32:        tessellate_solid(&solid).unwrap()
./crates/mycad-kernel/benches/tessellation.rs:16:        let _mesh = tessellate_solid(&solid).unwrap();
./crates/mycad-kernel/src/tessellation/stl.rs:59:        let mesh = tessellate_solid(&solid).unwrap();
./crates/mycad-kernel/src/tessellation/stl.rs:76:            to_ascii_stl(&tessellate_solid(&s).unwrap(), "cube")
./crates/mycad-kernel/src/tessellation/mod.rs:84:pub fn tessellate_solid(solid: &Solid) -> Result<TriangleMesh, TessellationError> {
./crates/mycad-kernel/src/tessellation/mod.rs:297:        let mesh = tessellate_solid(&solid).unwrap();
./crates/mycad-kernel/src/tessellation/mod.rs:320:        let m1 = tessellate_solid(&s1).unwrap();
./crates/mycad-kernel/src/tessellation/mod.rs:321:        let m2 = tessellate_solid(&s2).unwrap();
./crates/mycad-kernel/src/tessellation/mod.rs:333:        let mesh = tessellate_solid(&solid).unwrap();
./crates/mycad-kernel/src/tessellation/mod.rs:350:        let m1 = tessellate_solid(&s1).unwrap();
./crates/mycad-kernel/src/tessellation/mod.rs:351:        let m2 = tessellate_solid(&s2).unwrap();
./crates/mycad-kernel/src/tessellation/mod.rs:363:        let mesh = tessellate_solid(&solid).unwrap();
./crates/mycad-kernel/src/tessellation/mod.rs:418:        let mesh_default = tessellate_solid(&solid).unwrap();
./crates/mycad-kernel/src/tessellation/mod.rs:463:        let result = tessellate_solid(&s);
./crates/mycad-kernel/src/tessellation/mod.rs:475:        let mesh = tessellate_solid(&solid).unwrap();
./crates/mycad-kernel/src/tessellation/mod.rs:491:        let mesh1 = tessellate_solid(&solid1).unwrap();
./crates/mycad-kernel/src/tessellation/mod.rs:492:        let mesh2 = tessellate_solid(&solid2).unwrap();

exec
/bin/bash -lc 'git show 360f324''^:crates/mycad-kernel/src/tessellation/mod.rs | sed -n '"'1,220p'" in /home/bacon/mycad
 succeeded in 0ms:
pub mod stl;
pub use stl::to_ascii_stl;

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
/// Each face's outer loop is triangulated as a fan from its first vertex.
/// Normals are evaluated per vertex from the underlying surface, so curved
/// faces get correct shading rather than a single per-face normal.
///
/// Not yet handled: interior subdivision of curved faces (uv sampling) — a
/// curved face is still approximated by its boundary fan — and inner loops
/// (holes). Both are deferred to the curved-primitive work (Phase 3).
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

        // Add vertices and triangulate as a fan from vertex 0.
        // Normals are evaluated per vertex from the surface so curved faces
        // are shaded correctly (planar faces yield a constant normal).
        let base_idx = mesh.positions.len() as u32;

        for &v in &loop_vertices {
            let normal = face.surface.normal_at_point(v);
            let n = if face.same_sense {
                [normal.x, normal.y, normal.z]
            } else {
                [-normal.x, -normal.y, -normal.z]
            };
            mesh.positions.push([v.x, v.y, v.z]);
            mesh.normals.push(n);
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

exec
/bin/bash -lc "grep -n \"TODO\\|FIXME\\|bug\\|issue\\|concern\\|warning\" -i features/12-cylinder/final-review.md | head -80" in /home/bacon/mycad
 succeeded in 0ms:
217: #[derive(Debug, Error)]
241: #[derive(Debug, Clone, Serialize, Deserialize)]
302:+#[derive(Debug, Clone, Copy, PartialEq, Eq)]
313: #[derive(Debug, Clone, Serialize, Deserialize)]
967: #[derive(Debug, Clone, Serialize, Deserialize)]
1009:+#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
1034:+#[derive(Debug, Error)]
1656:#[derive(Debug, Clone, Serialize, Deserialize)]
1665:#[derive(Debug, Clone, Serialize, Deserialize)]
1678:#[derive(Debug, Clone, Serialize, Deserialize)]
1690:#[derive(Debug, Clone, Serialize, Deserialize)]
1699:#[derive(Debug, Clone, Serialize, Deserialize)]
1713:#[derive(Debug, Clone, Serialize, Deserialize)]
1724:#[derive(Debug, Clone, Serialize, Deserialize)]
1829:#[derive(Debug, Clone)]
2493:#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
2502:#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
2627:#[derive(Debug, Clone, Serialize, Deserialize)]
2659:#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
2684:#[derive(Debug, Error)]
3361:#[derive(Debug, Clone, Serialize, Deserialize)]

exec
/bin/bash -lc "grep -n \"P[0-3]\\|risk\\|incorrect\\|unsupported\\|clamp\\|same_sense\\|inner_loops\" -i features/12-cylinder/final-review.md | head -120" in /home/bacon/mycad
 succeeded in 0ms:
46:    - Surface::tessellation_strategy() -> TessellationStrategy enum (Plane/Cylinder/Unsupported)
47:    - TessellationError propagates to CLI — no silent face-drop on unsupported surfaces
88:-        Feature::CreateCylinder { .. } => Err(KernelError::UnsupportedFeature {
92:         Feature::CreateSphere { .. } => Err(KernelError::UnsupportedFeature {
95:         Feature::Extrude { .. } => Err(KernelError::UnsupportedFeature { kind: "extrude" }),
96:         Feature::Cut { .. } => Err(KernelError::UnsupportedFeature { kind: "cut" }),
97:         Feature::Fuse { .. } => Err(KernelError::UnsupportedFeature { kind: "fuse" }),
98:         Feature::Intersect { .. } => Err(KernelError::UnsupportedFeature { kind: "intersect" }),
225:     #[error("unsupported feature variant: {kind}")]
226:     UnsupportedFeature { kind: &'static str },
309:+    Unsupported,
444:                 let v = (d.z / radius).clamp(-1.0, 1.0).asin();
474:+            Surface::Sphere { .. } | Surface::Cone { .. } => TessellationStrategy::Unsupported,
540:         let p1 = surface.evaluate(0.0, 0.0);
541:         let p2 = surface.evaluate(std::f64::consts::FRAC_PI_2, 0.3);
542:         let n1 = surface.normal_at_point(&p1);
543:         let n2 = surface.normal_at_point(&p2);
1036:+    #[error("unsupported surface type: {kind}")]
1037:+    UnsupportedSurface { kind: &'static str },
1039:+    TrimmedFaceUnsupported,
1079:+            TessellationStrategy::Unsupported => {
1080:+                return Err(TessellationError::UnsupportedSurface {
1113:+        let n = if face.same_sense {
1167:+    if !face.inner_loops.is_empty() {
1168:+        return Err(TessellationError::TrimmedFaceUnsupported);
1204:             let n = if face.same_sense {
1239:+    let p0: [f64; 3] = mesh.positions[i0 as usize];
1240:+    let p1: [f64; 3] = mesh.positions[i1 as usize];
1241:+    let p2: [f64; 3] = mesh.positions[i2 as usize];
1242:+    let u = [p1[0] - p0[0], p1[1] - p0[1], p1[2] - p0[2]];
1243:+    let v = [p2[0] - p0[0], p2[1] - p0[1], p2[2] - p0[2]];
1360:+            let p0 = mesh.positions[i0];
1361:+            let p1 = mesh.positions[i1];
1362:+            let p2 = mesh.positions[i2];
1366:+                (p0[0] + p1[0] + p2[0]) / 3.0,
1367:+                (p0[1] + p1[1] + p2[1]) / 3.0,
1368:+                (p0[2] + p1[2] + p2[2]) / 3.0,
1372:+            let u = [p1[0] - p0[0], p1[1] - p0[1], p1[2] - p0[2]];
1373:+            let v = [p2[0] - p0[0], p2[1] - p0[1], p2[2] - p0[2]];
1392:+    /// T12: Resolution externalization + clamping.
1394:+    fn test_tessellation_options_clamp() {
1418:+    /// T15: Unsupported surface type → error.
1420:+    fn test_unsupported_surface_error() {
1453:+            Err(TessellationError::UnsupportedSurface { kind: "sphere" })
1510:         let p0 = mesh.positions[i0];
1511:         let p1 = mesh.positions[i1];
1512:         let p2 = mesh.positions[i2];
1514:         let n = face_normal(p0, p1, p2);
1518:         writeln!(out, "      vertex {} {} {}", p0[0], p0[1], p0[2]).unwrap();
1519:         writeln!(out, "      vertex {} {} {}", p1[0], p1[1], p1[2]).unwrap();
1520:         writeln!(out, "      vertex {} {} {}", p2[0], p2[1], p2[2]).unwrap();
1529: fn face_normal(p0: [f64; 3], p1: [f64; 3], p2: [f64; 3]) -> [f64; 3] {
1530:     let u = [p1[0] - p0[0], p1[1] - p0[1], p1[2] - p0[2]];
1531:     let v = [p2[0] - p0[0], p2[1] - p0[1], p2[2] - p0[2]];
1707:    pub inner_loops: Vec<usize>,
1709:    pub same_sense: bool,
1805:        inner_loops: Vec<usize>,
1806:        same_sense: bool,
1813:            inner_loops,
1814:            same_sense,
2249:            let p0 = solid.vertices[v[v0]].point;
2250:            let p1 = solid.vertices[v[v1]].point;
2251:            let direction = p1 - p0;
2256:                    origin: p0,
2271:        same_sense: bool,
2289:            same_sense: true,
2305:            same_sense: true,
2321:            same_sense: true,
2337:            same_sense: true,
2353:            same_sense: true,
2369:            same_sense: true,
2390:            face_def.same_sense,
2686:    #[error("unsupported surface type: {kind}")]
2687:    UnsupportedSurface { kind: &'static str },
2689:    TrimmedFaceUnsupported,
2715:            TessellationStrategy::Unsupported => {
2716:                return Err(TessellationError::UnsupportedSurface {
2745:        let n = if face.same_sense {
2797:    if !face.inner_loops.is_empty() {
2798:        return Err(TessellationError::TrimmedFaceUnsupported);
2834:            let n = if face.same_sense {
2863:    let p0: [f64; 3] = mesh.positions[i0 as usize];
2864:    let p1: [f64; 3] = mesh.positions[i1 as usize];
2865:    let p2: [f64; 3] = mesh.positions[i2 as usize];
2866:    let u = [p1[0] - p0[0], p1[1] - p0[1], p1[2] - p0[2]];
2867:    let v = [p2[0] - p0[0], p2[1] - p0[1], p2[2] - p0[2]];
2986:            let p0 = mesh.positions[i0];
2987:            let p1 = mesh.positions[i1];
2988:            let p2 = mesh.positions[i2];
2992:                (p0[0] + p1[0] + p2[0]) / 3.0,
2993:                (p0[1] + p1[1] + p2[1]) / 3.0,
2994:                (p0[2] + p1[2] + p2[2]) / 3.0,
2998:            let u = [p1[0] - p0[0], p1[1] - p0[1], p1[2] - p0[2]];
2999:            let v = [p2[0] - p0[0], p2[1] - p0[1], p2[2] - p0[2]];
3018:    /// T12: Resolution externalization + clamping.
3020:    fn test_tessellation_options_clamp() {
3044:    /// T15: Unsupported surface type → error.
3046:    fn test_unsupported_surface_error() {
3079:            Err(TessellationError::UnsupportedSurface { kind: "sphere" })
3428:            let n = if face.same_sense {

exec
/bin/bash -lc "grep -n \"u_min\\|2π\\|angular_segments\\|public\" -i features/12-cylinder/final-review.md | head -80" in /home/bacon/mycad
 succeeded in 0ms:
359:                 // u = longitude [0, 2π), v = latitude [-π/2, π/2]
1011:+    pub angular_segments: usize,
1018:+            angular_segments: 32,
1025:+    pub fn new(angular_segments: usize, axial_segments: usize) -> Self {
1027:+            angular_segments: angular_segments.max(3),
1103:+    let loop_points = collect_loop_points(solid, outer_loop, opts.angular_segments)?;
1172:+    // UvGridFullPatch assumes a full 2π revolution in u starting at u=0;
1173:+    // we do not sample the circle arcs here — the grid covers [0,2π] directly.
1186:+    let u_min = 0.0_f64;
1188:+    let n_u = opts.angular_segments;
1194:+    // u goes from u_min to u_min + 2π, v goes from v_min to v_max
1200:+            let u = u_min + du * iu as f64;
1396:+        assert_eq!(opts.angular_segments, 3);
2661:    pub angular_segments: usize,
2668:            angular_segments: 32,
2675:    pub fn new(angular_segments: usize, axial_segments: usize) -> Self {
2677:            angular_segments: angular_segments.max(3),
2735:    let loop_points = collect_loop_points(solid, outer_loop, opts.angular_segments)?;
2802:    // UvGridFullPatch assumes a full 2π revolution in u starting at u=0;
2803:    // we do not sample the circle arcs here — the grid covers [0,2π] directly.
2816:    let u_min = 0.0_f64;
2818:    let n_u = opts.angular_segments;
2824:    // u goes from u_min to u_min + 2π, v goes from v_min to v_max
2830:            let u = u_min + du * iu as f64;
3022:        assert_eq!(opts.angular_segments, 3);

exec
/bin/bash -lc "grep -R \"same_sense: false\" -n crates | sed -n '1,80p'" in /home/bacon/mycad
 succeeded in 0ms:
exec
/bin/bash -lc "grep -R \"Surface::Cylinder\" -n crates | sed -n '1,80p'" in /home/bacon/mycad
 succeeded in 0ms:
crates/mycad-kernel/src/primitives/cylinder.rs:99:    // Lateral face: Surface::Cylinder, loop = 4 HE
crates/mycad-kernel/src/primitives/cylinder.rs:112:        Surface::Cylinder {
crates/mycad-kernel/src/primitives/cylinder.rs:289:            Surface::Cylinder {
crates/mycad-kernel/src/primitives/cylinder.rs:364:        let s = Surface::Cylinder {
crates/mycad-kernel/src/geometry/surface.rs:53:            Surface::Cylinder {
crates/mycad-kernel/src/geometry/surface.rs:87:            Surface::Cylinder { axis, .. } => {
crates/mycad-kernel/src/geometry/surface.rs:137:            Surface::Cylinder { origin, axis, .. } => {
crates/mycad-kernel/src/geometry/surface.rs:177:            Surface::Cylinder { .. } => TessellationStrategy::UvGridFullPatch,
crates/mycad-kernel/src/geometry/surface.rs:208:        let surface = Surface::Cylinder {
crates/mycad-kernel/src/tessellation/mod.rs:277:            Surface::Cylinder { .. } => "cylinder",

exec
/bin/bash -lc "grep -n \"問題\\|issue\\|bug\\|fix\\|nit\\|懸念\\|risk\" -i features/12-cylinder/design-review-3.md | head -120" in /home/bacon/mycad
 succeeded in 0ms:
17:stdin に渡された設計・テスト計画ドキュメントを読み、以下の観点で問題を指摘してください。
36:- `Debug, Clone, Serialize, Deserialize` が付与されているか（JsonSchema が必要な型は追記）
57:issues:
71:`issues` が空の場合は `issues: []` と書く。
75:# Issue #12: 円柱プリミティブ + 曲面テッセレーション土台
79:ROADMAP Phase 3「円柱・球・押し出しが作れる」の最初の一歩。現状カーネルは `make_cuboid` と平面専用の `tessellate_solid` しか持たない。本 Issue で **解析曲面(円柱)を厳密 B-rep として生成し、それを三角形メッシュへ離散化する土台**を確立し、`mycad export examples/cylinder.mycad -o out.stl` を動作させる。
88:- Issue: #12 / Milestone: Phase 3
127:**退化入力の検証（R02）**: `.mycad` 由来のユーザー入力は境界で弾く。`KernelError` に `InvalidParameter { kind: &'static str }` を追加し、`build_solid_from_features` の `CreateCylinder` 分岐で `radius > 0.0 && height > 0.0` を検証（非正なら `InvalidParameter` を返し `make_cylinder` を呼ばない）。`make_cylinder` 自体は `make_cuboid` と同じ `-> Solid` シグネチャを保ち、正値前提（検証は呼び出し側＝境界に集約）。前提条件はドキュメントコメントに明記し、`debug_assert!(radius > 0.0 && height > 0.0)` を先頭に置いて開発時の契約違反を検出（リリースでは境界検証済み前提で素通り）。解像度の退化は `TessellationOptions` のクランプ（前述）で吸収。
146:- **トリム面ガード（R01）**: UvGrid 経路は「外側 loop がパラメータ箱全体を覆う**非トリム**面」を前提とする。`face.inner_loops` が非空（穴あき）の面は #12 のスコープ外なので、`UvGrid` 経路で穴を**無視して静かに塗りつぶさない** — `debug_assert!(face.inner_loops.is_empty())` で開発時に検出し、リリースでも穴を含む面は処理対象外として明示スキップ（誤った中実メッシュを出さない）。トリム済み外側 loop を持つ面が出るのは Phase 3後半/4（Boolean・押し出し断面）で、そこで trim 対応メッシャを `UvGrid` とは別 strategy として追加する。
171:- `TessellationOptions` に `Debug, Clone, Serialize, Deserialize`（既存 `TriangleMesh` と同様）。`Copy` も可。
185:- R01(high) 反映: `is_planar() -> bool` の二分岐は将来 variant を既定経路へ静かに流す危険がある → `tessellation_strategy() -> TessellationStrategy`（enum・網羅 match）へ変更。さらに UvGrid 経路にトリム面ガード（`inner_loops` 非空はスキップ＋`debug_assert`）を追加し、穴あき面の誤メッシュ化を防止（§2）。
187:- R04(再掲) 反映: `make_cylinder` に `debug_assert!(radius>0 && height>0)` と前提コメントを追加（境界検証は呼び出し側に集約のまま）。
389:#[derive(Debug, Clone, Serialize, Deserialize)]
398:#[derive(Debug, Clone, Serialize, Deserialize)]
411:#[derive(Debug, Clone, Serialize, Deserialize)]
423:#[derive(Debug, Clone, Serialize, Deserialize)]
432:#[derive(Debug, Clone, Serialize, Deserialize)]
446:#[derive(Debug, Clone, Serialize, Deserialize)]
457:#[derive(Debug, Clone, Serialize, Deserialize)]
562:#[derive(Debug, Clone)]
691:    // plus the surface definition.
878:#[derive(Debug, Clone, Serialize, Deserialize)]
1009:#[derive(Debug, Clone, Serialize, Deserialize)]
1011:    /// An infinite plane.
1257:#[derive(Debug, Clone, Serialize, Deserialize)]
1294:#[derive(Debug, Error)]
1441:    let tmp = tempfile::NamedTempFile::with_suffix(".stl").expect("tempfile");
1509:crates/mycad-format/src/feature.rs:5:#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
1510:crates/mycad-format/src/feature.rs:14:#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
1515:crates/mycad-format/src/component.rs:8:#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
1522:crates/mycad-format/src/component.rs:96:#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
1525:crates/mycad-format/src/document.rs:8:#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
1564:crates/mycad-kernel/src/brep/topology.rs:11:#[derive(Debug, Clone, Serialize, Deserialize)]
1565:crates/mycad-kernel/src/brep/topology.rs:20:#[derive(Debug, Clone, Serialize, Deserialize)]
1566:crates/mycad-kernel/src/brep/topology.rs:33:#[derive(Debug, Clone, Serialize, Deserialize)]
1567:crates/mycad-kernel/src/brep/topology.rs:45:#[derive(Debug, Clone, Serialize, Deserialize)]
1568:crates/mycad-kernel/src/brep/topology.rs:54:#[derive(Debug, Clone, Serialize, Deserialize)]
1569:crates/mycad-kernel/src/brep/topology.rs:68:#[derive(Debug, Clone, Serialize, Deserialize)]
1570:crates/mycad-kernel/src/brep/topology.rs:79:#[derive(Debug, Clone, Serialize, Deserialize)]
1572:crates/mycad-kernel/src/geometry/curve.rs:6:#[derive(Debug, Clone, Serialize, Deserialize)]
1574:crates/mycad-kernel/src/geometry/mod.rs:15:#[derive(Debug, Clone, Serialize, Deserialize)]
1576:crates/mycad-kernel/src/tessellation/mod.rs:9:#[derive(Debug, Clone, Serialize, Deserialize)]
1578:crates/mycad-kernel/src/geometry/surface.rs:6:#[derive(Debug, Clone, Serialize, Deserialize)]
1634:/// Given a normal vector, computes two perpendicular unit vectors
1680:#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
1689:#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
1804:#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
2143:issues:
2162:    finding: "`radius > 0.0 && height > 0.0` の検証を `build_solid_from_features` にだけ置き、`make_cylinder` 自体は `debug_assert!` のみとしているため、公開カーネル API を直接使う呼び出し元は非正値や非有限値で退化 Solid を生成できる。"
2163:    suggestion: "`make_cylinder` も fallible にするか、少なくとも `is_finite()` と正値チェックをカーネル境界で強制して、ビルド層の検証はその前段の利便層として重ねること"
2173:issues:
2192:    finding: "`radius > 0.0 && height > 0.0` の検証を `build_solid_from_features` にだけ置き、`make_cylinder` 自体は `debug_assert!` のみとしているため、公開カーネル API を直接使う呼び出し元は非正値や非有限値で退化 Solid を生成できる。"
2193:    suggestion: "`make_cylinder` も fallible にするか、少なくとも `is_finite()` と正値チェックをカーネル境界で強制して、ビルド層の検証はその前段の利便層として重ねること"

exec
/bin/bash -lc "sed -n '2130,2210p' features/12-cylinder/design-review-3.md" in /home/bacon/mycad
 succeeded in 0ms:

    #[test]
    fn ascii_stl_deterministic() {
        let run = || {
            let mut g = IdGenerator::new(0);
            let s = make_cuboid(1.0, 1.0, 1.0, &mut g);
            to_ascii_stl(&tessellate_solid(&s), "cube")
        };
        assert_eq!(run(), run());
    }
}

codex
issues:
  - id: R01
    severity: critical
    section: "設計方針 > 2. テッセレータ > トリム面ガード"
    finding: "UvGrid 経路で未対応の面を release では『明示スキップ』する設計になっており、`mycad export` が成功したまま面欠落の開いた STL を静かに出力しうる。これは watertight 性と出力完全性を壊す。"
    suggestion: "`tessellate_solid_with` は unsupported face をスキップせず失敗を返す設計に変更し、`TessellationError` もしくは `KernelError` 経由で CLI まで明示的に伝播させること"
  - id: R02
    severity: high
    section: "設計方針 > 2. テッセレータ > UvGrid"
    finding: "`face.inner_loops.is_empty()` だけでは『非トリムのパラメータ矩形面』を判定できない。外側 loop だけで構成された trimmed face でも UvGrid がその UV bbox 全体を塗り、実面積より大きい誤メッシュを生成する。"
    suggestion: "UvGrid を『矩形 UV パッチであることを証明できる面』に限定すること。少なくとも #12 では `make_cylinder` の側面のような既知トポロジーだけに適用し、一般 face には使わないか、矩形トリム判定を別途導入すること"
  - id: R03
    severity: medium
    section: "設計方針 > 2. テッセレータ > Surface::tessellation_strategy"
    finding: "`Sphere` と `Cone` も `UvGrid` に割り当てているが、提案アルゴリズムとテスト計画は円柱前提で、球の極や円錐 apex の特異点処理が未設計・未検証のまま公開 API の振る舞いだけ先に広がっている。"
    suggestion: "#12 の範囲では `Plane` と `Cylinder` のみを明示対応に留め、`Sphere`/`Cone` は未対応エラーにするか、特異点専用の分割規則とテストを用意してから有効化すること"
  - id: R04
    severity: medium
    section: "設計方針 > 1. トポロジー / 退化入力の検証"
    finding: "`radius > 0.0 && height > 0.0` の検証を `build_solid_from_features` にだけ置き、`make_cylinder` 自体は `debug_assert!` のみとしているため、公開カーネル API を直接使う呼び出し元は非正値や非有限値で退化 Solid を生成できる。"
    suggestion: "`make_cylinder` も fallible にするか、少なくとも `is_finite()` と正値チェックをカーネル境界で強制して、ビルド層の検証はその前段の利便層として重ねること"
  - id: R05
    severity: low
    section: "テスト計画 > T01"
    finding: "T01 の『全 ID・座標・面が一致を `assert_eq!`』という記述は、現行の `Solid`/`Vertex`/`Face` などが `PartialEq` を derive していないため、そのままでは実装できない。"
    suggestion: "比較対象を ID 列・座標列・面構成のフィールド単位に分解するか、必要ならテスト専用に比較ヘルパーを用意すること"

verdict: fail
tokens used
94,395
issues:
  - id: R01
    severity: critical
    section: "設計方針 > 2. テッセレータ > トリム面ガード"
    finding: "UvGrid 経路で未対応の面を release では『明示スキップ』する設計になっており、`mycad export` が成功したまま面欠落の開いた STL を静かに出力しうる。これは watertight 性と出力完全性を壊す。"
    suggestion: "`tessellate_solid_with` は unsupported face をスキップせず失敗を返す設計に変更し、`TessellationError` もしくは `KernelError` 経由で CLI まで明示的に伝播させること"
  - id: R02
    severity: high
    section: "設計方針 > 2. テッセレータ > UvGrid"
    finding: "`face.inner_loops.is_empty()` だけでは『非トリムのパラメータ矩形面』を判定できない。外側 loop だけで構成された trimmed face でも UvGrid がその UV bbox 全体を塗り、実面積より大きい誤メッシュを生成する。"
    suggestion: "UvGrid を『矩形 UV パッチであることを証明できる面』に限定すること。少なくとも #12 では `make_cylinder` の側面のような既知トポロジーだけに適用し、一般 face には使わないか、矩形トリム判定を別途導入すること"
  - id: R03
    severity: medium
    section: "設計方針 > 2. テッセレータ > Surface::tessellation_strategy"
    finding: "`Sphere` と `Cone` も `UvGrid` に割り当てているが、提案アルゴリズムとテスト計画は円柱前提で、球の極や円錐 apex の特異点処理が未設計・未検証のまま公開 API の振る舞いだけ先に広がっている。"
    suggestion: "#12 の範囲では `Plane` と `Cylinder` のみを明示対応に留め、`Sphere`/`Cone` は未対応エラーにするか、特異点専用の分割規則とテストを用意してから有効化すること"
  - id: R04
    severity: medium
    section: "設計方針 > 1. トポロジー / 退化入力の検証"
    finding: "`radius > 0.0 && height > 0.0` の検証を `build_solid_from_features` にだけ置き、`make_cylinder` 自体は `debug_assert!` のみとしているため、公開カーネル API を直接使う呼び出し元は非正値や非有限値で退化 Solid を生成できる。"
    suggestion: "`make_cylinder` も fallible にするか、少なくとも `is_finite()` と正値チェックをカーネル境界で強制して、ビルド層の検証はその前段の利便層として重ねること"
  - id: R05
    severity: low
    section: "テスト計画 > T01"
    finding: "T01 の『全 ID・座標・面が一致を `assert_eq!`』という記述は、現行の `Solid`/`Vertex`/`Face` などが `PartialEq` を derive していないため、そのままでは実装できない。"
    suggestion: "比較対象を ID 列・座標列・面構成のフィールド単位に分解するか、必要ならテスト専用に比較ヘルパーを用意すること"

verdict: fail

exec
/bin/bash -lc "grep -n \"codex\\|issues:\" -n features/12-cylinder/final-review.md | tail -20" in /home/bacon/mycad
 succeeded in 0ms:
3664:57:issues:
3665:71:`issues` が空の場合は `issues: []` と書く。
3709:2143:issues:
3712:2173:issues:
3731:codex
3732:issues:
3762:issues:

exec
/bin/bash -lc "sed -n '3720,3785p' features/12-cylinder/final-review.md" in /home/bacon/mycad
 succeeded in 0ms:
    #[test]
    fn ascii_stl_deterministic() {
        let run = || {
            let mut g = IdGenerator::new(0);
            let s = make_cuboid(1.0, 1.0, 1.0, &mut g);
            to_ascii_stl(&tessellate_solid(&s), "cube")
        };
        assert_eq!(run(), run());
    }
}

codex
issues:
  - id: R01
    severity: critical
    section: "設計方針 > 2. テッセレータ > トリム面ガード"
    finding: "UvGrid 経路で未対応の面を release では『明示スキップ』する設計になっており、`mycad export` が成功したまま面欠落の開いた STL を静かに出力しうる。これは watertight 性と出力完全性を壊す。"
    suggestion: "`tessellate_solid_with` は unsupported face をスキップせず失敗を返す設計に変更し、`TessellationError` もしくは `KernelError` 経由で CLI まで明示的に伝播させること"
  - id: R02
    severity: high
    section: "設計方針 > 2. テッセレータ > UvGrid"
    finding: "`face.inner_loops.is_empty()` だけでは『非トリムのパラメータ矩形面』を判定できない。外側 loop だけで構成された trimmed face でも UvGrid がその UV bbox 全体を塗り、実面積より大きい誤メッシュを生成する。"
    suggestion: "UvGrid を『矩形 UV パッチであることを証明できる面』に限定すること。少なくとも #12 では `make_cylinder` の側面のような既知トポロジーだけに適用し、一般 face には使わないか、矩形トリム判定を別途導入すること"
  - id: R03
    severity: medium
    section: "設計方針 > 2. テッセレータ > Surface::tessellation_strategy"
    finding: "`Sphere` と `Cone` も `UvGrid` に割り当てているが、提案アルゴリズムとテスト計画は円柱前提で、球の極や円錐 apex の特異点処理が未設計・未検証のまま公開 API の振る舞いだけ先に広がっている。"
    suggestion: "#12 の範囲では `Plane` と `Cylinder` のみを明示対応に留め、`Sphere`/`Cone` は未対応エラーにするか、特異点専用の分割規則とテストを用意してから有効化すること"
  - id: R04
    severity: medium
    section: "設計方針 > 1. トポロジー / 退化入力の検証"
    finding: "`radius > 0.0 && height > 0.0` の検証を `build_solid_from_features` にだけ置き、`make_cylinder` 自体は `debug_assert!` のみとしているため、公開カーネル API を直接使う呼び出し元は非正値や非有限値で退化 Solid を生成できる。"
    suggestion: "`make_cylinder` も fallible にするか、少なくとも `is_finite()` と正値チェックをカーネル境界で強制して、ビルド層の検証はその前段の利便層として重ねること"
  - id: R05
    severity: low
    section: "テスト計画 > T01"
    finding: "T01 の『全 ID・座標・面が一致を `assert_eq!`』という記述は、現行の `Solid`/`Vertex`/`Face` などが `PartialEq` を derive していないため、そのままでは実装できない。"
    suggestion: "比較対象を ID 列・座標列・面構成のフィールド単位に分解するか、必要ならテスト専用に比較ヘルパーを用意すること"

verdict: fail
tokens used
94,395
issues:
  - id: R01
    severity: critical
    section: "設計方針 > 2. テッセレータ > トリム面ガード"
    finding: "UvGrid 経路で未対応の面を release では『明示スキップ』する設計になっており、`mycad export` が成功したまま面欠落の開いた STL を静かに出力しうる。これは watertight 性と出力完全性を壊す。"
    suggestion: "`tessellate_solid_with` は unsupported face をスキップせず失敗を返す設計に変更し、`TessellationError` もしくは `KernelError` 経由で CLI まで明示的に伝播させること"
  - id: R02
    severity: high
    section: "設計方針 > 2. テッセレータ > UvGrid"
    finding: "`face.inner_loops.is_empty()` だけでは『非トリムのパラメータ矩形面』を判定できない。外側 loop だけで構成された trimmed face でも UvGrid がその UV bbox 全体を塗り、実面積より大きい誤メッシュを生成する。"
    suggestion: "UvGrid を『矩形 UV パッチであることを証明できる面』に限定すること。少なくとも #12 では `make_cylinder` の側面のような既知トポロジーだけに適用し、一般 face には使わないか、矩形トリム判定を別途導入すること"
  - id: R03
    severity: medium
    section: "設計方針 > 2. テッセレータ > Surface::tessellation_strategy"
    finding: "`Sphere` と `Cone` も `UvGrid` に割り当てているが、提案アルゴリズムとテスト計画は円柱前提で、球の極や円錐 apex の特異点処理が未設計・未検証のまま公開 API の振る舞いだけ先に広がっている。"
    suggestion: "#12 の範囲では `Plane` と `Cylinder` のみを明示対応に留め、`Sphere`/`Cone` は未対応エラーにするか、特異点専用の分割規則とテストを用意してから有効化すること"
  - id: R04
    severity: medium
    section: "設計方針 > 1. トポロジー / 退化入力の検証"
    finding: "`radius > 0.0 && height > 0.0` の検証を `build_solid_from_features` にだけ置き、`make_cylinder` 自体は `debug_assert!` のみとしているため、公開カーネル API を直接使う呼び出し元は非正値や非有限値で退化 Solid を生成できる。"
    suggestion: "`make_cylinder` も fallible にするか、少なくとも `is_finite()` と正値チェックをカーネル境界で強制して、ビルド層の検証はその前段の利便層として重ねること"
  - id: R05
    severity: low
    section: "テスト計画 > T01"

exec
/bin/bash -lc "nl -ba crates/mycad-kernel/src/tessellation/mod.rs | sed -n '1,260p'" in /home/bacon/mycad
 succeeded in 0ms:
     1	pub mod stl;
     2	pub use stl::to_ascii_stl;
     3	
     4	use crate::brep::topology::Solid;
     5	use crate::geometry::surface::{Surface, TessellationStrategy};
     6	use crate::geometry::Point;
     7	use serde::{Deserialize, Serialize};
     8	use std::f64::consts::PI;
     9	use thiserror::Error;
    10	
    11	/// Seam jump detection threshold: |Δu| > π indicates a parameter-space seam crossing.
    12	/// Minimum cross-product norm for a non-degenerate triangle.
    13	const AREA_EPS: f64 = 1e-14;
    14	
    15	/// A triangle mesh for rendering.
    16	#[derive(Debug, Clone, Serialize, Deserialize)]
    17	pub struct TriangleMesh {
    18	    /// Vertex positions (x, y, z).
    19	    pub positions: Vec<[f64; 3]>,
    20	    /// Normal vectors per vertex.
    21	    pub normals: Vec<[f64; 3]>,
    22	    /// Triangle indices (every 3 indices form a triangle).
    23	    pub indices: Vec<u32>,
    24	}
    25	
    26	impl TriangleMesh {
    27	    pub fn new() -> Self {
    28	        Self {
    29	            positions: Vec::new(),
    30	            normals: Vec::new(),
    31	            indices: Vec::new(),
    32	        }
    33	    }
    34	
    35	    /// Number of triangles in the mesh.
    36	    pub fn triangle_count(&self) -> usize {
    37	        self.indices.len() / 3
    38	    }
    39	}
    40	
    41	impl Default for TriangleMesh {
    42	    fn default() -> Self {
    43	        Self::new()
    44	    }
    45	}
    46	
    47	/// Controls the resolution of curved-surface tessellation.
    48	#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
    49	pub struct TessellationOptions {
    50	    pub angular_segments: usize,
    51	    pub axial_segments: usize,
    52	}
    53	
    54	impl Default for TessellationOptions {
    55	    fn default() -> Self {
    56	        Self {
    57	            angular_segments: 32,
    58	            axial_segments: 1,
    59	        }
    60	    }
    61	}
    62	
    63	impl TessellationOptions {
    64	    pub fn new(angular_segments: usize, axial_segments: usize) -> Self {
    65	        Self {
    66	            angular_segments: angular_segments.max(3),
    67	            axial_segments: axial_segments.max(1),
    68	        }
    69	    }
    70	}
    71	
    72	/// Errors from tessellation.
    73	#[derive(Debug, Error)]
    74	pub enum TessellationError {
    75	    #[error("unsupported surface type: {kind}")]
    76	    UnsupportedSurface { kind: &'static str },
    77	    #[error("trimmed or punctured faces are not yet supported")]
    78	    TrimmedFaceUnsupported,
    79	    #[error("non-manifold loop detected")]
    80	    NonManifoldLoop,
    81	}
    82	
    83	/// Tessellate a B-rep solid into a triangle mesh using default options.
    84	pub fn tessellate_solid(solid: &Solid) -> Result<TriangleMesh, TessellationError> {
    85	    tessellate_solid_with(solid, &TessellationOptions::default())
    86	}
    87	
    88	/// Tessellate a B-rep solid into a triangle mesh with the given options.
    89	pub fn tessellate_solid_with(
    90	    solid: &Solid,
    91	    opts: &TessellationOptions,
    92	) -> Result<TriangleMesh, TessellationError> {
    93	    let mut mesh = TriangleMesh::new();
    94	
    95	    for face in &solid.faces {
    96	        let strategy = face.surface.tessellation_strategy();
    97	        match strategy {
    98	            TessellationStrategy::BoundaryFan => {
    99	                tessellate_face_fan(solid, face, opts, &mut mesh)?;
   100	            }
   101	            TessellationStrategy::UvGridFullPatch => {
   102	                tessellate_face_uv_grid(solid, face, opts, &mut mesh)?;
   103	            }
   104	            TessellationStrategy::Unsupported => {
   105	                return Err(TessellationError::UnsupportedSurface {
   106	                    kind: face.surface.kind_name(),
   107	                });
   108	            }
   109	        }
   110	    }
   111	
   112	    Ok(mesh)
   113	}
   114	
   115	/// Triangulate a face as a fan from its first boundary vertex (planar convex faces).
   116	fn tessellate_face_fan(
   117	    solid: &Solid,
   118	    face: &crate::brep::topology::Face,
   119	    opts: &TessellationOptions,
   120	    mesh: &mut TriangleMesh,
   121	) -> Result<(), TessellationError> {
   122	    let outer_loop = &solid.loops[face.outer_loop];
   123	
   124	    let loop_points = collect_loop_points(solid, outer_loop, opts.angular_segments)?;
   125	
   126	    if loop_points.len() < 3 {
   127	        return Ok(());
   128	    }
   129	
   130	    let base_idx = mesh.positions.len() as u32;
   131	
   132	    for p in &loop_points {
   133	        let normal = face.surface.normal_at_point(p);
   134	        let n = if face.same_sense {
   135	            [normal.x, normal.y, normal.z]
   136	        } else {
   137	            [-normal.x, -normal.y, -normal.z]
   138	        };
   139	        mesh.positions.push([p.x, p.y, p.z]);
   140	        mesh.normals.push(n);
   141	    }
   142	
   143	    for i in 1..(loop_points.len() as u32 - 1) {
   144	        mesh.indices.push(base_idx);
   145	        mesh.indices.push(base_idx + i);
   146	        mesh.indices.push(base_idx + i + 1);
   147	    }
   148	
   149	    Ok(())
   150	}
   151	
   152	/// Collect the ordered boundary points of a loop by sampling each HE's curve.
   153	fn collect_loop_points(
   154	    solid: &Solid,
   155	    lp: &crate::brep::topology::Loop,
   156	    segments: usize,
   157	) -> Result<Vec<Point>, TessellationError> {
   158	    let mut points = Vec::new();
   159	
   160	    for &he_idx in &lp.half_edges {
   161	        let he = &solid.half_edges[he_idx];
   162	        let edge = &solid.edges[he.edge];
   163	
   164	        let (t_start, t_end) = if he.forward {
   165	            (edge.t_range[0], edge.t_range[1])
   166	        } else {
   167	            (edge.t_range[1], edge.t_range[0])
   168	        };
   169	
   170	        let seg_points = edge.curve.sample_segment(t_start, t_end, segments);
   171	        for p in seg_points {
   172	            points.push(p);
   173	        }
   174	    }
   175	
   176	    Ok(points)
   177	}
   178	
   179	/// Triangulate a face using UV grid sampling (full untrimmed periodic/rectangular patch).
   180	fn tessellate_face_uv_grid(
   181	    solid: &Solid,
   182	    face: &crate::brep::topology::Face,
   183	    opts: &TessellationOptions,
   184	    mesh: &mut TriangleMesh,
   185	) -> Result<(), TessellationError> {
   186	    if !face.inner_loops.is_empty() {
   187	        return Err(TessellationError::TrimmedFaceUnsupported);
   188	    }
   189	
   190	    // Determine v range from loop corner vertices (start_vertex of each HE).
   191	    // UvGridFullPatch assumes a full 2π revolution in u starting at u=0;
   192	    // we do not sample the circle arcs here — the grid covers [0,2π] directly.
   193	    let outer_loop = &solid.loops[face.outer_loop];
   194	    let corner_uvs: Vec<(f64, f64)> = outer_loop
   195	        .half_edges
   196	        .iter()
   197	        .map(|&he_idx| {
   198	            let he = &solid.half_edges[he_idx];
   199	            face.surface.uv_of(&solid.vertices[he.start_vertex].point)
   200	        })
   201	        .collect();
   202	
   203	    let v_min = corner_uvs.iter().map(|(_, v)| *v).fold(f64::MAX, f64::min);
   204	    let v_max = corner_uvs.iter().map(|(_, v)| *v).fold(f64::MIN, f64::max);
   205	    let u_min = 0.0_f64;
   206	
   207	    let n_u = opts.angular_segments;
   208	    let n_v = opts.axial_segments;
   209	
   210	    let base_idx = mesh.positions.len() as u32;
   211	
   212	    // Sample UV grid: (n_u+1) × (n_v+1) vertices
   213	    // u goes from u_min to u_min + 2π, v goes from v_min to v_max
   214	    let du = 2.0 * PI / n_u as f64;
   215	    let dv = (v_max - v_min) / n_v as f64;
   216	
   217	    for iv in 0..=n_v {
   218	        for iu in 0..=n_u {
   219	            let u = u_min + du * iu as f64;
   220	            let v = v_min + dv * iv as f64;
   221	            let p = face.surface.evaluate(u, v);
   222	            let normal = face.surface.normal_at(u, v);
   223	            let n = if face.same_sense {
   224	                [normal.x, normal.y, normal.z]
   225	            } else {
   226	                [-normal.x, -normal.y, -normal.z]
   227	            };
   228	            mesh.positions.push([p.x, p.y, p.z]);
   229	            mesh.normals.push(n);
   230	        }
   231	    }
   232	
   233	    // Generate triangles for each cell
   234	    for iv in 0..n_v {
   235	        for iu in 0..n_u {
   236	            let i00 = base_idx + (iv * (n_u + 1) + iu) as u32;
   237	            let i10 = base_idx + (iv * (n_u + 1) + iu + 1) as u32;
   238	            let i01 = base_idx + ((iv + 1) * (n_u + 1) + iu) as u32;
   239	            let i11 = base_idx + ((iv + 1) * (n_u + 1) + iu + 1) as u32;
   240	
   241	            // Two triangles per cell, orientation so ∂u×∂v points outward
   242	            push_triangle(mesh, i00, i10, i01);
   243	            push_triangle(mesh, i10, i11, i01);
   244	        }
   245	    }
   246	
   247	    Ok(())
   248	}
   249	
   250	/// Push a triangle, but skip degenerate (zero-area) ones.
   251	fn push_triangle(mesh: &mut TriangleMesh, i0: u32, i1: u32, i2: u32) {
   252	    let p0: [f64; 3] = mesh.positions[i0 as usize];
   253	    let p1: [f64; 3] = mesh.positions[i1 as usize];
   254	    let p2: [f64; 3] = mesh.positions[i2 as usize];
   255	    let u = [p1[0] - p0[0], p1[1] - p0[1], p1[2] - p0[2]];
   256	    let v = [p2[0] - p0[0], p2[1] - p0[1], p2[2] - p0[2]];
   257	    let cross_norm = (u[1] * v[2] - u[2] * v[1]).powi(2)
   258	        + (u[2] * v[0] - u[0] * v[2]).powi(2)
   259	        + (u[0] * v[1] - u[1] * v[0]).powi(2).sqrt();
   260	    if cross_norm < AREA_EPS * AREA_EPS {

exec
/bin/bash -lc "nl -ba crates/mycad-kernel/src/tessellation/mod.rs | sed -n '260,520p'" in /home/bacon/mycad
 succeeded in 0ms:
   260	    if cross_norm < AREA_EPS * AREA_EPS {
   261	        return;
   262	    }
   263	    mesh.indices.push(i0);
   264	    mesh.indices.push(i1);
   265	    mesh.indices.push(i2);
   266	}
   267	
   268	/// Surface kind name for error messages.
   269	trait SurfaceKind {
   270	    fn kind_name(&self) -> &'static str;
   271	}
   272	
   273	impl SurfaceKind for Surface {
   274	    fn kind_name(&self) -> &'static str {
   275	        match self {
   276	            Surface::Plane { .. } => "plane",
   277	            Surface::Cylinder { .. } => "cylinder",
   278	            Surface::Sphere { .. } => "sphere",
   279	            Surface::Cone { .. } => "cone",
   280	        }
   281	    }
   282	}
   283	
   284	#[cfg(test)]
   285	mod tests {
   286	    use super::*;
   287	    use crate::brep::topology::IdGenerator;
   288	    use crate::geometry::Vec3;
   289	    use crate::primitives::make_cuboid;
   290	    use crate::primitives::make_cylinder;
   291	
   292	    /// T06: Normal tessellation — triangle count for cylinder with default 32 angular segments.
   293	    #[test]
   294	    fn test_tessellate_cylinder_triangle_count() {
   295	        let mut gen = IdGenerator::new(0);
   296	        let solid = make_cylinder(5.0, 20.0, &mut gen).unwrap();
   297	        let mesh = tessellate_solid(&solid).unwrap();
   298	
   299	        // Lateral: 32 angular segments × 1 axial × 2 triangles = 64
   300	        // Each cap: 32-gon fan = 30 triangles (32 - 2)
   301	        // Total: 64 + 30 + 30 = 124
   302	        let expected = 32 * 2 + (32 - 2) * 2;
   303	        assert_eq!(
   304	            mesh.triangle_count(),
   305	            expected,
   306	            "expected {expected} triangles, got {}",
   307	            mesh.triangle_count()
   308	        );
   309	    }
   310	
   311	    /// T07: Determinism — same input → same output.
   312	    #[test]
   313	    fn test_tessellation_cylinder_deterministic() {
   314	        let mut gen1 = IdGenerator::new(0);
   315	        let mut gen2 = IdGenerator::new(0);
   316	
   317	        let s1 = make_cylinder(5.0, 20.0, &mut gen1).unwrap();
   318	        let s2 = make_cylinder(5.0, 20.0, &mut gen2).unwrap();
   319	
   320	        let m1 = tessellate_solid(&s1).unwrap();
   321	        let m2 = tessellate_solid(&s2).unwrap();
   322	
   323	        assert_eq!(m1.positions, m2.positions);
   324	        assert_eq!(m1.normals, m2.normals);
   325	        assert_eq!(m1.indices, m2.indices);
   326	    }
   327	
   328	    /// T08: Regression — cuboid tessellation is byte-identical to pre-cylinder output.
   329	    #[test]
   330	    fn test_tessellate_cuboid_unchanged() {
   331	        let mut gen = IdGenerator::new(0);
   332	        let solid = make_cuboid(1.0, 1.0, 1.0, &mut gen);
   333	        let mesh = tessellate_solid(&solid).unwrap();
   334	
   335	        assert_eq!(mesh.triangle_count(), 12);
   336	        assert_eq!(mesh.positions.len(), 24);
   337	        assert_eq!(mesh.normals.len(), 24);
   338	        assert_eq!(mesh.indices.len(), 36);
   339	    }
   340	
   341	    /// T08 extended: exact positions and indices snapshot.
   342	    #[test]
   343	    fn test_cuboid_positions_indices_snapshot() {
   344	        let mut gen1 = IdGenerator::new(0);
   345	        let mut gen2 = IdGenerator::new(0);
   346	
   347	        let s1 = make_cuboid(1.0, 1.0, 1.0, &mut gen1);
   348	        let s2 = make_cuboid(1.0, 1.0, 1.0, &mut gen2);
   349	
   350	        let m1 = tessellate_solid(&s1).unwrap();
   351	        let m2 = tessellate_solid(&s2).unwrap();
   352	
   353	        // Byte-identical across two runs
   354	        assert_eq!(m1.positions, m2.positions);
   355	        assert_eq!(m1.indices, m2.indices);
   356	    }
   357	
   358	    /// T09: Outward normals — all facet normals point away from centroid.
   359	    #[test]
   360	    fn test_cylinder_outward_normals() {
   361	        let mut gen = IdGenerator::new(0);
   362	        let solid = make_cylinder(5.0, 20.0, &mut gen).unwrap();
   363	        let mesh = tessellate_solid(&solid).unwrap();
   364	
   365	        // Centroid of cylinder: (0, 0, 10)
   366	        let centroid = [0.0f64, 0.0, 10.0];
   367	
   368	        for tri in 0..mesh.triangle_count() {
   369	            let i0 = mesh.indices[tri * 3] as usize;
   370	            let i1 = mesh.indices[tri * 3 + 1] as usize;
   371	            let i2 = mesh.indices[tri * 3 + 2] as usize;
   372	
   373	            let p0 = mesh.positions[i0];
   374	            let p1 = mesh.positions[i1];
   375	            let p2 = mesh.positions[i2];
   376	
   377	            // Facet center
   378	            let center = [
   379	                (p0[0] + p1[0] + p2[0]) / 3.0,
   380	                (p0[1] + p1[1] + p2[1]) / 3.0,
   381	                (p0[2] + p1[2] + p2[2]) / 3.0,
   382	            ];
   383	
   384	            // Cross product for facet normal
   385	            let u = [p1[0] - p0[0], p1[1] - p0[1], p1[2] - p0[2]];
   386	            let v = [p2[0] - p0[0], p2[1] - p0[1], p2[2] - p0[2]];
   387	            let nx = u[1] * v[2] - u[2] * v[1];
   388	            let ny = u[2] * v[0] - u[0] * v[2];
   389	            let nz = u[0] * v[1] - u[1] * v[0];
   390	
   391	            // Vector from centroid to facet center
   392	            let dx = center[0] - centroid[0];
   393	            let dy = center[1] - centroid[1];
   394	            let dz = center[2] - centroid[2];
   395	
   396	            // Dot product should be positive (normal points outward)
   397	            let dot = nx * dx + ny * dy + nz * dz;
   398	            assert!(
   399	                dot > 0.0,
   400	                "facet {tri}: normal should point outward (dot={dot})"
   401	            );
   402	        }
   403	    }
   404	
   405	    /// T12: Resolution externalization + clamping.
   406	    #[test]
   407	    fn test_tessellation_options_clamp() {
   408	        let opts = TessellationOptions::new(2, 0);
   409	        assert_eq!(opts.angular_segments, 3);
   410	        assert_eq!(opts.axial_segments, 1);
   411	    }
   412	
   413	    #[test]
   414	    fn test_different_resolution_different_count() {
   415	        let mut gen = IdGenerator::new(0);
   416	        let solid = make_cylinder(5.0, 20.0, &mut gen).unwrap();
   417	
   418	        let mesh_default = tessellate_solid(&solid).unwrap();
   419	        let mesh_low = tessellate_solid_with(&solid, &TessellationOptions::new(8, 1)).unwrap();
   420	        let mesh_hi = tessellate_solid_with(&solid, &TessellationOptions::new(64, 2)).unwrap();
   421	
   422	        assert_ne!(mesh_default.triangle_count(), mesh_low.triangle_count());
   423	        assert_ne!(mesh_default.triangle_count(), mesh_hi.triangle_count());
   424	
   425	        // 8 angular, 1 axial: lateral=8*1*2=16, caps=(8-2)*2=12 → 28
   426	        assert_eq!(mesh_low.triangle_count(), 8 * 2 + (8 - 2) * 2);
   427	        // 64 angular, 2 axial: lateral=64*2*2=256, caps=(64-2)*2=124 → 380
   428	        assert_eq!(mesh_hi.triangle_count(), 64 * 2 * 2 + (64 - 2) * 2);
   429	    }
   430	
   431	    /// T15: Unsupported surface type → error.
   432	    #[test]
   433	    fn test_unsupported_surface_error() {
   434	        use crate::brep::topology::Solid as S;
   435	        use crate::geometry::curve::Curve;
   436	
   437	        let mut s = S::new(0);
   438	        let v0 = s.add_vertex(1, Point::new(0.0, 0.0, 0.0));
   439	        let v1 = s.add_vertex(2, Point::new(1.0, 0.0, 0.0));
   440	        let e0 = s.add_edge(
   441	            3,
   442	            [v0, v1],
   443	            Curve::Line {
   444	                origin: Point::origin(),
   445	                direction: Vec3::x(),
   446	            },
   447	            [0.0, 1.0],
   448	        );
   449	        let he0 = s.add_half_edge(4, v0, e0, true);
   450	        let lp = s.add_loop(5, vec![he0]);
   451	        s.add_face(
   452	            6,
   453	            Surface::Sphere {
   454	                center: Point::origin(),
   455	                radius: 1.0,
   456	            },
   457	            lp,
   458	            vec![],
   459	            true,
   460	        );
   461	        s.add_shell(7, vec![0], true);
   462	
   463	        let result = tessellate_solid(&s);
   464	        assert!(matches!(
   465	            result,
   466	            Err(TessellationError::UnsupportedSurface { kind: "sphere" })
   467	        ));
   468	    }
   469	
   470	    // Legacy tests preserved for backward compatibility
   471	    #[test]
   472	    fn test_tessellate_cuboid() {
   473	        let mut id_gen = IdGenerator::new(0);
   474	        let solid = make_cuboid(1.0, 1.0, 1.0, &mut id_gen);
   475	        let mesh = tessellate_solid(&solid).unwrap();
   476	
   477	        assert_eq!(mesh.triangle_count(), 12);
   478	        assert_eq!(mesh.positions.len(), 24);
   479	        assert_eq!(mesh.normals.len(), 24);
   480	        assert_eq!(mesh.indices.len(), 36);
   481	    }
   482	
   483	    #[test]
   484	    fn test_tessellation_deterministic() {
   485	        let mut id_gen1 = IdGenerator::new(0);
   486	        let mut id_gen2 = IdGenerator::new(0);
   487	
   488	        let solid1 = make_cuboid(2.0, 3.0, 4.0, &mut id_gen1);
   489	        let solid2 = make_cuboid(2.0, 3.0, 4.0, &mut id_gen2);
   490	
   491	        let mesh1 = tessellate_solid(&solid1).unwrap();
   492	        let mesh2 = tessellate_solid(&solid2).unwrap();
   493	
   494	        assert_eq!(mesh1.positions, mesh2.positions);
   495	        assert_eq!(mesh1.normals, mesh2.normals);
   496	        assert_eq!(mesh1.indices, mesh2.indices);
   497	    }
   498	}

exec
/bin/bash -lc "grep -R \"half_edges.len()\" -n crates/mycad-kernel/src | sed -n '1,120p'" in /home/bacon/mycad
 succeeded in 0ms:
crates/mycad-kernel/src/primitives/cylinder.rs:144:        assert_eq!(s1.half_edges.len(), s2.half_edges.len());
crates/mycad-kernel/src/primitives/cylinder.rs:176:        assert_eq!(s.half_edges.len(), 6, "6 half-edges (3 edges × 2 HE each)");
crates/mycad-kernel/src/primitives/cylinder.rs:227:            for i in 0..lp.half_edges.len() {
crates/mycad-kernel/src/primitives/cylinder.rs:229:                let he_next = &s.half_edges[lp.half_edges[(i + 1) % lp.half_edges.len()]];
crates/mycad-kernel/src/primitives/cylinder.rs:246:                    (i + 1) % lp.half_edges.len(),
crates/mycad-kernel/src/primitives/cuboid.rs:237:            solid.half_edges.len(),
crates/mycad-kernel/src/brep/topology.rs:137:        let idx = self.half_edges.len();

exec
/bin/bash -lc "grep -n \"cross_norm\\|AREA_EPS\\|degenerate\" -i features/12-cylinder/final-review.md | head -80" in /home/bacon/mycad
 succeeded in 0ms:
861:+    /// T14: Degenerate inputs — radius<=0, height<=0, non-finite → InvalidParameter.
863:+    fn test_cylinder_degenerate_inputs() {
963:+/// Minimum cross-product norm for a non-degenerate triangle.
964:+const AREA_EPS: f64 = 1e-14;
1237:+/// Push a triangle, but skip degenerate (zero-area) ones.
1244:+    let cross_norm = (u[1] * v[2] - u[2] * v[1]).powi(2)
1247:+    if cross_norm < AREA_EPS * AREA_EPS {
2623:/// Minimum cross-product norm for a non-degenerate triangle.
2624:const AREA_EPS: f64 = 1e-14;
2861:/// Push a triangle, but skip degenerate (zero-area) ones.
2868:    let cross_norm = (u[1] * v[2] - u[2] * v[1]).powi(2)
2871:    if cross_norm < AREA_EPS * AREA_EPS {
2873:    if cross_norm < AREA_EPS * AREA_EPS {
3155:    /// T14: Degenerate inputs — radius<=0, height<=0, non-finite → InvalidParameter.
3157:    fn test_cylinder_degenerate_inputs() {
3886:    12	/// Minimum cross-product norm for a non-degenerate triangle.
3887:    13	const AREA_EPS: f64 = 1e-14;
4124:   250	/// Push a triangle, but skip degenerate (zero-area) ones.
4131:   257	    let cross_norm = (u[1] * v[2] - u[2] * v[1]).powi(2)
4134:   260	    if cross_norm < AREA_EPS * AREA_EPS {
4139:   260	    if cross_norm < AREA_EPS * AREA_EPS {

codex
The cylinder path works for the new canonical primitive, but the new tessellation API still emits incorrect geometry for non-canonical cylindrical faces and can generate NaN meshes when segment counts are bypassed through the public options type. Those are correctness bugs in newly introduced code paths.

Full review comments:

- [P2] Restrict UV-grid tessellation to canonical full cylinders — /home/bacon/mycad/crates/mycad-kernel/src/tessellation/mod.rs:205-205
  This hard-codes the side patch to start at `u=0` and later spans a full `2π`, so any cylindrical face that is trimmed in U but has no `inner_loops` (for example a half-cylinder built via the public `Solid` API, or a full cylinder whose seam vertex is not at `u=0`) will be meshed as the wrong surface instead of returning `TrimmedFaceUnsupported`. Because every `Surface::Cylinder` is routed through this path, future/non-canonical cylinder faces will silently export triangles outside the B-rep boundary.

- [P2] Validate segment counts when consuming tessellation options — /home/bacon/mycad/crates/mycad-kernel/src/tessellation/mod.rs:214-215
  `TessellationOptions::new()` clamps invalid values, but callers can bypass that via the public fields or `Deserialize`. If `angular_segments == 0` or `axial_segments == 0`, these divisions produce `inf`, and the subsequent `0 * inf` in the UV loop yields `NaN` vertices while the planar caps are skipped for having too few sampled points. In other words, a perfectly valid cylinder can export an invalid mesh just by passing a deserialized or literal `TessellationOptions` value.
The cylinder path works for the new canonical primitive, but the new tessellation API still emits incorrect geometry for non-canonical cylindrical faces and can generate NaN meshes when segment counts are bypassed through the public options type. Those are correctness bugs in newly introduced code paths.

Full review comments:

- [P2] Restrict UV-grid tessellation to canonical full cylinders — /home/bacon/mycad/crates/mycad-kernel/src/tessellation/mod.rs:205-205
  This hard-codes the side patch to start at `u=0` and later spans a full `2π`, so any cylindrical face that is trimmed in U but has no `inner_loops` (for example a half-cylinder built via the public `Solid` API, or a full cylinder whose seam vertex is not at `u=0`) will be meshed as the wrong surface instead of returning `TrimmedFaceUnsupported`. Because every `Surface::Cylinder` is routed through this path, future/non-canonical cylinder faces will silently export triangles outside the B-rep boundary.

- [P2] Validate segment counts when consuming tessellation options — /home/bacon/mycad/crates/mycad-kernel/src/tessellation/mod.rs:214-215
  `TessellationOptions::new()` clamps invalid values, but callers can bypass that via the public fields or `Deserialize`. If `angular_segments == 0` or `axial_segments == 0`, these divisions produce `inf`, and the subsequent `0 * inf` in the UV loop yields `NaN` vertices while the planar caps are skipped for having too few sampled points. In other words, a perfectly valid cylinder can export an invalid mesh just by passing a deserialized or literal `TessellationOptions` value.
