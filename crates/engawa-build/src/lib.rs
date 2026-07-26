pub mod feature_crud;

pub use feature_crud::{FeatureCrud, FeatureCrudError};

use engawa_format::{Feature, PlaneRef, RefPlane, SketchPlane};
use engawa_kernel::booleans::boolean;
use engawa_kernel::brep::topology::{IdGenerator, Solid};
use engawa_kernel::error::KernelError;
use engawa_kernel::geometry::surface::Surface;
use engawa_kernel::geometry::transform::{euler_to_matrix, rotate_vec};
use engawa_kernel::geometry::Plane;
use engawa_kernel::geometry::Point;
use engawa_kernel::geometry::Vec3;
use engawa_kernel::primitives::{make_cuboid, make_cylinder, make_extrusion, make_sphere};
use engawa_kernel::BooleanOp;
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

const IDENTITY3: [[f64; 3]; 3] = [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]];

/// 3×3 行列積 (純関数、決定的)
fn matrix_mul3(a: [[f64; 3]; 3], b: [[f64; 3]; 3]) -> [[f64; 3]; 3] {
    [
        [
            a[0][0] * b[0][0] + a[0][1] * b[1][0] + a[0][2] * b[2][0],
            a[0][0] * b[0][1] + a[0][1] * b[1][1] + a[0][2] * b[2][1],
            a[0][0] * b[0][2] + a[0][1] * b[1][2] + a[0][2] * b[2][2],
        ],
        [
            a[1][0] * b[0][0] + a[1][1] * b[1][0] + a[1][2] * b[2][0],
            a[1][0] * b[0][1] + a[1][1] * b[1][1] + a[1][2] * b[2][1],
            a[1][0] * b[0][2] + a[1][1] * b[1][2] + a[1][2] * b[2][2],
        ],
        [
            a[2][0] * b[0][0] + a[2][1] * b[1][0] + a[2][2] * b[2][0],
            a[2][0] * b[0][1] + a[2][1] * b[1][1] + a[2][2] * b[2][1],
            a[2][0] * b[0][2] + a[2][1] * b[1][2] + a[2][2] * b[2][2],
        ],
    ]
}

#[derive(Debug, Clone)]
pub struct Body {
    pub feature_id: String,
    pub solid: Solid,
}

#[derive(Debug, Clone, Default)]
pub struct BuiltBodies {
    bodies: Vec<Body>,
    index: HashMap<String, usize>,
    consumed: HashSet<String>,
}

impl BuiltBodies {
    pub fn get(&self, feature_id: &str) -> Option<&Body> {
        if self.consumed.contains(feature_id) {
            return None;
        }
        let &idx = self.index.get(feature_id)?;
        Some(&self.bodies[idx])
    }

    pub fn all(&self) -> &[Body] {
        &self.bodies
    }

    pub fn live(&self) -> impl Iterator<Item = &Body> {
        self.bodies
            .iter()
            .filter(|b| !self.consumed.contains(&b.feature_id))
    }

    pub fn len(&self) -> usize {
        self.bodies.len()
    }

    pub fn is_empty(&self) -> bool {
        self.bodies.is_empty()
    }

    fn register(&mut self, feature_id: String, solid: Solid) {
        let idx = self.bodies.len();
        self.index.insert(feature_id.clone(), idx);
        self.bodies.push(Body { feature_id, solid });
    }

    fn consume(&mut self, feature_id: &str) {
        self.consumed.insert(feature_id.into());
        self.index.remove(feature_id);
    }
}

