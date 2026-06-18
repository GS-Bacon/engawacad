//! Acceptance tests for #239: schema_version field + MigrationHook trait entry.
//!
//! テスト ID は features/239-phase9-schema-version-migration/plan.md と対応。

use engawa_format::document::CURRENT_SCHEMA_VERSION;
use engawa_format::feature::Feature;
use engawa_format::{Document, FormatError, MigrationHook};

#[test]
fn t01_determinism_from_yaml() {
    let yaml = "\
schema_version: 1
version: 0.1.0
root_component:
  name: Test
  features:
    - type: create_box
      id: box_1
      width: 10.0
      height: 20.0
      depth: 30.0
";
    let doc1 = Document::from_yaml(yaml).unwrap();
    let doc2 = Document::from_yaml(yaml).unwrap();
    // Deep equality via canonical YAML serialization.
    let yaml1 = doc1.to_yaml().expect("doc1 to_yaml");
    let yaml2 = doc2.to_yaml().expect("doc2 to_yaml");
    assert_eq!(yaml1, yaml2, "from_yaml must produce identical Documents");
}

#[test]
fn t_deg_unknown_version_99() {
    let yaml = "\
schema_version: 99
version: 0.1.0
root_component:
  name: Future
  features: []
";
    let result = Document::from_yaml(yaml);
    match result {
        Err(FormatError::UnknownSchemaVersion { found, current }) => {
            assert_eq!(found, 99);
            assert_eq!(current, CURRENT_SCHEMA_VERSION);
        }
        other => panic!("expected UnknownSchemaVersion, got {:?}", other),
    }
}

#[test]
fn t_boundary_current_version() {
    let yaml = format!(
        "\
schema_version: {current}
version: 0.1.0
root_component:
  name: Current
  features: []
",
        current = CURRENT_SCHEMA_VERSION
    );
    let doc = Document::from_yaml(&yaml).expect("current version should parse");
    assert_eq!(doc.schema_version, CURRENT_SCHEMA_VERSION);
}

#[test]
fn t_deg_max_u32_version() {
    let yaml = format!(
        "\
schema_version: {max}
version: 0.1.0
root_component:
  name: Max
  features: []
",
        max = u32::MAX
    );
    let result = Document::from_yaml(&yaml);
    match result {
        Err(FormatError::UnknownSchemaVersion { found, current }) => {
            assert_eq!(found, u32::MAX);
            assert_eq!(current, CURRENT_SCHEMA_VERSION);
        }
        other => panic!("expected UnknownSchemaVersion, got {:?}", other),
    }
}

#[test]
fn t_trait_migration_hook_signature() {
    // Dummy implementation for compile-time verification
    struct DummyMigration;

    impl MigrationHook for DummyMigration {
        fn migrate(&self, _from: u32, _to: u32, _doc: &mut Document) -> Result<(), FormatError> {
            Ok(())
        }
    }

    let hook = DummyMigration;

    // Verify migrate can be called on a real Document
    let mut doc = Document::new("test");
    doc.root_component.features.push(Feature::CreateBox {
        id: "box_1".to_string(),
        width: 10.0,
        height: 20.0,
        depth: 30.0,
    });
    assert!(hook.migrate(1, 2, &mut doc).is_ok());
}

/// T_FUTURE_unknown_feature_type: schema_version > CURRENT で、現行 Component が
/// deserialize できない future Feature.type を含むペイロードでも、
/// UnknownSchemaVersion で reject されること (2-stage 経路の回帰テスト)。
#[test]
fn t_future_unknown_feature_type_reject() {
    let yaml = "\
schema_version: 99
version: 0.1.0
root_component:
  name: Future
  features:
    - type: create_hyperspace_warp
      id: hw_1
      foo: 42
      bar: 7.0
";
    let result = Document::from_yaml(yaml);
    match result {
        Err(FormatError::UnknownSchemaVersion { found, current }) => {
            assert_eq!(found, 99);
            assert_eq!(current, CURRENT_SCHEMA_VERSION);
        }
        other => panic!("expected UnknownSchemaVersion, got {:?}", other),
    }
}
