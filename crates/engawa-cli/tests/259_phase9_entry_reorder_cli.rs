//! #259 Phase 9: engawa entry reorder CLI integration test (T03)

use std::fs;
use std::process::Command;
use tempfile::TempDir;

/// T03: CLI reorder — engawa entry reorder moves feature before another.
#[test]
fn t03_cli_reorder_moves_feature() {
    let temp_dir = TempDir::new().unwrap();
    let input_path = temp_dir.path().join("input.engawa");
    let output_path = temp_dir.path().join("output.engawa");

    // Create initial document with [box_1, sphere_1, cyl_1]
    let initial_yaml = r#"version: "0.1.0"
root_component:
  name: Test
  features:
    - type: create_box
      id: box_1
      width: 10.0
      height: 20.0
      depth: 30.0
    - type: create_sphere
      id: sphere_1
      radius: 5.0
      center: [0.0, 0.0, 0.0]
    - type: create_cylinder
      id: cyl_1
      radius: 8.0
      height: 40.0
      origin: [0.0, 0.0, 0.0]
"#;
    fs::write(&input_path, initial_yaml).unwrap();

    // Run engawa entry reorder: move cyl_1 before sphere_1
    let output = Command::new(env!("CARGO_BIN_EXE_engawa"))
        .arg("entry")
        .arg("reorder")
        .arg(&input_path)
        .arg("cyl_1")
        .arg("--before")
        .arg("sphere_1")
        .arg("--output")
        .arg(&output_path)
        .output()
        .expect("failed to execute engawa entry reorder");

    // Verify command succeeded
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    // Read output document and verify order: [box_1, cyl_1, sphere_1]
    let output_yaml = fs::read_to_string(&output_path).unwrap();
    let output_doc: engawa_format::Document =
        serde_yaml::from_str(&output_yaml).expect("failed to parse output YAML");

    assert_eq!(output_doc.root_component.features.len(), 3);
    assert_eq!(output_doc.root_component.features[0].id(), "box_1");
    assert_eq!(output_doc.root_component.features[1].id(), "cyl_1");
    assert_eq!(output_doc.root_component.features[2].id(), "sphere_1");

    // Byte-equal check: expected YAML with reordered features
    let expected_yaml = r#"schema_version: 1
version: 0.1.0
root_component:
  name: Test
  features:
  - type: create_box
    id: box_1
    width: 10.0
    height: 20.0
    depth: 30.0
  - type: create_cylinder
    id: cyl_1
    radius: 8.0
    height: 40.0
  - type: create_sphere
    id: sphere_1
    radius: 5.0
"#;
    assert_eq!(output_yaml, expected_yaml, "output YAML matches expected");
}

/// T03 dry-run: --dry-run emits to stdout.
#[test]
fn t03_cli_reorder_dry_run() {
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
    - type: create_sphere
      id: sphere_1
      radius: 5.0
      center: [0.0, 0.0, 0.0]
    - type: create_cylinder
      id: cyl_1
      radius: 8.0
      height: 40.0
      origin: [0.0, 0.0, 0.0]
"#;
    fs::write(&input_path, initial_yaml).unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_engawa"))
        .arg("entry")
        .arg("reorder")
        .arg(&input_path)
        .arg("cyl_1")
        .arg("--before")
        .arg("sphere_1")
        .arg("--dry-run")
        .output()
        .expect("failed to execute engawa entry reorder --dry-run");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8(output.stdout).expect("stdout not utf-8");
    let output_doc: engawa_format::Document =
        serde_yaml::from_str(&stdout).expect("failed to parse stdout YAML");

    assert_eq!(output_doc.root_component.features.len(), 3);
    assert_eq!(output_doc.root_component.features[0].id(), "box_1");
    assert_eq!(output_doc.root_component.features[1].id(), "cyl_1");
    assert_eq!(output_doc.root_component.features[2].id(), "sphere_1");
}