pub fn build_bodies_from_features(
    features: &[Feature],
    ref_planes: &[RefPlane],
    gen: &mut IdGenerator,
) -> Result<BuiltBodies, KernelError> {
    // Filter out suppressed features. All-suppressed is a valid state (returns empty bodies).
    let active_features: Vec<_> = features
        .iter()
        .filter(|f| !f.is_suppressed())
        .cloned()
        .collect();

    // Only error if there are no features at all (caller error).
    if features.is_empty() {
        return Err(KernelError::EmptyFeatureList);
    }
    if active_features.is_empty() {
        // All features are suppressed — return empty bodies.
        return Ok(BuiltBodies::default());
    }

    /// Sketch entry with optional ref_plane reference.
    struct SketchEntry<'a> {
        plane_ref: Option<PlaneRef>,
        plane: SketchPlane,
        offset: f64,
        profile: &'a [engawa_format::SketchElement],
    }

    let mut sketches: HashMap<&str, SketchEntry> = HashMap::new();
    let mut built_sketch_profiles: HashMap<String, Vec<engawa_format::SketchElement>> =
        HashMap::new();
    let mut built = BuiltBodies::default();
    let mut seen_ids: HashMap<&str, ()> = HashMap::new();

    /// Resolve a Plane from a SketchEntry, using plane_ref if present.
    fn resolve_plane(
        entry: &SketchEntry,
        ref_planes: &[RefPlane],
        built: &BuiltBodies,
    ) -> Result<Plane, KernelError> {
        match &entry.plane_ref {
            Some(PlaneRef::RefPlane(ref_id)) => {
                // Legacy ref_plane path (unchanged behavior)
                let rp = ref_planes
                    .iter()
                    .find(|p| &p.id == ref_id)
                    .ok_or_else(|| KernelError::UnknownRefPlane { id: ref_id.clone() })?;
                if !rp.offset.is_finite() {
                    return Err(KernelError::InvalidRefPlaneOffset { id: rp.id.clone() });
                }
                let base = sketch_plane_to_plane(rp.plane);
                Ok(if rp.offset != 0.0 {
                    base.translate(base.normal * rp.offset)
                } else {
                    base
                })
            }
            Some(PlaneRef::Entity(entity_ref)) => {
                // New Face EntityRef path
                for body in built.live() {
                    if let Some(face_idx) = body.solid.find_face_by_entity_ref(entity_ref) {
                        let face = &body.solid.faces[face_idx];
                        return surface_to_plane(&face.surface).ok_or_else(|| {
                            KernelError::FaceNotPlanar {
                                canonical_name: entity_ref.canonical_name(),
                            }
                        });
                    }
                }
                Err(KernelError::FaceEntityRefNotFound {
                    canonical_name: entity_ref.canonical_name(),
                })
            }
            None => {
                // Default plane/offset path (unchanged behavior)
                let base = sketch_plane_to_plane(entry.plane);
                Ok(if entry.offset != 0.0 {
                    base.translate(base.normal * entry.offset)
                } else {
                    base
                })
            }
        }
    }

    fn sketch_plane_to_plane(sp: SketchPlane) -> Plane {
        match sp {
            SketchPlane::Xy => Plane::xy(),
            SketchPlane::Xz => Plane::xz(),
            SketchPlane::Yz => Plane::yz(),
        }
    }

    /// Extract a Plane from a Surface if it is planar.
    fn surface_to_plane(surface: &Surface) -> Option<Plane> {
        match surface {
            Surface::Plane {
                origin,
                normal,
                u_axis,
                v_axis,
            } => Some(Plane {
                origin: *origin,
                normal: *normal,
                u_axis: *u_axis,
                v_axis: *v_axis,
            }),
            _ => None,
        }
    }

    for feature in features {
        if feature.is_suppressed() {
            continue;
        }
        let id = feature.id();
        if !seen_ids.contains_key(id) {
            seen_ids.insert(id, ());
        } else {
            return Err(KernelError::DuplicateFeatureId { id: id.to_string() });
        }

        validate_feature_id(id)?;
        match feature {
            Feature::CreateSketch {
                id: _,
                plane,
                offset,
                variables: _,
                profile,
                plane_ref,
                suppressed: _,
            } => {
                validate_sketch_element_ids(profile)?;
                validate_profile_closed(profile)?;
                if sketches
                    .insert(
                        id,
                        SketchEntry {
                            plane_ref: plane_ref.clone(),
                            plane: *plane,
                            offset: *offset,
                            profile: profile.as_slice(),
                        },
                    )
                    .is_some()
                {
                    return Err(KernelError::DuplicateFeatureId { id: id.to_string() });
                }
            }
            Feature::Extrude {
                id: _,
                sketch,
                depth,
                fuse_target,
                suppressed: _,
            } => {
                let entry =
                    sketches
                        .get(sketch.as_str())
                        .ok_or_else(|| KernelError::SketchNotFound {
                            sketch: sketch.clone(),
                        })?;

                let plane = resolve_plane(entry, ref_planes, &built)?;

                // Effective profile: use offset-applied profile if available (ADR-017 §3 dispatch order).
                let effective_profile: &[engawa_format::SketchElement] = built_sketch_profiles
                    .get(sketch.as_str())
                    .map(|v| v.as_slice())
                    .unwrap_or(entry.profile);

                // Reject multi-element profiles that include a closed primitive (Phase 10 scope).
                // Multi-contour profiles are deferred to Phase 11+ (#288).
                validate_sketch_profile_contours(effective_profile)?;

                // Tessellate each sketch element into polyline points.
                const BASE_SEGMENTS: usize = 32;
                let profile_uv: Vec<(f64, f64)> = effective_profile
                    .iter()
                    .map(|elem| {
                        engawa_kernel::tessellation::sketch::tessellate_sketch_element(
                            elem,
                            BASE_SEGMENTS,
                        )
                    })
                    .collect::<Result<Vec<_>, _>>()?
                    .into_iter()
                    .flat_map(|poly| poly.into_iter().map(|p| (p[0], p[1])))
                    .collect();
                let extruded = make_extrusion(&plane, &profile_uv, *depth, gen)?;

                if let Some(target_id) = fuse_target {
                    let t_solid =
                        built
                            .get(target_id)
                            .ok_or_else(|| KernelError::BodyNotFound {
                                id: target_id.clone(),
                            })?;
                    let result = boolean(&t_solid.solid, &extruded, BooleanOp::Fuse, gen)?;
                    built.consume(target_id);
                    built.register(id.to_string(), result);
                } else {
                    built.register(id.to_string(), extruded);
                }
            }
            Feature::ExtrudeCut {
                id: _,
                sketch,
                depth,
                target,
                suppressed: _,
            } => {
                if *depth <= 0.0 {
                    return Err(KernelError::InvalidParameter { kind: "depth" });
                }

                let entry =
                    sketches
                        .get(sketch.as_str())
                        .ok_or_else(|| KernelError::SketchNotFound {
                            sketch: sketch.clone(),
                        })?;

                // ExtrudeCut は元来 CreateSketch.offset を無視する仕様バグがあった (#161 で別途修正予定)。
                // 本 Issue のスコープを守るため、plane_ref が None の経路では従来挙動を温存する。
                // plane_ref が Some なら ref_planes または Face EntityRef から解決する。
                let plane = match &entry.plane_ref {
                    Some(PlaneRef::RefPlane(ref_id)) => {
                        // RefPlane 経路
                        let rp = ref_planes
                            .iter()
                            .find(|p| &p.id == ref_id)
                            .ok_or_else(|| KernelError::UnknownRefPlane { id: ref_id.clone() })?;
                        if !rp.offset.is_finite() {
                            return Err(KernelError::InvalidRefPlaneOffset { id: rp.id.clone() });
                        }
                        let base = sketch_plane_to_plane(rp.plane);
                        if rp.offset != 0.0 {
                            base.translate(base.normal * rp.offset)
                        } else {
                            base
                        }
                    }
                    Some(PlaneRef::Entity(entity_ref)) => {
                        // Face EntityRef 経路。find_face_by_entity_ref が見つけた Face で
                        // surface_to_plane が None なら非平面 → FaceNotPlanar とし、
                        // どの Solid からも Face が見つからなかった場合のみ FaceEntityRefNotFound。
                        // (Codex F01 #215: 非平面 face を未検出扱いにせず分類する)
                        let mut face_found = false;
                        let mut found_plane = None;
                        for body in built.live() {
                            if let Some(face_idx) = body.solid.find_face_by_entity_ref(entity_ref) {
                                let face = &body.solid.faces[face_idx];
                                face_found = true;
                                found_plane = surface_to_plane(&face.surface);
                                break;
                            }
                        }
                        if let Some(p) = found_plane {
                            p
                        } else if face_found {
                            return Err(KernelError::FaceNotPlanar {
                                canonical_name: entity_ref.canonical_name(),
                            });
                        } else {
                            return Err(KernelError::FaceEntityRefNotFound {
                                canonical_name: entity_ref.canonical_name(),
                            });
                        }
                    }
                    None => {
                        // 旧経路: plane だけ使い、offset は **無視** (元仕様バグの温存、#161 で対応)
                        sketch_plane_to_plane(entry.plane)
                    }
                };

                // Effective profile: use offset-applied profile if available (ADR-017 §3 dispatch order).
                let effective_profile: &[engawa_format::SketchElement] = built_sketch_profiles
                    .get(sketch.as_str())
                    .map(|v| v.as_slice())
                    .unwrap_or(entry.profile);

                // Reject multi-element profiles that include a closed primitive (Phase 10 scope).
                // Multi-contour profiles are deferred to Phase 11+ (#288).
                validate_sketch_profile_contours(effective_profile)?;

                const BASE_SEGMENTS: usize = 32;
                let profile_uv: Vec<(f64, f64)> = effective_profile
                    .iter()
                    .map(|elem| {
                        engawa_kernel::tessellation::sketch::tessellate_sketch_element(
                            elem,
                            BASE_SEGMENTS,
                        )
                    })
                    .collect::<Result<Vec<_>, _>>()?
                    .into_iter()
                    .flat_map(|poly| poly.into_iter().map(|p| (p[0], p[1])))
                    .collect();
                let tool = make_extrusion(&plane, &profile_uv, *depth, gen)?;

                let t_solid = built
                    .get(target)
                    .ok_or_else(|| KernelError::BodyNotFound { id: target.clone() })?;
                let result = boolean(&t_solid.solid, &tool, BooleanOp::Cut, gen)?;
                built.consume(target);
                built.register(id.to_string(), result);
            }
            Feature::CreateBox {
                id: _,
                width,
                height,
                depth,
                suppressed: _,
            } => {
                let solid = make_cuboid(*width, *height, *depth, id, gen)?;
                built.register(id.to_string(), solid);
            }
            Feature::CreateCylinder {
                id: _,
                radius,
                height,
                origin,
                suppressed: _,
            } => {
                let solid = make_cylinder(
                    *radius,
                    *height,
                    Point::new(origin[0], origin[1], origin[2]),
                    gen,
                )?;
                built.register(id.to_string(), solid);
            }
            Feature::CreateSphere {
                id: _,
                radius,
                center,
                suppressed: _,
            } => {
                let solid = make_sphere(*radius, Point::new(center[0], center[1], center[2]), gen)?;
                built.register(id.to_string(), solid);
            }
            Feature::Cut {
                id: _,
                target,
                tool,
                suppressed: _,
            } => {
                let t_solid = built
                    .get(target)
                    .ok_or_else(|| KernelError::BodyNotFound { id: target.clone() })?;
                let u_solid = built
                    .get(tool)
                    .ok_or_else(|| KernelError::BodyNotFound { id: tool.clone() })?;
                let result = boolean(&t_solid.solid, &u_solid.solid, BooleanOp::Cut, gen)?;
                built.consume(target);
                built.consume(tool);
                built.register(id.to_string(), result);
            }
            Feature::Fuse {
                id: _,
                target,
                tool,
                suppressed: _,
            } => {
                let t_solid = built
                    .get(target)
                    .ok_or_else(|| KernelError::BodyNotFound { id: target.clone() })?;
                let u_solid = built
                    .get(tool)
                    .ok_or_else(|| KernelError::BodyNotFound { id: tool.clone() })?;
                let result = boolean(&t_solid.solid, &u_solid.solid, BooleanOp::Fuse, gen)?;
                built.consume(target);
                built.consume(tool);
                built.register(id.to_string(), result);
            }
            Feature::Intersect {
                id: _,
                target,
                tool,
                suppressed: _,
            } => {
                let t_solid = built
                    .get(target)
                    .ok_or_else(|| KernelError::BodyNotFound { id: target.clone() })?;
                let u_solid = built
                    .get(tool)
                    .ok_or_else(|| KernelError::BodyNotFound { id: tool.clone() })?;
                let result = boolean(&t_solid.solid, &u_solid.solid, BooleanOp::Intersect, gen)?;
                built.consume(target);
                built.consume(tool);
                built.register(id.to_string(), result);
            }
            Feature::SketchOffset {
                id: _,
                sketch,
                selection,
                distance,
                suppressed: _,
            } => {
                let entry =
                    sketches
                        .get(sketch.as_str())
                        .ok_or_else(|| KernelError::SketchNotFound {
                            sketch: sketch.clone(),
                        })?;

                // Source profile: use previously accumulated offsets, or fall back to CreateSketch.profile
                let source: Vec<engawa_format::SketchElement> = built_sketch_profiles
                    .get(sketch.as_str())
                    .cloned()
                    .unwrap_or_else(|| entry.profile.to_vec());

                // Build-level contract: Circle single-element only (Phase 10 scope defense)
                let out = engawa_kernel::geometry::sketch_offset::apply_sketch_offset_build(
                    &source, selection, *distance,
                )?;
                built_sketch_profiles.insert(sketch.clone(), out);
                // id remains in feature history only (no body generation)
            }
            Feature::SketchFillet {
                id: _,
                sketch,
                elem1_id,
                elem2_id,
                radius,
                suppressed: _,
            } => {
                let entry =
                    sketches
                        .get(sketch.as_str())
                        .ok_or_else(|| KernelError::SketchNotFound {
                            sketch: sketch.clone(),
                        })?;

                // Source profile: previously accumulated edits, else CreateSketch.profile.
                let source: Vec<engawa_format::SketchElement> = built_sketch_profiles
                    .get(sketch.as_str())
                    .cloned()
                    .unwrap_or_else(|| entry.profile.to_vec());

                let out = engawa_kernel::geometry::sketch_fillet::apply_sketch_fillet_build(
                    &source, elem1_id, elem2_id, *radius,
                )?;
                built_sketch_profiles.insert(sketch.clone(), out);
            }
            Feature::SketchChamfer {
                id: _,
                sketch,
                elem1_id,
                elem2_id,
                length,
                suppressed: _,
            } => {
                let entry =
                    sketches
                        .get(sketch.as_str())
                        .ok_or_else(|| KernelError::SketchNotFound {
                            sketch: sketch.clone(),
                        })?;

                // Source profile: previously accumulated edits, else CreateSketch.profile.
                let source: Vec<engawa_format::SketchElement> = built_sketch_profiles
                    .get(sketch.as_str())
                    .cloned()
                    .unwrap_or_else(|| entry.profile.to_vec());

                let out = engawa_kernel::geometry::sketch_chamfer::apply_sketch_chamfer_build(
                    &source, elem1_id, elem2_id, *length,
                )?;
                built_sketch_profiles.insert(sketch.clone(), out);
            }
        }
    }

    // Only error if there are active body producers but produced nothing.
    // Sketch-only or all-suppressed components are valid (empty bodies).
    let active_body_producers = features
        .iter()
        .filter(|f| !f.is_suppressed() && is_body_producer(f))
        .count();
    if built.is_empty() && active_body_producers > 0 {
        return Err(KernelError::EmptyFeatureList);
    }

    Ok(built)
}

