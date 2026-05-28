use mycad_format::Feature;
use mycad_kernel::brep::topology::{IdGenerator, Solid};
use mycad_kernel::error::KernelError;
use mycad_kernel::geometry::Plane;
use mycad_kernel::primitives::{make_cuboid, make_cylinder, make_extrusion, make_sphere};
use std::collections::HashMap;

#[derive(Debug, Clone)]
pub struct Body {
    pub feature_id: String,
    pub solid: Solid,
}

#[derive(Debug, Clone, Default)]
pub struct BuiltBodies {
    bodies: Vec<Body>,
    index: HashMap<String, usize>,
}

impl BuiltBodies {
    pub fn get(&self, feature_id: &str) -> Option<&Body> {
        let &idx = self.index.get(feature_id)?;
        Some(&self.bodies[idx])
    }

    pub fn all(&self) -> &[Body] {
        &self.bodies
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
}

pub fn build_bodies_from_features(
    features: &[Feature],
    gen: &mut IdGenerator,
) -> Result<BuiltBodies, KernelError> {
    if features.is_empty() {
        return Err(KernelError::EmptyFeatureList);
    }

    let mut sketches: HashMap<&str, (&mycad_format::SketchPlane, &[mycad_format::SketchSegment])> =
        HashMap::new();
    let mut built = BuiltBodies::default();
    let mut seen_ids: HashMap<&str, ()> = HashMap::new();

    for feature in features {
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
                profile,
            } => {
                validate_sketch_segment_ids(profile)?;
                validate_profile_closed(profile)?;
                if sketches.insert(id, (plane, profile.as_slice())).is_some() {
                    return Err(KernelError::DuplicateFeatureId { id: id.to_string() });
                }
            }
            Feature::Extrude {
                id: _,
                sketch,
                depth,
            } => {
                let (sketch_plane, segments) =
                    sketches
                        .get(sketch.as_str())
                        .ok_or_else(|| KernelError::SketchNotFound {
                            sketch: sketch.clone(),
                        })?;

                let plane = match sketch_plane {
                    mycad_format::SketchPlane::Xy => Plane::xy(),
                    mycad_format::SketchPlane::Xz => Plane::xz(),
                    mycad_format::SketchPlane::Yz => Plane::yz(),
                };

                let profile_uv: Vec<(f64, f64)> =
                    segments.iter().map(|s| (s.from[0], s.from[1])).collect();
                let solid = make_extrusion(&plane, &profile_uv, *depth, gen)?;
                built.register(id.to_string(), solid);
            }
            Feature::CreateBox {
                id: _,
                width,
                height,
                depth,
            } => {
                let solid = make_cuboid(*width, *height, *depth, gen)?;
                built.register(id.to_string(), solid);
            }
            Feature::CreateCylinder {
                id: _,
                radius,
                height,
            } => {
                let solid = make_cylinder(*radius, *height, gen)?;
                built.register(id.to_string(), solid);
            }
            Feature::CreateSphere { id: _, radius } => {
                let solid = make_sphere(*radius, gen)?;
                built.register(id.to_string(), solid);
            }
            Feature::Cut {
                id: _,
                target,
                tool,
            } => {
                if built.get(target).is_none() {
                    return Err(KernelError::BodyNotFound { id: target.clone() });
                }
                if built.get(tool).is_none() {
                    return Err(KernelError::BodyNotFound { id: tool.clone() });
                }
                return Err(KernelError::UnsupportedFeature { kind: "cut" });
            }
            Feature::Fuse {
                id: _,
                target,
                tool,
            } => {
                if built.get(target).is_none() {
                    return Err(KernelError::BodyNotFound { id: target.clone() });
                }
                if built.get(tool).is_none() {
                    return Err(KernelError::BodyNotFound { id: tool.clone() });
                }
                return Err(KernelError::UnsupportedFeature { kind: "fuse" });
            }
            Feature::Intersect {
                id: _,
                target,
                tool,
            } => {
                if built.get(target).is_none() {
                    return Err(KernelError::BodyNotFound { id: target.clone() });
                }
                if built.get(tool).is_none() {
                    return Err(KernelError::BodyNotFound { id: tool.clone() });
                }
                return Err(KernelError::UnsupportedFeature { kind: "intersect" });
            }
        }
    }

    if built.is_empty() {
        return Err(KernelError::EmptyFeatureList);
    }

    Ok(built)
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

fn validate_sketch_segment_ids(
    segments: &[mycad_format::SketchSegment],
) -> Result<(), KernelError> {
    let mut seen = HashMap::new();
    for seg in segments {
        if seg.id.is_empty() {
            return Err(KernelError::InvalidParameter {
                kind: "sketch_segment_id",
            });
        }
        if !seg
            .id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
        {
            return Err(KernelError::InvalidParameter {
                kind: "sketch_segment_id",
            });
        }
        if seen.insert(&seg.id, ()).is_some() {
            return Err(KernelError::InvalidParameter {
                kind: "sketch_segment_id",
            });
        }
    }
    Ok(())
}

fn validate_profile_closed(segments: &[mycad_format::SketchSegment]) -> Result<(), KernelError> {
    use mycad_kernel::LENGTH_TOLERANCE;
    if segments.is_empty() {
        return Err(KernelError::InvalidParameter { kind: "profile" });
    }
    for i in 0..segments.len() {
        let j = (i + 1) % segments.len();
        let du = segments[j].from[0] - segments[i].to[0];
        let dv = segments[j].from[1] - segments[i].to[1];
        if (du * du + dv * dv).sqrt() > LENGTH_TOLERANCE {
            return Err(KernelError::InvalidParameter { kind: "profile" });
        }
    }
    Ok(())
}
