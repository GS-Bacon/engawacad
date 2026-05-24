use mycad_format::Feature;
use mycad_kernel::brep::topology::{IdGenerator, Solid};
use mycad_kernel::error::KernelError;
use mycad_kernel::primitives::{make_cuboid, make_cylinder};

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
        Feature::CreateCylinder { radius, height, .. } => make_cylinder(*radius, *height, gen),
        Feature::CreateSphere { .. } => Err(KernelError::UnsupportedFeature {
            kind: "create_sphere",
        }),
        Feature::Extrude { .. } => Err(KernelError::UnsupportedFeature { kind: "extrude" }),
        Feature::Cut { .. } => Err(KernelError::UnsupportedFeature { kind: "cut" }),
        Feature::Fuse { .. } => Err(KernelError::UnsupportedFeature { kind: "fuse" }),
        Feature::Intersect { .. } => Err(KernelError::UnsupportedFeature { kind: "intersect" }),
    }
}
