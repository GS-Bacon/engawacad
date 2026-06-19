//! #258 Phase 9: engawa entry suppress/restore CLI integration test (T04/T05)

use std::fs;
use std::process::Command;
use tempfile::TempDir;

/// T04: CLI suppress — engawa entry suppress sets suppressed: true in output file.
#[test]
fn t04_cli_suppress_sets_flag() {
    let temp_dir = TempDir::new().unwrap();
    let input_path = temp_dir.path().join("input.engawa");
    let output_path = temp_dir.path().join("output.engawa");

    // Create initial document with CreateBox (no suppressed field)
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

    // Run engawa entry suppress
    let output = Command::new(env!("CARGO_BIN_EXE_engawa"))
        .arg("entry")
        .arg("suppress")
        .arg(&input_path)
        .arg("box_1")
        .arg("--output")
        .arg(&output_path)
        .output()
        .expect("failed to execute engawa entry suppress");

    // Verify command succeeded
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    // Read output document and verify suppressed: true
    let output_yaml = fs::read_to_string(&output_path).unwrap();
    let output_doc: engawa_format::Document =
        serde_yaml::from_str(&output_yaml).expect("failed to parse output YAML");

    assert_eq!(output_doc.root_component.features.len(), 1);
    match &output_doc.root_component.features[0] {
        engawa_format::Feature::CreateBox { id, suppressed, .. } => {
            assert_eq!(id, "box_1");
            assert!(*suppressed, "feature should be suppressed");
        }
        _ => panic!("expected CreateBox"),
    }

    // Byte-equal check: construct expected Document with suppressed: true
    let mut expected_doc: engawa_format::Document =
        serde_yaml::from_str(&initial_yaml).expect("failed to parse initial");
    expected_doc.root_component.features[0] = engawa_format::Feature::CreateBox {
        id: "box_1".to_string(),
        width: 10.0,
        height: 20.0,
        depth: 30.0,
        suppressed: true,
    };
    let expected_yaml = expected_doc.to_yaml().unwrap();
    assert_eq!(output_yaml, expected_yaml, "output YAML matches expected");
}

/// T04 dry-run: --dry-run emits to stdout with suppressed: true.
#[test]
fn t04_cli_suppress_dry_run() {
    let temp_dir = TempDir::new().unwrap();
    let input_path = temp_dir.path().join("input.engawa");

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

    let output = Command::new(env!("CARGO_BIN_EXE_engawa"))
        .arg("entry")
        .arg("suppress")
        .arg(&input_path)
        .arg("box_1")
        .arg("--dry-run")
        .output()
        .expect("failed to execute engawa entry suppress --dry-run");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8(output.stdout).expect("stdout not utf-8");
    let output_doc: engawa_format::Document =
        serde_yaml::from_str(&stdout).expect("failed to parse stdout YAML");

    match &output_doc.root_component.features[0] {
        engawa_format::Feature::CreateBox { id, suppressed, .. } => {
            assert_eq!(id, "box_1");
            assert!(*suppressed, "feature should be suppressed");
        }
        _ => panic!("expected CreateBox"),
    }
}

/// T05: CLI restore — engawa entry restore clears suppressed flag (no suppressed line in YAML).
#[test]
fn t05_cli_restore_clears_flag() {
    let temp_dir = TempDir::new().unwrap();
    let input_path = temp_dir.path().join("input.engawa");
    let output_path = temp_dir.path().join("output.engawa");

    // Create initial document with suppressed: true
    let initial_yaml = r#"version: "0.1.0"
root_component:
  name: Test
  features:
    - type: create_box
      id: box_1
      width: 10.0
      height: 20.0
      depth: 30.0
      suppressed: true
"#;
    fs::write(&input_path, initial_yaml).unwrap();

    // Run engawa entry restore
    let output = Command::new(env!("CARGO_BIN_EXE_engawa"))
        .arg("entry")
        .arg("restore")
        .arg(&input_path)
        .arg("box_1")
        .arg("--output")
        .arg(&output_path)
        .output()
        .expect("failed to execute engawa entry restore");

    // Verify command succeeded
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    // Read output document and verify suppressed: false (no suppressed line in YAML)
    let output_yaml = fs::read_to_string(&output_path).unwrap();
    let output_doc: engawa_format::Document =
        serde_yaml::from_str(&output_yaml).expect("failed to parse output YAML");

    assert_eq!(output_doc.root_component.features.len(), 1);
    match &output_doc.root_component.features[0] {
        engawa_format::Feature::CreateBox { id, suppressed, .. } => {
            assert_eq!(id, "box_1");
            assert!(!*suppressed, "feature should not be suppressed");
        }
        _ => panic!("expected CreateBox"),
    }

    // Byte-equal check: construct expected Document with suppressed: false
    // (which serializes without the suppressed line)
    let mut expected_doc: engawa_format::Document =
        serde_yaml::from_str(&initial_yaml).expect("failed to parse initial");
    expected_doc.root_component.features[0] = engawa_format::Feature::CreateBox {
        id: "box_1".to_string(),
        width: 10.0,
        height: 20.0,
        depth: 30.0,
        suppressed: false,
    };
    let expected_yaml = expected_doc.to_yaml().unwrap();
    assert_eq!(output_yaml, expected_yaml, "output YAML matches expected");
}

/// T05 dry-run: --dry-run emits to stdout without suppressed line.
#[test]
fn t05_cli_restore_dry_run() {
    let temp_dir = TempDir::new().unwrap();
    let input_path = temp_dir.path().join("input.engawa");

    let initial_yaml = r#"version: "0.1.0"
root_component:
  name: Test
  features:
    - type: create_box
      id: box_1
      width: 10.0
      height: 20.0
      depth: 30.0
      suppressed: true
"#;
    fs::write(&input_path, initial_yaml).unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_engawa"))
        .arg("entry")
        .arg("restore")
        .arg(&input_path)
        .arg("box_1")
        .arg("--dry-run")
        .output()
        .expect("failed to execute engawa entry restore --dry-run");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8(output.stdout).expect("stdout not utf-8");
    let output_doc: engawa_format::Document =
        serde_yaml::from_str(&stdout).expect("failed to parse stdout YAML");

    match &output_doc.root_component.features[0] {
        engawa_format::Feature::CreateBox { id, suppressed, .. } => {
            assert_eq!(id, "box_1");
            assert!(!*suppressed, "feature should not be suppressed");
        }
        _ => panic!("expected CreateBox"),
    }

    // Verify "suppressed:" does not appear in YAML (since it's false)
    assert!(
        !stdout.contains("suppressed:"),
        "suppressed line should not appear when false"
    );
}