/// Returns true if the feature can produce a Body (Solid).
/// CreateSketch is NOT a body producer; it only defines a profile.
fn is_body_producer(f: &Feature) -> bool {
    matches!(
        f,
        Feature::CreateBox { .. }
            | Feature::CreateCylinder { .. }
            | Feature::CreateSphere { .. }
            | Feature::Extrude { .. }
            | Feature::ExtrudeCut { .. }
            | Feature::Cut { .. }
            | Feature::Fuse { .. }
            | Feature::Intersect { .. }
    )
}

fn validate_feature_id(id: &str) -> Result<(), KernelError> {
    if id.is_empty() {
        return Err(KernelError::InvalidParameter { kind: "feature_id" });
    }
    if !id
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
    {
        return Err(KernelError::InvalidParameter { kind: "feature_id" });
    }
    Ok(())
}

fn validate_sketch_element_ids(
    elements: &[engawa_format::SketchElement],
) -> Result<(), KernelError> {
    let mut seen = HashMap::new();
    for elem in elements {
        let id = match elem {
            engawa_format::SketchElement::Line { id, .. }
            | engawa_format::SketchElement::Circle { id, .. }
            | engawa_format::SketchElement::Arc { id, .. }
            | engawa_format::SketchElement::Ellipse { id, .. }
            | engawa_format::SketchElement::Conic { id, .. } => id,
        };
        if id.is_empty() {
            return Err(KernelError::InvalidParameter {
                kind: "sketch_element_id",
            });
        }
        if !id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
        {
            return Err(KernelError::InvalidParameter {
                kind: "sketch_element_id",
            });
        }
        if seen.insert(id, ()).is_some() {
            return Err(KernelError::InvalidParameter {
                kind: "sketch_element_id",
            });
        }
    }
    Ok(())
}

