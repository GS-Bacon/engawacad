use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// A reference to a topological entity (face, edge, vertex) on a feature's result.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, TS)]
pub struct EntityRef {
    /// The feature that created this entity.
    pub feature_id: String,
    /// The role of this entity in the feature (e.g., "top_face", "side_edge_0").
    pub role: String,
}

/// The plane on which a sketch is drawn.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, TS)]
#[serde(rename_all = "lowercase")]
pub enum SketchPlane {
    Xy,
    Xz,
    Yz,
}

/// A line segment in a 2D sketch profile.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, TS)]
pub struct SketchSegment {
    pub id: String,
    pub from: [f64; 2],
    pub to: [f64; 2],
}

/// A feature — one step in the operation history.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, TS)]
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

    /// Create a sketch (2D closed profile on a plane).
    #[serde(rename = "create_sketch")]
    CreateSketch {
        id: String,
        plane: SketchPlane,
        profile: Vec<SketchSegment>,
    },

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
            | Feature::CreateSketch { id, .. }
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
    fn test_create_sphere_yaml_golden() {
        let feature = Feature::CreateSphere {
            id: "sphere_1".to_string(),
            radius: 5.0,
        };
        let yaml = serde_yaml::to_string(&feature).unwrap();
        // Byte-identical golden for schema drift detection
        assert!(yaml.contains("type: create_sphere"), "tag missing: {yaml}");
        assert!(yaml.contains("id: sphere_1"), "id missing: {yaml}");
        assert!(yaml.contains("radius: 5.0"), "radius missing: {yaml}");
        // Roundtrip
        let back: Feature = serde_yaml::from_str(&yaml).unwrap();
        assert_eq!(back.id(), "sphere_1");
        assert!(
            matches!(back, Feature::CreateSphere { radius, .. } if (radius - 5.0).abs() < 1e-12)
        );
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

    #[test]
    fn test_create_sketch_yaml_golden() {
        let f = Feature::CreateSketch {
            id: "sketch_1".to_string(),
            plane: SketchPlane::Xy,
            profile: vec![
                SketchSegment {
                    id: "seg_a".to_string(),
                    from: [0.0, 0.0],
                    to: [10.0, 0.0],
                },
                SketchSegment {
                    id: "seg_b".to_string(),
                    from: [10.0, 0.0],
                    to: [10.0, 5.0],
                },
                SketchSegment {
                    id: "seg_c".to_string(),
                    from: [10.0, 5.0],
                    to: [0.0, 5.0],
                },
                SketchSegment {
                    id: "seg_d".to_string(),
                    from: [0.0, 5.0],
                    to: [0.0, 0.0],
                },
            ],
        };
        let yaml = serde_yaml::to_string(&f).unwrap();
        assert_eq!(
            yaml,
            "type: create_sketch\nid: sketch_1\nplane: xy\nprofile:\n- id: seg_a\n  from:\n  - 0.0\n  - 0.0\n  to:\n  - 10.0\n  - 0.0\n- id: seg_b\n  from:\n  - 10.0\n  - 0.0\n  to:\n  - 10.0\n  - 5.0\n- id: seg_c\n  from:\n  - 10.0\n  - 5.0\n  to:\n  - 0.0\n  - 5.0\n- id: seg_d\n  from:\n  - 0.0\n  - 5.0\n  to:\n  - 0.0\n  - 0.0\n",
            "CreateSketch YAML golden mismatch — field order or rename drifted"
        );
        let back: Feature = serde_yaml::from_str(&yaml).unwrap();
        assert_eq!(back.id(), "sketch_1");
    }
}
