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
