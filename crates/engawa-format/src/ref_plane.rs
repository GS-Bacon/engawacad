//! Reference plane for sketches.
//!
//! A `RefPlane` represents a plane in 3D space where a 2D sketch can be drawn.
//! The canonical three planes (Front/Xy, Top/Xz, Right/Yz) are automatically
//! populated in `Document::new()` and `Document::from_yaml()` when `ref_planes` is empty.

use crate::feature::SketchPlane;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// A reference plane for sketch creation.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, TS)]
pub struct RefPlane {
    /// Unique identifier for this reference plane (e.g., "Front", "Top", "Right").
    pub id: String,

    /// The sketch plane type (xy, xz, or yz).
    pub plane: SketchPlane,

    /// Offset from the origin along the plane's normal.
    #[serde(default, skip_serializing_if = "is_zero")]
    pub offset: f64,
}

fn is_zero(v: &f64) -> bool {
    *v == 0.0
}

impl RefPlane {
    /// Returns the canonical three reference planes in a fixed order:
    /// Front (Xy, 0.0), Top (Xz, 0.0), Right (Yz, 0.0).
    ///
    /// This is a pure function and always returns the same result.
    /// The order is guaranteed to be `[Front, Top, Right]`.
    pub fn default_canonical_three() -> Vec<RefPlane> {
        vec![RefPlane::front(), RefPlane::top(), RefPlane::right()]
    }

    /// Front plane (xy plane, offset 0.0).
    pub fn front() -> RefPlane {
        RefPlane {
            id: "Front".to_string(),
            plane: SketchPlane::Xy,
            offset: 0.0,
        }
    }

    /// Top plane (xz plane, offset 0.0).
    pub fn top() -> RefPlane {
        RefPlane {
            id: "Top".to_string(),
            plane: SketchPlane::Xz,
            offset: 0.0,
        }
    }

    /// Right plane (yz plane, offset 0.0).
    pub fn right() -> RefPlane {
        RefPlane {
            id: "Right".to_string(),
            plane: SketchPlane::Yz,
            offset: 0.0,
        }
    }
}

/// Returns true if the ref_planes vector should be skipped during serialization.
///
/// An empty ref_planes list (common in child Components or before seeding) or
/// the default canonical three (Front/Top/Right) are both considered "default"
/// and will be omitted from YAML to keep it concise.
///
/// Used for `#[serde(skip_serializing_if = ...)]`.
pub fn is_default_canonical_three(ref_planes: &[RefPlane]) -> bool {
    if ref_planes.is_empty() {
        return true;
    }
    let default = RefPlane::default_canonical_three();
    if ref_planes.len() != default.len() {
        return false;
    }
    ref_planes
        .iter()
        .zip(default.iter())
        .all(|(a, b)| a.id == b.id && a.plane == b.plane && a.offset == b.offset)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_canonical_three_order() {
        let planes = RefPlane::default_canonical_three();
        assert_eq!(planes.len(), 3);
        assert_eq!(planes[0].id, "Front");
        assert_eq!(planes[0].plane, SketchPlane::Xy);
        assert_eq!(planes[0].offset, 0.0);
        assert_eq!(planes[1].id, "Top");
        assert_eq!(planes[1].plane, SketchPlane::Xz);
        assert_eq!(planes[1].offset, 0.0);
        assert_eq!(planes[2].id, "Right");
        assert_eq!(planes[2].plane, SketchPlane::Yz);
        assert_eq!(planes[2].offset, 0.0);
    }

    #[test]
    fn test_default_canonical_three_deterministic() {
        let a = RefPlane::default_canonical_three();
        let b = RefPlane::default_canonical_three();
        assert_eq!(a.len(), b.len());
        for (pa, pb) in a.iter().zip(b.iter()) {
            assert_eq!(pa.id, pb.id);
            assert_eq!(pa.plane, pb.plane);
            assert_eq!(pa.offset, pb.offset);
        }
    }

    #[test]
    fn test_is_default_canonical_three_true() {
        let planes = RefPlane::default_canonical_three();
        assert!(is_default_canonical_three(&planes));
    }

    #[test]
    fn test_is_default_canonical_three_false_wrong_order() {
        let mut planes = RefPlane::default_canonical_three();
        planes.reverse();
        assert!(!is_default_canonical_three(&planes));
    }

    #[test]
    fn test_is_default_canonical_three_false_custom_offset() {
        let mut planes = RefPlane::default_canonical_three();
        planes[0].offset = 5.0;
        assert!(!is_default_canonical_three(&planes));
    }

    #[test]
    fn test_is_default_canonical_three_false_extra_plane() {
        let mut planes = RefPlane::default_canonical_three();
        planes.push(RefPlane {
            id: "Custom".to_string(),
            plane: SketchPlane::Xy,
            offset: 0.0,
        });
        assert!(!is_default_canonical_three(&planes));
    }

    #[test]
    fn test_is_default_canonical_three_false_missing_plane() {
        let mut planes = RefPlane::default_canonical_three();
        planes.pop();
        assert!(!is_default_canonical_three(&planes));
    }

    #[test]
    fn test_front_top_right_constructors() {
        let front = RefPlane::front();
        assert_eq!(front.id, "Front");
        assert_eq!(front.plane, SketchPlane::Xy);
        assert_eq!(front.offset, 0.0);

        let top = RefPlane::top();
        assert_eq!(top.id, "Top");
        assert_eq!(top.plane, SketchPlane::Xz);
        assert_eq!(top.offset, 0.0);

        let right = RefPlane::right();
        assert_eq!(right.id, "Right");
        assert_eq!(right.plane, SketchPlane::Yz);
        assert_eq!(right.offset, 0.0);
    }

    #[test]
    fn test_ref_plane_serialization() {
        let rp = RefPlane {
            id: "Custom".to_string(),
            plane: SketchPlane::Xy,
            offset: 5.0,
        };
        let yaml = serde_yaml::to_string(&rp).unwrap();
        let back: RefPlane = serde_yaml::from_str(&yaml).unwrap();
        assert_eq!(back.id, rp.id);
        assert_eq!(back.plane, rp.plane);
        assert_eq!(back.offset, rp.offset);
    }

    #[test]
    fn test_ref_plane_zero_offset_skipped() {
        let rp = RefPlane {
            id: "Test".to_string(),
            plane: SketchPlane::Xy,
            offset: 0.0,
        };
        let yaml = serde_yaml::to_string(&rp).unwrap();
        assert!(!yaml.contains("offset"));
    }

    #[test]
    fn test_ref_plane_nonzero_offset_included() {
        let rp = RefPlane {
            id: "Test".to_string(),
            plane: SketchPlane::Xy,
            offset: 5.0,
        };
        let yaml = serde_yaml::to_string(&rp).unwrap();
        assert!(yaml.contains("offset: 5"));
    }
}