/// Validate that profile forms a closed loop.
///
/// For Line elements, checks that `to[i] == from[(i+1)%n]` within tolerance.
/// For Circle/Arc, tessellation is required to verify closure — this is
/// deferred to runtime (tessellate_sketch_element produces closed polylines).
/// For Conic hyperbola (discriminant > 0), rejects as open curve (F05).
fn validate_profile_closed(elements: &[engawa_format::SketchElement]) -> Result<(), KernelError> {
    use engawa_kernel::tessellation::sketch_element_is_open;
    use engawa_kernel::LENGTH_TOLERANCE;
    if elements.is_empty() {
        return Err(KernelError::InvalidParameter { kind: "profile" });
    }
    // F05: reject hyperbola conic (open curve) in profile
    for elem in elements {
        if sketch_element_is_open(elem) {
            return Err(KernelError::InvalidParameter { kind: "profile" });
        }
    }
    // Only Line elements have explicit to/from; Circle/Arc tessellate to closed polylines
    let line_count = elements
        .iter()
        .filter(|e| matches!(e, engawa_format::SketchElement::Line { .. }))
        .count();
    if line_count == elements.len() {
        // All Lines: verify explicit closure
        for i in 0..elements.len() {
            let j = (i + 1) % elements.len();
            let (from_j, to_i) = match (&elements[j], &elements[i]) {
                (
                    engawa_format::SketchElement::Line { from, .. },
                    engawa_format::SketchElement::Line { to, .. },
                ) => (from, to),
                _ => unreachable!(),
            };
            let du = from_j[0] - to_i[0];
            let dv = from_j[1] - to_i[1];
            if (du * du + dv * dv).sqrt() > LENGTH_TOLERANCE {
                return Err(KernelError::InvalidParameter { kind: "profile" });
            }
        }
    }
    Ok(())
}

