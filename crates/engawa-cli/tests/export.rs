use std::path::PathBuf;
use std::process::Command;

#[test]
fn export_simple_box_produces_12_facets() {
    let bin = env!("CARGO_BIN_EXE_engawa");
    let input = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("examples")
        .join("simple_box.engawa");
    let tmp = tempfile::NamedTempFile::with_suffix(".stl").expect("tempfile");
    let output = tmp.path();

    let status = Command::new(bin)
        .arg("export")
        .arg(&input)
        .arg("-o")
        .arg(output)
        .status()
        .expect("run engawa export");
    assert!(status.success(), "export should succeed");

    let stl = std::fs::read_to_string(output).expect("read stl");
    let facet_count = stl.matches("facet normal").count();
    assert_eq!(
        facet_count, 12,
        "cuboid should produce 12 facets (6 faces * 2 triangles)"
    );
}

/// T14: E2E — sphere export → STL with 960 facets.
#[test]
fn export_sphere_produces_960_facets() {
    let bin = env!("CARGO_BIN_EXE_engawa");
    let input = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("examples")
        .join("sphere.engawa");
    let tmp = tempfile::NamedTempFile::with_suffix(".stl").expect("tempfile");
    let output = tmp.path();

    let status = Command::new(bin)
        .arg("export")
        .arg(&input)
        .arg("-o")
        .arg(output)
        .status()
        .expect("run engawa export");
    assert!(status.success(), "export should succeed");

    let stl = std::fs::read_to_string(output).expect("read stl");
    let facet_count = stl.matches("facet normal").count();
    assert_eq!(
        facet_count, 960,
        "sphere should produce 960 facets (2*32*(16-1))"
    );
}

/// T12: E2E — extruded_rect export → STL with 12 facets.
#[test]
fn export_extruded_rect_produces_12_facets() {
    let bin = env!("CARGO_BIN_EXE_engawa");
    let input = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("examples")
        .join("extruded_rect.engawa");
    let tmp = tempfile::NamedTempFile::with_suffix(".stl").expect("tempfile");
    let output = tmp.path();

    let status = Command::new(bin)
        .arg("export")
        .arg(&input)
        .arg("-o")
        .arg(output)
        .status()
        .expect("run engawa export");
    assert!(status.success(), "export should succeed");

    let stl = std::fs::read_to_string(output).expect("read stl");
    let facet_count = stl.matches("facet normal").count();
    assert_eq!(
        facet_count, 12,
        "extruded rect should produce 12 facets (6 faces * 2 triangles)"
    );
}

/// T13: E2E — self-intersecting profile → export fails, no STL written.
#[test]
fn export_invalid_profile_fails_no_stl() {
    let bin = env!("CARGO_BIN_EXE_engawa");
    let input = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join("invalid_profile.engawa");
    let tmp = tempfile::NamedTempFile::with_suffix(".stl").expect("tempfile");
    let output = tmp.path();

    let result = Command::new(bin)
        .arg("export")
        .arg(&input)
        .arg("-o")
        .arg(output)
        .output()
        .expect("run engawa export");
    assert!(
        !result.status.success(),
        "export should fail for self-intersecting profile"
    );

    // Stderr must contain the expected error message
    // (regression guard for KernelError::InvalidParameter { kind: "profile" })
    let stderr = String::from_utf8_lossy(&result.stderr);
    assert!(
        stderr.contains("invalid parameter: profile"),
        "stderr should contain expected error, got: {stderr}"
    );

    // Output file should be absent or empty
    let written = std::fs::metadata(output).map(|m| m.len()).unwrap_or(0);
    assert_eq!(written, 0, "no STL should be written for invalid profile");
}

/// T16: CLI export of assembly document now succeeds (assembly guard removed).
#[test]
fn t16_export_assembly_succeeds() {
    let bin = env!("CARGO_BIN_EXE_engawa");
    let input = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("examples")
        .join("assembly.engawa");
    let tmp = tempfile::NamedTempFile::with_suffix(".stl").expect("tempfile");

    let output = Command::new(bin)
        .arg("export")
        .arg(&input)
        .arg("-o")
        .arg(tmp.path())
        .output()
        .expect("run engawa export");
    assert!(output.status.success(), "export of assembly must succeed");
    let stl = std::fs::read(tmp.path()).expect("read stl");
    assert!(!stl.is_empty(), "STL output must not be empty");
}
