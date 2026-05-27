use std::path::PathBuf;
use std::process::Command;

#[test]
fn export_simple_box_produces_12_facets() {
    let bin = env!("CARGO_BIN_EXE_mycad");
    let input = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("examples")
        .join("simple_box.mycad");
    let tmp = tempfile::NamedTempFile::with_suffix(".stl").expect("tempfile");
    let output = tmp.path();

    let status = Command::new(bin)
        .arg("export")
        .arg(&input)
        .arg("-o")
        .arg(output)
        .status()
        .expect("run mycad export");
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
    let bin = env!("CARGO_BIN_EXE_mycad");
    let input = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("examples")
        .join("sphere.mycad");
    let tmp = tempfile::NamedTempFile::with_suffix(".stl").expect("tempfile");
    let output = tmp.path();

    let status = Command::new(bin)
        .arg("export")
        .arg(&input)
        .arg("-o")
        .arg(output)
        .status()
        .expect("run mycad export");
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
    let bin = env!("CARGO_BIN_EXE_mycad");
    let input = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("examples")
        .join("extruded_rect.mycad");
    let tmp = tempfile::NamedTempFile::with_suffix(".stl").expect("tempfile");
    let output = tmp.path();

    let status = Command::new(bin)
        .arg("export")
        .arg(&input)
        .arg("-o")
        .arg(output)
        .status()
        .expect("run mycad export");
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
    let bin = env!("CARGO_BIN_EXE_mycad");
    let input = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join("invalid_profile.mycad");
    let tmp = tempfile::NamedTempFile::with_suffix(".stl").expect("tempfile");
    let output = tmp.path();

    let status = Command::new(bin)
        .arg("export")
        .arg(&input)
        .arg("-o")
        .arg(output)
        .status()
        .expect("run mycad export");
    assert!(
        !status.success(),
        "export should fail for self-intersecting profile"
    );

    // Output file should be absent or empty
    let written = std::fs::metadata(output).map(|m| m.len()).unwrap_or(0);
    assert_eq!(written, 0, "no STL should be written for invalid profile");
}
