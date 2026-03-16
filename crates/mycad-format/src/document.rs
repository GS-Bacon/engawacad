use crate::component::Component;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// The top-level document representing a MyCad design file.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct Document {
    /// Kernel version that created this document.
    pub version: String,
    /// The root component (assembly or single part).
    pub root_component: Component,
}

impl Document {
    /// Create a new document with a single empty root component.
    pub fn new(name: &str) -> Self {
        Self {
            version: env!("CARGO_PKG_VERSION").to_string(),
            root_component: Component::new(name),
        }
    }

    /// Serialize to YAML string.
    pub fn to_yaml(&self) -> Result<String, serde_yaml::Error> {
        serde_yaml::to_string(self)
    }

    /// Deserialize from YAML string.
    pub fn from_yaml(yaml: &str) -> Result<Self, serde_yaml::Error> {
        serde_yaml::from_str(yaml)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::feature::Feature;

    #[test]
    fn test_roundtrip_yaml() {
        let mut doc = Document::new("Test Assembly");
        doc.root_component.features.push(Feature::Extrude {
            id: "box_1".to_string(),
            sketch: "sketch_1".to_string(),
            depth: 10.0,
        });

        let yaml = doc.to_yaml().unwrap();
        let doc2 = Document::from_yaml(&yaml).unwrap();

        assert_eq!(doc.version, doc2.version);
        assert_eq!(doc.root_component.name, doc2.root_component.name);
        assert_eq!(
            doc.root_component.features.len(),
            doc2.root_component.features.len()
        );
    }

    #[test]
    fn test_yaml_is_deterministic() {
        let mut doc = Document::new("Test");
        doc.root_component.features.push(Feature::CreateBox {
            id: "box_1".to_string(),
            width: 10.0,
            height: 20.0,
            depth: 30.0,
        });

        let yaml1 = doc.to_yaml().unwrap();
        let yaml2 = doc.to_yaml().unwrap();
        assert_eq!(yaml1, yaml2, "YAML serialization must be deterministic");
    }
}
