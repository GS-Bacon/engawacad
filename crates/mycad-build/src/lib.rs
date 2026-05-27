use mycad_format::{Feature, SketchPlane};
use mycad_kernel::brep::topology::{IdGenerator, Solid};
use mycad_kernel::error::KernelError;
use mycad_kernel::geometry::Plane;
use mycad_kernel::primitives::{make_cuboid, make_cylinder, make_extrusion, make_sphere};
use std::collections::HashMap;

pub fn build_solid_from_features(
    features: &[Feature],
    gen: &mut IdGenerator,
) -> Result<Solid, KernelError> {
    if features.is_empty() {
        return Err(KernelError::EmptyFeatureList);
    }

    let mut sketches: HashMap<&str, (&SketchPlane, &[mycad_format::SketchSegment])> =
        HashMap::new();
    let mut solid: Option<Solid> = None;
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
                if solid.is_some() {
                    return Err(KernelError::MultipleFeatures {
                        count: count_solid_features(features),
                    });
                }
                let (sketch_plane, segments) =
                    sketches
                        .get(sketch.as_str())
                        .ok_or_else(|| KernelError::SketchNotFound {
                            sketch: sketch.clone(),
                        })?;

                let plane = match sketch_plane {
                    SketchPlane::Xy => Plane::xy(),
                    SketchPlane::Xz => Plane::xz(),
                    SketchPlane::Yz => Plane::yz(),
                };

                let profile_uv: Vec<(f64, f64)> =
                    segments.iter().map(|s| (s.from[0], s.from[1])).collect();
                solid = Some(make_extrusion(&plane, &profile_uv, *depth, gen)?);
            }
            Feature::CreateBox {
                id: _,
                width,
                height,
                depth,
            } => {
                if solid.is_some() {
                    return Err(KernelError::MultipleFeatures {
                        count: count_solid_features(features),
                    });
                }
                solid = Some(make_cuboid(*width, *height, *depth, gen)?);
            }
            Feature::CreateCylinder {
                id: _,
                radius,
                height,
            } => {
                if solid.is_some() {
                    return Err(KernelError::MultipleFeatures {
                        count: count_solid_features(features),
                    });
                }
                solid = Some(make_cylinder(*radius, *height, gen)?);
            }
            Feature::CreateSphere { id: _, radius } => {
                if solid.is_some() {
                    return Err(KernelError::MultipleFeatures {
                        count: count_solid_features(features),
                    });
                }
                solid = Some(make_sphere(*radius, gen)?);
            }
            Feature::Cut { .. } => {
                return Err(KernelError::UnsupportedFeature { kind: "cut" });
            }
            Feature::Fuse { .. } => {
                return Err(KernelError::UnsupportedFeature { kind: "fuse" });
            }
            Feature::Intersect { .. } => {
                return Err(KernelError::UnsupportedFeature { kind: "intersect" });
            }
        }
    }

    solid.ok_or(KernelError::EmptyFeatureList)
}

fn count_solid_features(features: &[Feature]) -> usize {
    features
        .iter()
        .filter(|f| {
            matches!(
                f,
                Feature::CreateBox { .. }
                    | Feature::CreateCylinder { .. }
                    | Feature::CreateSphere { .. }
                    | Feature::Extrude { .. }
            )
        })
        .count()
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
