//! #256 Phase 9: engawa entry edit CLI integration test (T03)

use std::fs;
use std::process::Command;
use tempfile::TempDir;

/// T03: CLI edit — engawa entry edit updates feature params in output file.
#[test]
fn t03_cli_edit_updates_feature() {
    let temp_dir = TempDir::new().unwrap();
    let input_path = temp_dir.path().join("input.engawa");
    let feature_path = temp_dir.path().join("new_feature.yaml");
    let output_path = temp_dir.path().join("output.engawa");

    // Create initial document with CreateBox
    let initial_yaml = r#"version: "0.1.0"
root_component:
  name: Test
  features:
    - type: create_box
      id: box_1
      width: 10.0
      height: 20.0
      depth: 30.0
"#;
    fs::write(&input_path, initial_yaml).unwrap();

    // Create new feature YAML with updated params
    let new_feature_yaml = r#"type: create_box
id: box_1
width: 100.0
height: 200.0
depth: 300.0
"#;
    fs::write(&feature_path, new_feature_yaml).unwrap();

    // Run engawa entry edit
    let output = Command::new(env!("CARGO_BIN_EXE_engawa"))
        .arg("entry")
        .arg("edit")
        .arg(&input_path)
        .arg("box_1")
        .arg(&feature_path)
        .arg("--output")
        .arg(&output_path)
        .output()
        .expect("failed to execute engawa entry edit");

    // Verify command succeeded
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    // Read output document and verify box_1 params updated
    let output_yaml = fs::read_to_string(&output_path).unwrap();
    let output_doc: engawa_format::Document =
        serde_yaml::from_str(&output_yaml).expect("failed to parse output YAML");

    assert_eq!(output_doc.root_component.features.len(), 1);
    match &output_doc.root_component.features[0] {
        engawa_format::Feature::CreateBox {
            id,
            width,
            height,
            depth,
        } => {
            assert_eq!(id, "box_1");
            assert_eq!(*width, 100.0);
            assert_eq!(*height, 200.0);
            assert_eq!(*depth, 300.0);
        }
        _ => panic!("expected CreateBox"),
    }

    // Byte-equal check: Parse input as Document, then replace the edited feature in-place.
    let mut expected_doc: engawa_format::Document =
        serde_yaml::from_str(&initial_yaml).expect("failed to parse initial");
    expected_doc.root_component.features[0] = engawa_format::Feature::CreateBox {
        id: "box_1".to_string(),
        width: 100.0,
        height: 200.0,
        depth: 300.0,
    };
    let expected_yaml = expected_doc.to_yaml().unwrap();
    assert_eq!(output_yaml, expected_yaml, "output YAML matches expected");
}

/// T03 dry-run: --dry-run emits to stdout.
#[test]
fn t03_cli_edit_dry_run() {
    let temp_dir = TempDir::new().unwrap();
    let input_path = temp_dir.path().join("input.engawa");
    let feature_path = temp_dir.path().join("new_feature.yaml");

    let initial_yaml = r#"version: "0.1.0"
root_component:
  name: Test
  features:
    - type: create_box
      id: box_1
      width: 10.0
      height: 20.0
      depth: 30.0
"#;
    fs::write(&input_path, initial_yaml).unwrap();

    let new_feature_yaml = r#"type: create_box
id: box_1
width: 100.0
height: 200.0
depth: 300.0
"#;
    fs::write(&feature_path, new_feature_yaml).unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_engawa"))
        .arg("entry")
        .arg("edit")
        .arg(&input_path)
        .arg("box_1")
        .arg(&feature_path)
        .arg("--dry-run")
        .output()
        .expect("failed to execute engawa entry edit --dry-run");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8(output.stdout).expect("stdout not utf-8");
    let output_doc: engawa_format::Document =
        serde_yaml::from_str(&stdout).expect("failed to parse stdout YAML");

    match &output_doc.root_component.features[0] {
        engawa_format::Feature::CreateBox {
            id,
            width,
            height,
            depth,
        } => {
            assert_eq!(id, "box_1");
            assert_eq!(*width, 100.0);
            assert_eq!(*height, 200.0);
            assert_eq!(*depth, 300.0);
        }
        _ => panic!("expected CreateBox"),
    }
}