/// #288 Phase 10: Reject multi-element profiles that include a closed primitive.
///
/// Multi-contour profiles are deferred to Phase 11+. This guard prevents the
/// current single-contour implementation from incorrectly flattening multiple
/// closed primitives (e.g., `[Circle, Circle]`) into a single broken polyline.
///
/// Closed primitives: Circle, Ellipse, Conic (conservatively treated as closed),
/// and Arc with sweep ≈ 2π (full circle).
/// Open primitives: Line, Arc with sweep < 2π.
fn is_closed_primitive(elem: &engawa_format::SketchElement) -> bool {
    use engawa_format::SketchElement;
    use engawa_kernel::ANGLE_TOLERANCE;
    use std::f64::consts::TAU;
    match elem {
        SketchElement::Circle { .. }
        | SketchElement::Ellipse { .. }
        | SketchElement::Conic { .. } => true,
        SketchElement::Arc {
            start_angle,
            end_angle,
            ..
        } => {
            let sweep = (end_angle - start_angle).abs();
            let n = (sweep / TAU).round();
            n >= 1.0 && (sweep - n * TAU).abs() <= ANGLE_TOLERANCE
        }
        SketchElement::Line { .. } => false,
    }
}

fn validate_sketch_profile_contours(
    profile: &[engawa_format::SketchElement],
) -> Result<(), KernelError> {
    if profile.len() > 1 && profile.iter().any(is_closed_primitive) {
        return Err(KernelError::InvalidParameter { kind: "profile" });
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Assembly build (Issue #73)
// ---------------------------------------------------------------------------

use engawa_format::component::{Component, ComponentRef};
use engawa_format::Document;

const MAX_REFERENCE_DEPTH: usize = 16;

/// Build an assembly by traversing the Component tree, resolving references,
/// and aggregating all live Bodies.
///
/// `base_dir` is the parent directory of the `.engawa` file being built.
/// Each Component's `transform` (position + rotation) is accumulated along the tree path
/// and applied to the resulting Bodies.
///
/// Transform composition:
///   total_rotation = accumulated_rotation * local_rotation
///   total_offset   = accumulated_offset + accumulated_rotation * local_offset
///   Apply order: rotate (around origin) → translate
pub fn build_assembly(
    doc: &Document,
    base_dir: &Path,
    gen: &mut IdGenerator,
) -> Result<Vec<Body>, KernelError> {
    let mut bodies: Vec<Body> = Vec::new();
    let mut visiting: Vec<PathBuf> = Vec::new();
    build_component_tree(
        &doc.root_component,
        base_dir,
        &mut visiting,
        0,
        Vec3::zeros(),
        IDENTITY3,
        gen,
        &mut bodies,
    )?;
    Ok(bodies)
}

#[allow(clippy::too_many_arguments)]
fn build_component_tree(
    component: &Component,
    base_dir: &Path,
    visiting: &mut Vec<PathBuf>,
    depth: usize,
    accumulated_offset: Vec3,
    accumulated_rotation: [[f64; 3]; 3],
    gen: &mut IdGenerator,
    out: &mut Vec<Body>,
) -> Result<(), KernelError> {
    // --- position ---
    let p = &component.transform.position;
    if p.iter().any(|v| !v.is_finite()) {
        return Err(KernelError::InvalidParameter {
            kind: "transform.position",
        });
    }
    let local_offset = Vec3::new(p[0], p[1], p[2]);

    // --- rotation (deg → rad → matrix) ---
    let r = &component.transform.rotation;
    if r.iter().any(|v| !v.is_finite()) {
        return Err(KernelError::InvalidParameter {
            kind: "transform.rotation",
        });
    }
    let local_rotation = euler_to_matrix(r[0].to_radians(), r[1].to_radians(), r[2].to_radians());

    // 累積:
    //   p_world(p_local) = accumulated_offset + accumulated_rotation * (local_offset + local_rotation * p_local)
    // を分配すると:
    //   total_offset   = accumulated_offset + accumulated_rotation * local_offset
    //   total_rotation = accumulated_rotation * local_rotation
    let rotated_local_offset = rotate_vec(local_offset, accumulated_rotation);
    let total_offset = accumulated_offset + rotated_local_offset;
    let total_rotation = matrix_mul3(accumulated_rotation, local_rotation);

    // 1. Build this component's own features and apply total transform.
    //
    // ADR-014: 各 Component は独立した coordinate frame を持ち、空の `ref_planes` は親を継承せず
    // canonical three (Front/Top/Right) にフォールバックする。child の `plane_ref: "Front"` は親が
    // custom-only ref_planes であっても常に解決可能。custom datum を child で使うには child 自身に
    // 宣言する。
    let canonical = RefPlane::default_canonical_three();
    let effective_ref_planes: &[RefPlane] = if !component.ref_planes.is_empty() {
        component.ref_planes.as_slice()
    } else {
        canonical.as_slice()
    };

    if !component.features.is_empty() {
        let built = build_bodies_from_features(&component.features, effective_ref_planes, gen)?;
        for mut body in built.live().cloned() {
            // 順序: rotate (around origin) → translate
            if total_rotation != IDENTITY3 {
                body.solid.rotate(total_rotation, Point::origin());
            }
            if total_offset != Vec3::zeros() {
                body.solid.translate(total_offset);
            }
            out.push(body);
        }
    }

    // 2. Resolve reference (if any) — propagates total_offset/total_rotation into the referenced tree
    if let Some(reference) = &component.reference {
        if depth >= MAX_REFERENCE_DEPTH {
            return Err(KernelError::MaxDepthExceeded {
                max: MAX_REFERENCE_DEPTH,
                path: component.name.clone(),
            });
        }
        let (resolved_path, child_base_dir) = resolve_reference(reference, base_dir)?;
        let canonical =
            std::fs::canonicalize(&resolved_path).unwrap_or_else(|_| resolved_path.clone());
        if visiting.contains(&canonical) {
            return Err(KernelError::CircularReference {
                path: resolved_path.display().to_string(),
            });
        }
        let ref_doc =
            Document::from_path(&resolved_path).map_err(|e| KernelError::ReferenceResolution {
                path: resolved_path.display().to_string(),
                reason: e.to_string(),
            })?;
        visiting.push(canonical);
        build_component_tree(
            &ref_doc.root_component,
            &child_base_dir,
            visiting,
            depth + 1,
            total_offset,
            total_rotation,
            gen,
            out,
        )?;
        visiting.pop();
    }

    // 3. Recurse into children, propagating total_offset and total_rotation.
    //
    // 各 child は独立した coordinate frame (ADR-014) を持ち、自身の effective_ref_planes を導出する。
    for child in &component.children {
        build_component_tree(
            child,
            base_dir,
            visiting,
            depth,
            total_offset,
            total_rotation,
            gen,
            out,
        )?;
    }
    Ok(())
}

fn resolve_reference(
    reference: &ComponentRef,
    base_dir: &Path,
) -> Result<(PathBuf, PathBuf), KernelError> {
    let path = match reference {
        ComponentRef::StdLib(rel) => {
            let root = resolve_stdlib_root().ok_or_else(|| KernelError::ReferenceResolution {
                path: format!("stdlib://{rel}"),
                reason: "stdlib root not found (set ENGAWA_STDLIB_PATH or provide stdlib/)".into(),
            })?;
            root.join(format!("{rel}.engawa"))
        }
        ComponentRef::File(rel) => base_dir.join(rel),
    };
    let parent = path
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_else(|| base_dir.to_path_buf());
    Ok((path, parent))
}

/// Determine stdlib root: env `ENGAWA_STDLIB_PATH` (non-empty) > repo-local `stdlib/`.
fn resolve_stdlib_root() -> Option<PathBuf> {
    if let Ok(p) = std::env::var("ENGAWA_STDLIB_PATH") {
        if !p.trim().is_empty() {
            return Some(PathBuf::from(p));
        }
    }
    let repo_stdlib = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join("stdlib");
    if repo_stdlib.is_dir() {
        Some(repo_stdlib)
    } else {
        None
    }
}
