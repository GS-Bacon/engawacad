use crate::error::FormatError;
use crate::feature::Feature;
use crate::ref_plane::{is_default_canonical_three, RefPlane};
use serde::{Deserialize, Serialize};
use std::fmt;
use std::path::PathBuf;
use std::str::FromStr;
use ts_rs::{Config, TS};

/// A transform in 3D space.
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema, TS)]
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

/// Reference to an external component, serialized as a URI string.
///
/// - `"stdlib://path/to/part"` → `ComponentRef::StdLib("path/to/part")`
/// - Any other string → `ComponentRef::File(value)`
#[derive(Debug, Clone)]
pub enum ComponentRef {
    /// Standard library reference (e.g., `"stdlib://fasteners/jis_b1176/M5x20"`)
    StdLib(String),
    /// File reference (e.g., `"./motor_v2.engawa"`)
    File(String),
}

impl FromStr for ComponentRef {
    type Err = FormatError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if s.is_empty() {
            return Err(FormatError::InvalidReference {
                value: s.to_string(),
                reason: "reference must not be empty",
            });
        }
        if let Some(path) = s.strip_prefix("stdlib://") {
            if path.is_empty() {
                return Err(FormatError::InvalidReference {
                    value: s.to_string(),
                    reason: "stdlib reference must have a non-empty path after 'stdlib://'",
                });
            }
            Ok(ComponentRef::StdLib(path.to_string()))
        } else {
            Ok(ComponentRef::File(s.to_string()))
        }
    }
}

impl fmt::Display for ComponentRef {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ComponentRef::StdLib(path) => write!(f, "stdlib://{path}"),
            ComponentRef::File(path) => write!(f, "{path}"),
        }
    }
}

impl Serialize for ComponentRef {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.to_string())
    }
}

impl<'de> Deserialize<'de> for ComponentRef {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let s = String::deserialize(deserializer)?;
        s.parse().map_err(serde::de::Error::custom)
    }
}

impl schemars::JsonSchema for ComponentRef {
    fn schema_name() -> String {
        "ComponentRef".to_string()
    }

    fn json_schema(gen: &mut schemars::gen::SchemaGenerator) -> schemars::schema::Schema {
        <String as schemars::JsonSchema>::json_schema(gen)
    }
}

impl TS for ComponentRef {
    type WithoutGenerics = ComponentRef;
    type OptionInnerType = Self;

    fn name(_cfg: &Config) -> String {
        "ComponentRef".to_owned()
    }

    fn inline(cfg: &Config) -> String {
        <String as TS>::inline(cfg)
    }

    fn decl(cfg: &Config) -> String {
        format!("type {} = {};", Self::name(cfg), Self::inline(cfg))
    }

    fn output_path() -> Option<PathBuf> {
        Some(PathBuf::from("ComponentRef.ts"))
    }
}

/// A component in the design hierarchy.
/// Can contain features (inline part definition), children (sub-components),
/// or be a reference to an external file/library.
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema, TS)]
pub struct Component {
    /// Human-readable name.
    pub name: String,

    /// Transform relative to the parent component.
    #[serde(default, skip_serializing_if = "is_default_transform")]
    pub transform: Transform,

    /// External reference (if this component is not defined inline).
    #[serde(default, skip_serializing_if = "Option::is_none", rename = "ref")]
    pub reference: Option<ComponentRef>,

    /// Ordered list of features (the operation history).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub features: Vec<Feature>,

    /// Child components.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub children: Vec<Component>,

    /// Reference planes for sketch creation.
    /// If omitted during serialization, defaults to the canonical three (Front, Top, Right).
    #[serde(default, skip_serializing_if = "is_default_canonical_three")]
    pub ref_planes: Vec<RefPlane>,
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
            ref_planes: Vec::new(),
        }
    }

    /// Create a component referencing an external file or library.
    pub fn from_ref(name: &str, reference: &str) -> Result<Self, FormatError> {
        Ok(Self {
            name: name.to_string(),
            transform: Transform::default(),
            reference: Some(reference.parse()?),
            features: Vec::new(),
            children: Vec::new(),
            ref_planes: Vec::new(),
        })
    }
}
