use crate::component::Component;
use crate::error::FormatError;
use crate::ref_plane::RefPlane;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::path::Path;
use ts_rs::TS;

pub const CURRENT_SCHEMA_VERSION: u32 = 1;

fn default_schema_version() -> u32 {
    CURRENT_SCHEMA_VERSION
}

/// The top-level document representing a EngawaCAD design file.
#[derive(Debug, Clone, Serialize, JsonSchema, TS)]
pub struct Document {
    /// Format schema version. Increment when the .engawa file format changes in a breaking way.
    #[serde(default = "default_schema_version")]
    pub schema_version: u32,
    /// Kernel version that created this document.
    pub version: String,
    /// The root component (assembly or single part).
    pub root_component: Component,
}

/// Wire-format shadow for deserialization (no validation).
#[derive(Deserialize)]
struct RawDocument {
    #[serde(default = "default_schema_version")]
    schema_version: u32,
    version: String,
    root_component: Component,
}

impl Document {
    /// Create a new document with a single empty root component.
    /// The root component will have the canonical three reference planes (Front, Top, Right).
    pub fn new(name: &str) -> Self {
        let mut root = Component::new(name);
        root.ref_planes = RefPlane::default_canonical_three();
        Self {
            schema_version: CURRENT_SCHEMA_VERSION,
            version: env!("CARGO_PKG_VERSION").to_string(),
            root_component: root,
        }
    }

    /// Validate the entire document tree.
    pub fn validate(&self) -> Result<(), FormatError> {
        validate_component(&self.root_component, &self.root_component.name)
    }

    /// Serialize to YAML string (validates first).
    pub fn to_yaml(&self) -> Result<String, FormatError> {
        self.validate()?;
        Ok(serde_yaml::to_string(self)?)
    }

    /// Deserialize from YAML string with typed validation errors.
    pub fn from_yaml(yaml: &str) -> Result<Self, FormatError> {
        let raw: RawDocument = serde_yaml::from_str(yaml)?;
        let mut doc = Document {
            schema_version: raw.schema_version,
            version: raw.version,
            root_component: raw.root_component,
        };
        // Populate default ref_planes if empty
        if doc.root_component.ref_planes.is_empty() {
            doc.root_component.ref_planes = RefPlane::default_canonical_three();
        }
        doc.validate()?;
        Ok(doc)
    }

    /// Load a Document from a `.engawa` file path.
    pub fn from_path(path: &Path) -> Result<Self, FormatError> {
        let ext = path.extension().and_then(|s| s.to_str());
        if ext != Some("engawa") {
            return Err(FormatError::InvalidExtension(ext.map(str::to_string)));
        }
        let content = std::fs::read_to_string(path)?;
        Self::from_yaml(&content)
    }
}

impl<'de> Deserialize<'de> for Document {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let raw = RawDocument::deserialize(deserializer)?;
        let mut doc = Document {
            schema_version: raw.schema_version,
            version: raw.version,
            root_component: raw.root_component,
        };
        // Populate default ref_planes if empty
        if doc.root_component.ref_planes.is_empty() {
            doc.root_component.ref_planes = RefPlane::default_canonical_three();
        }
        doc.validate().map_err(serde::de::Error::custom)?;
        Ok(doc)
    }
}

fn validate_identifier(value: &str, _field: &'static str) -> Result<(), FormatError> {
    if value.is_empty() {
        return Err(FormatError::InvalidName {
            value: value.to_string(),
            reason: "feature_id must not be empty",
        });
    }
    if !value
        .bytes()
        .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
    {
        return Err(FormatError::InvalidName {
            value: value.to_string(),
            reason: "feature_id contains invalid characters",
        });
    }
    Ok(())
}

fn check_finite_position(id: &str, p: &[f64; 3]) -> Result<(), FormatError> {
    if p.iter().any(|v| !v.is_finite()) {
        return Err(FormatError::InvalidPosition {
            id: id.to_string(),
            reason: "position components must be finite (no NaN/Inf)",
        });
    }
    Ok(())
}

