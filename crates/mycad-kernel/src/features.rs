use crate::brep::topology::{IdGenerator, Solid};
use crate::error::KernelError;
use crate::primitives::make_cuboid;
use mycad_format::Feature;

pub fn build_solid_from_features(
    features: &[Feature],
    gen: &mut IdGenerator,
) -> Result<Solid, KernelError> {
    if features.is_empty() {
        return Err(KernelError::UnsupportedFeature(
            "empty feature list".to_string(),
        ));
    }

    if features.len() > 1 {
        return Err(KernelError::UnsupportedFeature(
            "multiple features (composition not yet implemented)".to_string(),
        ));
    }

    match &features[0] {
        Feature::CreateBox {
            width,
            height,
            depth,
            ..
        } => Ok(make_cuboid(*width, *height, *depth, gen)),
        Feature::CreateCylinder { .. } => {
            Err(KernelError::UnsupportedFeature("create_cylinder".into()))
        }
        Feature::CreateSphere { .. } => {
            Err(KernelError::UnsupportedFeature("create_sphere".into()))
        }
        Feature::Extrude { .. } => Err(KernelError::UnsupportedFeature("extrude".into())),
        Feature::Cut { .. } => Err(KernelError::UnsupportedFeature("cut".into())),
        Feature::Fuse { .. } => Err(KernelError::UnsupportedFeature("fuse".into())),
        Feature::Intersect { .. } => Err(KernelError::UnsupportedFeature("intersect".into())),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::brep::topology::IdGenerator;
    use crate::primitives::make_cuboid;
    use mycad_format::Feature;

    #[test]
    fn create_box_matches_make_cuboid() {
        let features = vec![Feature::CreateBox {
            id: "box_1".into(),
            width: 10.0,
            height: 20.0,
            depth: 30.0,
        }];

        let mut g1 = IdGenerator::new(0);
        let solid_a = build_solid_from_features(&features, &mut g1).unwrap();

        let mut g2 = IdGenerator::new(0);
        let solid_b = make_cuboid(10.0, 20.0, 30.0, &mut g2);

        assert_eq!(solid_a.vertices.len(), solid_b.vertices.len());
        assert_eq!(solid_a.edges.len(), solid_b.edges.len());
        assert_eq!(solid_a.faces.len(), solid_b.faces.len());
        assert_eq!(solid_a.half_edges.len(), solid_b.half_edges.len());
        assert_eq!(solid_a.loops.len(), solid_b.loops.len());
        assert_eq!(solid_a.shells.len(), solid_b.shells.len());
    }

    #[test]
    fn unsupported_feature_returns_error() {
        let features = vec![Feature::CreateCylinder {
            id: "c1".into(),
            radius: 1.0,
            height: 2.0,
        }];
        let mut g = IdGenerator::new(0);
        let err = build_solid_from_features(&features, &mut g).unwrap_err();
        match err {
            KernelError::UnsupportedFeature(name) => assert_eq!(name, "create_cylinder"),
        }
    }

    #[test]
    fn empty_features_returns_error() {
        let mut g = IdGenerator::new(0);
        let err = build_solid_from_features(&[], &mut g).unwrap_err();
        assert!(matches!(err, KernelError::UnsupportedFeature(_)));
    }
}
