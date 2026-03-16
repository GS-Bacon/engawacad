use crate::feature::Feature;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// A transform in 3D space.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct Transform {
    /// Position offset [x, y, z].
    pub position: [f64; 3],
    /// Rotation as Euler angles in degrees [rx, ry, rz].
    pub rotation: [f64; 3],
}

impl Default for Transform {
    fn default() -> Self {
        Self {
            position: [0.0, 0.0, 0.0],
            rotation: [0.0, 0.0, 0.0],
        }
    }
}

/// Reference to an external component.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(untagged)]
pub enum ComponentRef {
    /// Standard library reference (e.g., "stdlib://fasteners/jis_b1176/M5x20")
    StdLib(String),
    /// File reference (e.g., "./motor_v2.mycad")
    File(String),
}

/// A component in the design hierarchy.
/// Can contain features (inline part definition), children (sub-components),
/// or be a reference to an external file/library.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct Component {
    /// Human-readable name.
    pub name: String,

    /// Transform relative to the parent component.
    #[serde(default, skip_serializing_if = "is_default_transform")]
    pub transform: Transform,

    /// External reference (if this component is not defined inline).
    #[serde(default, skip_serializing_if = "Option::is_none", rename = "ref")]
    pub reference: Option<String>,

    /// Ordered list of features (the operation history).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub features: Vec<Feature>,

    /// Child components.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub children: Vec<Component>,
}

fn is_default_transform(t: &Transform) -> bool {
    t.position == [0.0, 0.0, 0.0] && t.rotation == [0.0; 3]
}

impl Component {
    /// Create a new empty component.
    pub fn new(name: &str) -> Self {
        Self {
            name: name.to_string(),
            transform: Transform::default(),
            reference: None,
            features: Vec::new(),
            children: Vec::new(),
        }
    }

    /// Create a component referencing an external file or library.
    pub fn from_ref(name: &str, reference: &str) -> Self {
        Self {
            name: name.to_string(),
            transform: Transform::default(),
            reference: Some(reference.to_string()),
            features: Vec::new(),
            children: Vec::new(),
        }
    }
}