fn validate_component(component: &Component, component_name: &str) -> Result<(), FormatError> {
    if let Some(reference) = &component.reference {
        match reference {
            crate::component::ComponentRef::StdLib(path) if path.is_empty() => {
                return Err(FormatError::InvalidReference {
                    value: "stdlib://".to_string(),
                    reason: "stdlib reference must have a non-empty path after 'stdlib://'",
                });
            }
            crate::component::ComponentRef::File(path) if path.is_empty() => {
                return Err(FormatError::InvalidReference {
                    value: String::new(),
                    reason: "reference must not be empty",
                });
            }
            crate::component::ComponentRef::File(path) if path.starts_with("stdlib://") => {
                return Err(FormatError::InvalidReference {
                    value: path.clone(),
                    reason: "file reference must not start with 'stdlib://'",
                });
            }
            _ => {}
        }
    }

    // Validate RefPlane id uniqueness and offset finiteness
    let mut seen_ref_planes = std::collections::HashSet::new();
    for ref_plane in &component.ref_planes {
        validate_identifier(&ref_plane.id, "ref_plane_id")?;
        if !seen_ref_planes.insert(&ref_plane.id) {
            return Err(FormatError::DuplicateRefPlaneId {
                id: ref_plane.id.clone(),
                component: component_name.to_string(),
            });
        }
        if !ref_plane.offset.is_finite() {
            return Err(FormatError::InvalidRefPlaneOffset {
                id: ref_plane.id.clone(),
            });
        }
    }

    let mut seen = std::collections::HashSet::new();
    for feature in &component.features {
        let id = feature.id();
        validate_identifier(id, "feature_id")?;
        if !seen.insert(id.to_string()) {
            return Err(FormatError::DuplicateFeatureId {
                id: id.to_string(),
                component: component_name.to_string(),
            });
        }
        match feature {
            crate::feature::Feature::CreateCylinder { origin, .. } => {
                check_finite_position(id, origin)?
            }
            crate::feature::Feature::CreateSphere { center, .. } => {
                check_finite_position(id, center)?
            }
            _ => {}
        }
    }
    for child in &component.children {
        validate_component(child, &child.name)?;
    }
    Ok(())
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
            fuse_target: None,
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
            .join("simple_box.engawa");
        let doc = Document::from_path(&path).unwrap();
        assert_eq!(doc.version, "0.1.0");
        assert_eq!(doc.root_component.name, "Simple Box");
    }

    #[test]
    fn test_from_path_missing_file_returns_io_error() {
        let path = Path::new("does_not_exist.engawa");
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
            .join("assembly.engawa");
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

    /// T08: TS derive 追加後も .engawa fixture の YAML 表現が不変であること。
    #[test]
    fn test_ts_derive_backward_compat() {
        let examples_dir = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join("..")
            .join("examples");

        for entry in std::fs::read_dir(&examples_dir).unwrap() {
            let entry = entry.unwrap();
            let path = entry.path();
            if path.extension().map_or(true, |e| e != "engawa") {
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

        // Golden comparison: simple_box.engawa canonical YAML must be byte-identical
        let simple_box_path = examples_dir.join("simple_box.engawa");
        let doc = Document::from_path(&simple_box_path).unwrap();
        let yaml = doc.to_yaml().unwrap();
        let golden = "schema_version: 1\nversion: 0.1.0\nroot_component:\n  name: Simple Box\n  features:\n  - type: create_box\n    id: box_1\n    width: 10.0\n    height: 20.0\n    depth: 30.0\n";
        assert_eq!(yaml, golden, "simple_box.engawa golden YAML mismatch");
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
            .join("extruded_rect.engawa");
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
        assert_eq!(yaml, GOLDEN, "extruded_rect.engawa YAML golden mismatch");

        // Also verify constructed Document produces same YAML
        let mut doc2 = Document::new("Extruded Rect");
        doc2.root_component.features.push(Feature::CreateSketch {
            id: "sketch_1".to_string(),
            plane: SketchPlane::Xy,
            offset: 0.0,
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
            plane_ref: None,
        });
        doc2.root_component.features.push(Feature::Extrude {
            id: "extrude_1".to_string(),
            sketch: "sketch_1".to_string(),
            depth: 8.0,
            fuse_target: None,
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
            if path.extension().map_or(true, |e| e != "engawa") {
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

    // --- T07-T14: Topological naming document validation tests ---

    /// T07: Duplicate feature_id in same component rejected.
    #[test]
    fn t07_duplicate_feature_id_rejected() {
        let yaml = "\
schema_version: 1
version: 0.1.0
root_component:
  name: Dup
  features:
    - type: create_box
      id: box_1
      width: 10.0
      height: 20.0
      depth: 30.0
    - type: create_box
      id: box_1
      width: 5.0
      height: 5.0
      depth: 5.0
";
        let result = Document::from_yaml(yaml);
        assert!(result.is_err());
        match result.unwrap_err() {
            FormatError::DuplicateFeatureId { id, component } => {
                assert_eq!(id, "box_1");
                assert_eq!(component, "Dup");
            }
            other => panic!("expected DuplicateFeatureId, got {other:?}"),
        }
    }

    /// T08: Same feature_id in sibling components is allowed.
    #[test]
    fn t08_same_id_in_sibling_components_allowed() {
        let yaml = "\
schema_version: 1
version: 0.1.0
root_component:
  name: Root
  children:
    - name: PartA
      features:
        - type: create_box
          id: box_1
          width: 10.0
          height: 20.0
          depth: 30.0
    - name: PartB
      features:
        - type: create_box
          id: box_1
          width: 5.0
          height: 5.0
          depth: 5.0
";
        let doc =
            Document::from_yaml(yaml).expect("same id in sibling components should be allowed");
        assert_eq!(doc.root_component.children.len(), 2);
    }

    /// T09a: from_yaml returns typed FormatError for invalid feature_id.
    #[test]
    fn t09a_from_yaml_typed_error_invalid_feature_id() {
        let yaml = "\
schema_version: 1
version: 0.1.0
root_component:
  name: Test
  features:
    - type: create_box
      id: ''
      width: 10.0
      height: 20.0
      depth: 30.0
";
        let result = Document::from_yaml(yaml);
        assert!(matches!(result, Err(FormatError::InvalidName { .. })));
    }

    /// T09b: Direct serde_yaml::from_str::<Document> also rejects invalid feature_id.
    #[test]
    fn t09b_direct_deserialize_rejects_invalid() {
        let yaml = "\
schema_version: 1
version: 0.1.0
root_component:
  name: Test
  features:
    - type: create_box
      id: ''
      width: 10.0
      height: 20.0
      depth: 30.0
";
        let result: Result<Document, serde_yaml::Error> = serde_yaml::from_str(yaml);
        assert!(result.is_err());
    }

    /// T14: to_yaml rejects document with invalid feature_id.
    #[test]
    fn t14_to_yaml_rejects_invalid() {
        let mut doc = Document::new("Test");
        doc.root_component.features.push(Feature::CreateBox {
            id: "".to_string(),
            width: 10.0,
            height: 20.0,
            depth: 30.0,
        });
        let result = doc.to_yaml();
        assert!(matches!(result, Err(FormatError::InvalidName { .. })));
    }

    /// T14b: to_yaml rejects document with duplicate feature_id.
    #[test]
    fn t14b_to_yaml_rejects_duplicate() {
        let mut doc = Document::new("Test");
        doc.root_component.features.push(Feature::CreateBox {
            id: "box_1".to_string(),
            width: 10.0,
            height: 20.0,
            depth: 30.0,
        });
        doc.root_component.features.push(Feature::CreateSphere {
            id: "box_1".to_string(),
            radius: 5.0,
            center: [0.0, 0.0, 0.0],
        });
        let result = doc.to_yaml();
        assert!(matches!(
            result,
            Err(FormatError::DuplicateFeatureId { .. })
        ));
    }

    /// T09a extended: DuplicateFeatureId returned as typed error from from_yaml.
    #[test]
    fn t09a_from_yaml_typed_error_duplicate() {
        let yaml = "\
schema_version: 1
version: 0.1.0
root_component:
  name: Test
  features:
    - type: create_box
      id: dup
      width: 10.0
      height: 20.0
      depth: 30.0
    - type: create_sphere
      id: dup
      radius: 5.0
";
        let result = Document::from_yaml(yaml);
        match result {
            Err(FormatError::DuplicateFeatureId { id, component }) => {
                assert_eq!(id, "dup");
                assert_eq!(component, "Test");
            }
            other => panic!("expected DuplicateFeatureId, got {other:?}"),
        }
    }

    /// Edge: feature_id with invalid characters rejected on load.
    #[test]
    fn test_invalid_chars_in_feature_id_rejected() {
        let yaml = "\
schema_version: 1
version: 0.1.0
root_component:
  name: Test
  features:
    - type: create_box
      id: 'box;1'
      width: 10.0
      height: 20.0
      depth: 30.0
";
        let result = Document::from_yaml(yaml);
        assert!(matches!(result, Err(FormatError::InvalidName { .. })));
    }

    /// T15: ComponentRef::StdLib("") rejected by validate.
    #[test]
    fn t15_stdlib_empty_path_rejected_by_validate() {
        let mut doc = Document::new("Test");
        doc.root_component.reference = Some(crate::component::ComponentRef::StdLib("".into()));
        let err = doc.to_yaml().unwrap_err();
        assert!(matches!(err, FormatError::InvalidReference { .. }));
    }

    /// T16: ComponentRef::File("") rejected by validate.
    #[test]
    fn t16_file_empty_path_rejected_by_validate() {
        let mut doc = Document::new("Test");
        doc.root_component.reference = Some(crate::component::ComponentRef::File("".into()));
        let err = doc.to_yaml().unwrap_err();
        assert!(matches!(err, FormatError::InvalidReference { .. }));
    }

    // --- Edge-case tests for ComponentRef validation (adversarial persona) ---

    /// Empty StdLib ref in child component also rejected.
    #[test]
    fn edge_stdlib_empty_in_child_rejected() {
        let mut doc = Document::new("Root");
        let mut child = Component::new("Child");
        child.reference = Some(crate::component::ComponentRef::StdLib("".into()));
        doc.root_component.children.push(child);
        let err = doc.to_yaml().unwrap_err();
        assert!(matches!(err, FormatError::InvalidReference { .. }));
    }

    /// Empty File ref in deeply nested child rejected.
    #[test]
    fn edge_file_empty_in_deeply_nested_child_rejected() {
        let mut doc = Document::new("Root");
        let mut inner = Component::new("Inner");
        inner.reference = Some(crate::component::ComponentRef::File("".into()));
        let mut outer = Component::new("Outer");
        outer.children.push(inner);
        doc.root_component.children.push(outer);
        let err = doc.to_yaml().unwrap_err();
        assert!(matches!(err, FormatError::InvalidReference { .. }));
    }

    /// Valid StdLib ref passes validation.
    #[test]
    fn edge_valid_stdlib_ref_passes() {
        let mut doc = Document::new("Test");
        doc.root_component.reference = Some(crate::component::ComponentRef::StdLib(
            "fasteners/M5".into(),
        ));
        assert!(doc.validate().is_ok());
    }

    /// Valid File ref passes validation.
    #[test]
    fn edge_valid_file_ref_passes() {
        let mut doc = Document::new("Test");
        doc.root_component.reference = Some(crate::component::ComponentRef::File(
            "./motor.engawa".into(),
        ));
        assert!(doc.validate().is_ok());
    }

    /// No reference (None) passes validation.
    #[test]
    fn edge_no_reference_passes() {
        let doc = Document::new("Test");
        assert!(doc.validate().is_ok());
    }

    /// StdLib empty path produces correct error message.
    #[test]
    fn edge_stdlib_empty_error_message() {
        let mut doc = Document::new("Test");
        doc.root_component.reference = Some(crate::component::ComponentRef::StdLib("".into()));
        match doc.to_yaml().unwrap_err() {
            FormatError::InvalidReference { value, reason } => {
                assert_eq!(value, "stdlib://");
                assert!(reason.contains("non-empty path"));
            }
            other => panic!("expected InvalidReference, got {other:?}"),
        }
    }

    /// File empty path produces correct error message.
    #[test]
    fn edge_file_empty_error_message() {
        let mut doc = Document::new("Test");
        doc.root_component.reference = Some(crate::component::ComponentRef::File("".into()));
        match doc.to_yaml().unwrap_err() {
            FormatError::InvalidReference { value, reason } => {
                assert_eq!(value, "");
                assert!(reason.contains("must not be empty"));
            }
            other => panic!("expected InvalidReference, got {other:?}"),
        }
    }

    /// StdLib ref with valid feature_id in same component passes.
    #[test]
    fn edge_stdlib_with_features_passes() {
        use crate::feature::Feature;
        let mut doc = Document::new("Test");
        doc.root_component.reference =
            Some(crate::component::ComponentRef::StdLib("parts/screw".into()));
        doc.root_component.features.push(Feature::CreateBox {
            id: "box_1".to_string(),
            width: 1.0,
            height: 2.0,
            depth: 3.0,
        });
        assert!(doc.validate().is_ok());
    }

    /// Multiple children, one with empty ref — only the bad one is caught.
    #[test]
    fn edge_mixed_children_one_bad_ref() {
        let mut doc = Document::new("Root");
        let good_child = Component::from_ref("Good", "stdlib://parts/a").unwrap();
        let mut bad_child = Component::new("Bad");
        bad_child.reference = Some(crate::component::ComponentRef::File("".into()));
        doc.root_component.children.push(good_child);
        doc.root_component.children.push(bad_child);
        let err = doc.to_yaml().unwrap_err();
        assert!(matches!(err, FormatError::InvalidReference { .. }));
    }

    /// Determinism: same invalid doc always produces InvalidReference.
    #[test]
    fn edge_deterministic_empty_ref_error() {
        for _ in 0..100 {
            let mut doc = Document::new("Test");
            doc.root_component.reference = Some(crate::component::ComponentRef::StdLib("".into()));
            assert!(matches!(
                doc.to_yaml().unwrap_err(),
                FormatError::InvalidReference { .. }
            ));
        }
    }

    /// T17: ComponentRef::File("stdlib://...") rejected by validate.
    #[test]
    fn t17_file_with_stdlib_prefix_rejected() {
        let mut doc = Document::new("Test");
        doc.root_component.reference =
            Some(crate::component::ComponentRef::File("stdlib://foo".into()));
        assert!(matches!(
            doc.to_yaml().unwrap_err(),
            FormatError::InvalidReference { .. }
        ));
    }

    // --- Adversarial edge-case tests (F01) ---

    /// File("stdlib://") with empty path after prefix rejected.
    #[test]
    fn edge_file_stdlib_prefix_empty_path_rejected() {
        let mut doc = Document::new("Test");
        doc.root_component.reference =
            Some(crate::component::ComponentRef::File("stdlib://".into()));
        let err = doc.to_yaml().unwrap_err();
        assert!(matches!(err, FormatError::InvalidReference { .. }));
    }

    /// File("stdlib://x/y/z") with multi-segment stdlib prefix rejected.
    #[test]
    fn edge_file_stdlib_prefix_multi_segment_rejected() {
        let mut doc = Document::new("Test");
        doc.root_component.reference = Some(crate::component::ComponentRef::File(
            "stdlib://a/b/c".into(),
        ));
        assert!(matches!(
            doc.to_yaml().unwrap_err(),
            FormatError::InvalidReference { .. }
        ));
    }

    /// File("stdlib://...") error message contains correct reason.
    #[test]
    fn edge_file_stdlib_prefix_error_message() {
        let mut doc = Document::new("Test");
        doc.root_component.reference =
            Some(crate::component::ComponentRef::File("stdlib://foo".into()));
        match doc.to_yaml().unwrap_err() {
            FormatError::InvalidReference { value, reason } => {
                assert_eq!(value, "stdlib://foo");
                assert!(reason.contains("must not start with 'stdlib://'"));
            }
            other => panic!("expected InvalidReference, got {other:?}"),
        }
    }

    /// File("stdlib://...") in child component also rejected.
    #[test]
    fn edge_file_stdlib_prefix_in_child_rejected() {
        let mut doc = Document::new("Root");
        let mut child = Component::new("Child");
        child.reference = Some(crate::component::ComponentRef::File(
            "stdlib://screw".into(),
        ));
        doc.root_component.children.push(child);
        assert!(matches!(
            doc.to_yaml().unwrap_err(),
            FormatError::InvalidReference { .. }
        ));
    }

    /// File("STDLIB://...") (uppercase) NOT rejected — case-sensitive.
    #[test]
    fn edge_file_uppercase_stdlib_prefix_allowed() {
        let mut doc = Document::new("Test");
        doc.root_component.reference =
            Some(crate::component::ComponentRef::File("STDLIB://foo".into()));
        assert!(doc.validate().is_ok());
    }

    /// File("./stdlib://fake") — contains but doesn't start with — allowed.
    #[test]
    fn edge_file_stdlib_midstring_allowed() {
        let mut doc = Document::new("Test");
        doc.root_component.reference = Some(crate::component::ComponentRef::File(
            "./stdlib://fake".into(),
        ));
        assert!(doc.validate().is_ok());
    }

    /// Deserialize rejects File("stdlib://...") via YAML.
    #[test]
    fn edge_deserialize_file_stdlib_prefix_rejected() {
        let yaml = "\
schema_version: 1
version: 0.1.0
root_component:
  name: Test
  ref: \"stdlib://foo\"
  features: []
";
        // from_str parses "stdlib://foo" as StdLib("foo") — this is valid
        let doc = Document::from_yaml(yaml).unwrap();
        match doc.root_component.reference {
            Some(crate::component::ComponentRef::StdLib(ref p)) => assert_eq!(p, "foo"),
            other => panic!("expected StdLib, got {other:?}"),
        }
    }

    /// Determinism: File("stdlib://x") rejected 100 times consistently.
    #[test]
    fn edge_file_stdlib_prefix_deterministic_100() {
        for _ in 0..100 {
            let mut doc = Document::new("Test");
            doc.root_component.reference =
                Some(crate::component::ComponentRef::File("stdlib://x".into()));
            assert!(matches!(
                doc.to_yaml().unwrap_err(),
                FormatError::InvalidReference { .. }
            ));
        }
    }

    /// Edge: from_path also validates feature_ids.
    #[test]
    fn test_from_path_validates_feature_ids() {
        // All example files have valid IDs — this just ensures validation runs on from_path
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join("..")
            .join("examples")
            .join("simple_box.engawa");
        let doc = Document::from_path(&path).unwrap();
        assert!(doc.validate().is_ok());
    }

    // --- Position parameter validation tests (Issue #48) ---

    /// NaN in cylinder origin rejected with InvalidPosition.
    #[test]
    fn test_nan_cylinder_origin_rejected() {
        let yaml = "\
schema_version: 1
version: 0.1.0
root_component:
  name: test
  features:
    - type: create_cylinder
      id: c1
      radius: 5.0
      height: 10.0
      origin: [.nan, 0.0, 0.0]
";
        let result = Document::from_yaml(yaml);
        assert!(
            matches!(result, Err(FormatError::InvalidPosition { .. })),
            "expected InvalidPosition, got {result:?}"
        );
    }

    /// Infinity in cylinder origin rejected with InvalidPosition.
    #[test]
    fn test_inf_cylinder_origin_rejected() {
        let yaml = "\
schema_version: 1
version: 0.1.0
root_component:
  name: test
  features:
    - type: create_cylinder
      id: c1
      radius: 5.0
      height: 10.0
      origin: [0.0, .inf, 0.0]
";
        let result = Document::from_yaml(yaml);
        assert!(
            matches!(result, Err(FormatError::InvalidPosition { .. })),
            "expected InvalidPosition, got {result:?}"
        );
    }

    /// NaN in sphere center rejected with InvalidPosition.
    #[test]
    fn test_nan_sphere_center_rejected() {
        let yaml = "\
schema_version: 1
version: 0.1.0
root_component:
  name: test
  features:
    - type: create_sphere
      id: s1
      radius: 5.0
      center: [0.0, 0.0, .nan]
";
        let result = Document::from_yaml(yaml);
        assert!(
            matches!(result, Err(FormatError::InvalidPosition { .. })),
            "expected InvalidPosition, got {result:?}"
        );
    }

    /// Negative infinity in sphere center rejected.
    #[test]
    fn test_neg_inf_sphere_center_rejected() {
        let yaml = "\
schema_version: 1
version: 0.1.0
root_component:
  name: test
  features:
    - type: create_sphere
      id: s1
      radius: 5.0
      center: [-.inf, 0.0, 0.0]
";
        let result = Document::from_yaml(yaml);
        assert!(
            matches!(result, Err(FormatError::InvalidPosition { .. })),
            "expected InvalidPosition, got {result:?}"
        );
    }

    /// Valid large finite coordinates pass validation.
    #[test]
    fn test_large_finite_origin_accepted() {
        let yaml = "\
schema_version: 1
version: 0.1.0
root_component:
  name: test
  features:
    - type: create_cylinder
      id: c1
      radius: 5.0
      height: 10.0
      origin: [1e15, -1e15, 0.0]
";
        let doc = Document::from_yaml(yaml).expect("large finite values should be accepted");
        assert_eq!(doc.root_component.features.len(), 1);
    }

    /// Negative coordinates are accepted (only NaN/Inf are rejected).
    #[test]
    fn test_negative_origin_accepted() {
        let yaml = "\
schema_version: 1
version: 0.1.0
root_component:
  name: test
  features:
    - type: create_sphere
      id: s1
      radius: 5.0
      center: [-100.0, -50.0, -200.0]
";
        let doc = Document::from_yaml(yaml).expect("negative coordinates should be accepted");
        assert_eq!(doc.root_component.features.len(), 1);
    }

    /// Omitted origin/center defaults to [0,0,0] and passes validation.
    #[test]
    fn test_omitted_origin_defaults_and_validates() {
        let yaml = "\
schema_version: 1
version: 0.1.0
root_component:
  name: test
  features:
    - type: create_cylinder
      id: c1
      radius: 5.0
      height: 10.0
";
        let doc = Document::from_yaml(yaml).expect("omitted origin should default and pass");
        match &doc.root_component.features[0] {
            Feature::CreateCylinder { origin, .. } => {
                assert_eq!(*origin, [0.0, 0.0, 0.0]);
            }
            other => panic!("expected CreateCylinder, got {other:?}"),
        }
    }

    /// Determinism: NaN rejection is consistent over 100 attempts.
    #[test]
    fn test_nan_rejection_deterministic_100() {
        for _ in 0..100 {
            let mut doc = Document::new("Test");
            doc.root_component.features.push(Feature::CreateCylinder {
                id: "c".into(),
                radius: 5.0,
                height: 10.0,
                origin: [f64::NAN, 0.0, 0.0],
            });
            assert!(matches!(
                doc.to_yaml().unwrap_err(),
                FormatError::InvalidPosition { .. }
            ));
        }
    }
}
