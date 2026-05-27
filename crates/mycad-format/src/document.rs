use crate::component::Component;
use crate::error::FormatError;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::path::Path;
use ts_rs::TS;

pub const CURRENT_SCHEMA_VERSION: u32 = 1;

fn default_schema_version() -> u32 {
    CURRENT_SCHEMA_VERSION
}

/// The top-level document representing a MyCad design file.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, TS)]
pub struct Document {
    /// Format schema version. Increment when the .mycad file format changes in a breaking way.
    #[serde(default = "default_schema_version")]
    pub schema_version: u32,
    /// Kernel version that created this document.
    pub version: String,
    /// The root component (assembly or single part).
    pub root_component: Component,
}

impl Document {
    /// Create a new document with a single empty root component.
    pub fn new(name: &str) -> Self {
        Self {
            schema_version: CURRENT_SCHEMA_VERSION,
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

    /// Load a Document from a `.mycad` file path.
    pub fn from_path(path: &Path) -> Result<Self, FormatError> {
        let ext = path.extension().and_then(|s| s.to_str());
        if ext != Some("mycad") {
            return Err(FormatError::InvalidExtension(ext.map(str::to_string)));
        }
        let content = std::fs::read_to_string(path)?;
        let doc = Self::from_yaml(&content)?;
        Ok(doc)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::FormatError;
    use crate::feature::Feature;
    use std::path::Path;

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

    #[test]
    fn test_from_path_loads_simple_box() {
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join("..")
            .join("examples")
            .join("simple_box.mycad");
        let doc = Document::from_path(&path).unwrap();
        assert_eq!(doc.version, "0.1.0");
        assert_eq!(doc.root_component.name, "Simple Box");
    }

    #[test]
    fn test_from_path_missing_file_returns_io_error() {
        let path = Path::new("does_not_exist.mycad");
        let err = Document::from_path(path).unwrap_err();
        assert!(matches!(err, FormatError::Io(_)));
    }

    #[test]
    fn test_from_path_invalid_extension() {
        let path = Path::new("foo.txt");
        let err = Document::from_path(path).unwrap_err();
        assert!(matches!(err, FormatError::InvalidExtension(Some(ref e)) if e == "txt"));
    }

    #[test]
    fn test_assembly_roundtrip() {
        use crate::component::ComponentRef;

        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join("..")
            .join("examples")
            .join("assembly.mycad");
        let doc = Document::from_path(&path).unwrap();

        // Bolt child has a stdlib reference
        let bolt = doc
            .root_component
            .children
            .iter()
            .find(|c| c.name == "Bolt")
            .expect("Bolt component not found");
        assert!(
            matches!(&bolt.reference, Some(ComponentRef::StdLib(p)) if p == "fasteners/jis_b1176/M5x20")
        );

        // Roundtrip: serialize then deserialize yields the same YAML
        let yaml = doc.to_yaml().unwrap();
        let doc2 = Document::from_yaml(&yaml).unwrap();
        assert_eq!(doc.to_yaml().unwrap(), doc2.to_yaml().unwrap());
    }

    #[test]
    fn test_schema_version_backward_compat() {
        // schema_version フィールドがない古い形式の YAML でも読めること
        let old_yaml = "version: 0.1.0\nroot_component:\n  name: Old\n  features: []\n";
        let doc = Document::from_yaml(old_yaml).expect("old yaml should parse");
        assert_eq!(
            doc.schema_version, 1,
            "missing schema_version defaults to 1"
        );
        // 再シリアライズすると schema_version: 1 が出力されること
        let yaml = doc.to_yaml().unwrap();
        assert!(
            yaml.starts_with("schema_version: 1\n"),
            "re-serialized yaml must include schema_version"
        );
    }

    /// T08: TS derive 追加後も .mycad fixture の YAML 表現が不変であること。
    #[test]
    fn test_ts_derive_backward_compat() {
        let examples_dir = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join("..")
            .join("examples");

        for entry in std::fs::read_dir(&examples_dir).unwrap() {
            let entry = entry.unwrap();
            let path = entry.path();
            if path.extension().map_or(true, |e| e != "mycad") {
                continue;
            }

            let doc = Document::from_path(&path)
                .unwrap_or_else(|e| panic!("failed to load {:?}: {e}", path));
            let yaml1 = doc.to_yaml().unwrap();
            let doc2 = Document::from_yaml(&yaml1)
                .unwrap_or_else(|e| panic!("failed to parse roundtrip YAML for {:?}: {e}", path));
            let yaml2 = doc2.to_yaml().unwrap();
            assert_eq!(
                yaml1,
                yaml2,
                "YAML roundtrip mismatch for {}",
                path.file_name().unwrap().to_string_lossy()
            );
        }

        // Golden comparison: simple_box.mycad canonical YAML must be byte-identical
        let simple_box_path = examples_dir.join("simple_box.mycad");
        let doc = Document::from_path(&simple_box_path).unwrap();
        let yaml = doc.to_yaml().unwrap();
        let golden = "schema_version: 1\nversion: 0.1.0\nroot_component:\n  name: Simple Box\n  features:\n  - type: create_box\n    id: box_1\n    width: 10.0\n    height: 20.0\n    depth: 30.0\n";
        assert_eq!(yaml, golden, "simple_box.mycad golden YAML mismatch");
    }

    #[test]
    fn test_extruded_rect_yaml_golden() {
        use crate::feature::{Feature, SketchPlane, SketchSegment};
        use std::path::Path;

        // Load from file and verify exact serialization
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join("..")
            .join("examples")
            .join("extruded_rect.mycad");
        let doc = Document::from_path(&path).unwrap();
        let yaml = doc.to_yaml().unwrap();

        static GOLDEN: &str = concat!(
            "schema_version: 1\nversion: 0.1.0\nroot_component:\n  name: Extruded Rect\n  features:\n",
            "  - type: create_sketch\n    id: sketch_1\n    plane: xy\n    profile:\n",
            "    - id: seg_a\n      from:\n      - 0.0\n      - 0.0\n      to:\n      - 10.0\n      - 0.0\n",
            "    - id: seg_b\n      from:\n      - 10.0\n      - 0.0\n      to:\n      - 10.0\n      - 5.0\n",
            "    - id: seg_c\n      from:\n      - 10.0\n      - 5.0\n      to:\n      - 0.0\n      - 5.0\n",
            "    - id: seg_d\n      from:\n      - 0.0\n      - 5.0\n      to:\n      - 0.0\n      - 0.0\n",
            "  - type: extrude\n    id: extrude_1\n    sketch: sketch_1\n    depth: 8.0\n",
        );
        assert_eq!(yaml, GOLDEN, "extruded_rect.mycad YAML golden mismatch");

        // Also verify constructed Document produces same YAML
        let mut doc2 = Document::new("Extruded Rect");
        doc2.root_component.features.push(Feature::CreateSketch {
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
        });
        doc2.root_component.features.push(Feature::Extrude {
            id: "extrude_1".to_string(),
            sketch: "sketch_1".to_string(),
            depth: 8.0,
        });
        assert_eq!(
            doc2.to_yaml().unwrap(),
            GOLDEN,
            "constructed doc YAML must match golden"
        );
    }

    // --- Edge case tests (adversarial persona) ---

    #[test]
    fn test_schema_version_explicit_zero() {
        let yaml =
            "schema_version: 0\nversion: 0.1.0\nroot_component:\n  name: Zero\n  features: []\n";
        let doc = Document::from_yaml(yaml).expect("schema_version 0 should parse");
        assert_eq!(doc.schema_version, 0);
        let reserialized = doc.to_yaml().unwrap();
        assert!(
            reserialized.starts_with("schema_version: 0\n"),
            "should preserve explicit 0"
        );
    }

    #[test]
    fn test_schema_version_large_value() {
        let yaml = "schema_version: 4294967295\nversion: 0.1.0\nroot_component:\n  name: Max\n  features: []\n";
        let doc = Document::from_yaml(yaml).expect("u32 max should parse");
        assert_eq!(doc.schema_version, u32::MAX);
    }

    #[test]
    fn test_schema_version_roundtrip_preserves_value() {
        let yaml =
            "schema_version: 42\nversion: 0.1.0\nroot_component:\n  name: v42\n  features: []\n";
        let doc = Document::from_yaml(yaml).unwrap();
        let reserialized = doc.to_yaml().unwrap();
        let doc2 = Document::from_yaml(&reserialized).unwrap();
        assert_eq!(doc2.schema_version, 42);
    }

    #[test]
    fn test_new_doc_has_current_schema_version() {
        let doc = Document::new("Test");
        assert_eq!(doc.schema_version, CURRENT_SCHEMA_VERSION);
    }

    #[test]
    fn test_yaml_deterministic_100_runs() {
        let mut doc = Document::new("Determinism");
        doc.root_component.features.push(Feature::CreateBox {
            id: "box_1".to_string(),
            width: 10.0,
            height: 20.0,
            depth: 30.0,
        });
        let reference = doc.to_yaml().unwrap();
        for i in 0..100 {
            let yaml = doc.to_yaml().unwrap();
            assert_eq!(yaml, reference, "serialization differs at run {i}");
        }
    }

    #[test]
    fn test_schema_version_negative_rejected() {
        let yaml =
            "schema_version: -1\nversion: 0.1.0\nroot_component:\n  name: Neg\n  features: []\n";
        let result = Document::from_yaml(yaml);
        assert!(
            result.is_err(),
            "negative schema_version should be rejected"
        );
    }

    #[test]
    fn test_schema_version_string_rejected() {
        let yaml = "schema_version: \"hello\"\nversion: 0.1.0\nroot_component:\n  name: Str\n  features: []\n";
        let result = Document::from_yaml(yaml);
        assert!(result.is_err(), "string schema_version should be rejected");
    }

    #[test]
    fn test_empty_features_yaml() {
        let yaml =
            "schema_version: 1\nversion: 0.1.0\nroot_component:\n  name: Empty\n  features: []\n";
        let doc = Document::from_yaml(yaml).unwrap();
        assert!(doc.root_component.features.is_empty());
    }

    #[test]
    fn test_missing_features_defaults() {
        let yaml = "schema_version: 1\nversion: 0.1.0\nroot_component:\n  name: NoFeatures\n";
        let doc = Document::from_yaml(yaml).unwrap();
        assert!(doc.root_component.features.is_empty());
    }

    #[test]
    fn test_all_example_files_have_schema_version() {
        let examples_dir = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join("..")
            .join("examples");

        for entry in std::fs::read_dir(&examples_dir).unwrap() {
            let entry = entry.unwrap();
            let path = entry.path();
            if path.extension().map_or(true, |e| e != "mycad") {
                continue;
            }
            let content = std::fs::read_to_string(&path).unwrap();
            assert!(
                content.starts_with("schema_version: 1\n"),
                "{} should start with schema_version: 1",
                path.file_name().unwrap().to_string_lossy()
            );
            let doc = Document::from_path(&path).unwrap();
            assert_eq!(
                doc.schema_version,
                1,
                "{} should have schema_version 1",
                path.file_name().unwrap().to_string_lossy()
            );
        }
    }
}
