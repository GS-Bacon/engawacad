//! Issue #158: format-refplane acceptance tests (format layer)
//! STEP 6.6 実装完了

use engawa_format::{Document, FormatError, SketchPlane};

#[test]
fn t02_document_new_seeds_three_refplanes() {
    let doc = Document::new("test");
    let ref_planes = &doc.root_component.ref_planes;

    assert_eq!(ref_planes.len(), 3);
    assert_eq!(ref_planes[0].id, "Front");
    assert_eq!(ref_planes[0].plane, SketchPlane::Xy);
    assert_eq!(ref_planes[0].offset, 0.0);
    assert_eq!(ref_planes[1].id, "Top");
    assert_eq!(ref_planes[1].plane, SketchPlane::Xz);
    assert_eq!(ref_planes[1].offset, 0.0);
    assert_eq!(ref_planes[2].id, "Right");
    assert_eq!(ref_planes[2].plane, SketchPlane::Yz);
    assert_eq!(ref_planes[2].offset, 0.0);
}

#[test]
fn t03_from_yaml_seeds_three_refplanes() {
    let yaml = "schema_version: 1\nversion: '0.1.0'\nroot_component:\n  name: x\n";
    let doc = Document::from_yaml(yaml).unwrap();
    let ref_planes = &doc.root_component.ref_planes;

    assert_eq!(ref_planes.len(), 3);
    assert_eq!(ref_planes[0].id, "Front");
    assert_eq!(ref_planes[1].id, "Top");
    assert_eq!(ref_planes[2].id, "Right");
}

#[test]
fn t04_legacy_examples_roundtrip() {
    let manifest_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let workspace_root = manifest_dir.ancestors().nth(2).unwrap();
    let examples = vec![
        workspace_root.join("examples/extruded_rect.engawa"),
        workspace_root.join("examples/two_bodies.engawa"),
    ];

    for path in examples {
        let original = std::fs::read_to_string(&path).unwrap();
        let doc1 = Document::from_yaml(&original).unwrap();
        let normalized = doc1.to_yaml().unwrap();
        // #158 Codex F02 r4: 元 YAML は手書きでフォーマットが緩い (空白・引用符等) ため
        // 1 回目の serialize で正規化される。2 度目以降は byte-identical で安定する
        // (= wire-format idempotency)。崩れがあると idempotency が失われる。
        let doc2 = Document::from_yaml(&normalized).unwrap();
        let normalized2 = doc2.to_yaml().unwrap();
        assert_eq!(
            normalized,
            normalized2,
            "{} normalized round-trip must be idempotent (wire-format 不変)",
            path.display()
        );
        // 加えて構造的等価性も確認
        assert_eq!(doc1.schema_version, doc2.schema_version);
        assert_eq!(doc1.version, doc2.version);
        assert_eq!(doc1.root_component.name, doc2.root_component.name);
        assert_eq!(
            doc1.root_component.features.len(),
            doc2.root_component.features.len()
        );
        assert_eq!(
            doc1.root_component.ref_planes.len(),
            doc2.root_component.ref_planes.len()
        );
    }
}

#[test]
fn t08_yaml_golden_new_format() {
    let manifest_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let workspace_root = manifest_dir.ancestors().nth(2).unwrap();
    let yaml_path = workspace_root.join("examples/sketch_via_refplane.engawa");
    let yaml = std::fs::read_to_string(yaml_path).unwrap();
    let doc = Document::from_yaml(&yaml).unwrap();
    let reserialized = doc.to_yaml().unwrap();

    // Re-parse and verify content equivalence
    let doc2 = Document::from_yaml(&reserialized).unwrap();
    assert_eq!(doc.schema_version, doc2.schema_version);
    assert_eq!(doc.version, doc2.version);
    assert_eq!(doc.root_component.name, doc2.root_component.name);
    assert_eq!(
        doc.root_component.features.len(),
        doc2.root_component.features.len()
    );
}

#[test]
fn t09_yaml_golden_document_with_explicit_refplanes() {
    let mut doc = Document::new("test");
    // Modify Front offset to 5.0
    doc.root_component.ref_planes[0].offset = 5.0;

    let yaml = doc.to_yaml().unwrap();

    assert!(yaml.contains("ref_planes:"));
    assert!(yaml.contains("offset: 5"));
}

#[test]
fn t17_degen_refplane_offset_not_finite() {
    // NaN offset
    let yaml_nan = r#"schema_version: 1
version: '0.1.0'
root_component:
  name: x
  ref_planes:
  - id: Foo
    plane: xy
    offset: .nan
"#;
    let err = Document::from_yaml(yaml_nan).unwrap_err();
    assert!(matches!(err, FormatError::InvalidRefPlaneOffset { ref id } if id == "Foo"));

    // +Inf offset
    let yaml_inf = r#"schema_version: 1
version: '0.1.0'
root_component:
  name: x
  ref_planes:
  - id: Bar
    plane: xz
    offset: .inf
"#;
    let err = Document::from_yaml(yaml_inf).unwrap_err();
    assert!(matches!(err, FormatError::InvalidRefPlaneOffset { ref id } if id == "Bar"));
}

#[test]
fn t12_degen_duplicate_refplane_id() {
    let yaml = concat!(
        "schema_version: 1\n",
        "version: '0.1.0'\n",
        "root_component:\n",
        "  name: test\n",
        "  ref_planes:\n",
        "  - id: Front\n",
        "    plane: xy\n",
        "  - id: Front\n",
        "    plane: xz\n"
    );

    let result = Document::from_yaml(yaml);
    assert!(result.is_err());
    if let Err(FormatError::DuplicateRefPlaneId { id, .. }) = result {
        assert_eq!(id, "Front");
    } else {
        panic!("Expected FormatError::DuplicateRefPlaneId");
    }
}

#[test]
fn t13_default_three_skips_serialize() {
    let doc = Document::new("test");
    let yaml = doc.to_yaml().unwrap();

    assert!(!yaml.contains("ref_planes"));
}

#[test]
fn t14_explicit_three_with_custom_offset_serializes() {
    let mut doc = Document::new("test");
    doc.root_component.ref_planes[0].offset = 5.0;

    let yaml = doc.to_yaml().unwrap();

    assert!(yaml.contains("ref_planes:"));
    assert!(yaml.contains("offset: 5"));
}
