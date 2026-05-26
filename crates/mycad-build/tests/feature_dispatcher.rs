use mycad_build::build_solid_from_features;
use mycad_format::Document;
use mycad_kernel::brep::topology::IdGenerator;
use std::path::Path;

#[test]
fn simple_box_mycad_builds_solid() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("examples")
        .join("simple_box.mycad");

    let doc = Document::from_path(&path).expect("load .mycad");
    let mut g = IdGenerator::new(0);
    let solid =
        build_solid_from_features(&doc.root_component.features, &mut g).expect("build solid");

    // simple_box.mycad: width=10, height=20, depth=30 → cuboid topology
    assert_eq!(solid.vertices.len(), 8);
    assert_eq!(solid.edges.len(), 12);
    assert_eq!(solid.faces.len(), 6);
    assert_eq!(solid.shells.len(), 1);
}

/// T11: Integration — sphere.mycad → build → topology V=2, E=1, F=1.
#[test]
fn sphere_mycad_builds_solid() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("examples")
        .join("sphere.mycad");

    let doc = Document::from_path(&path).expect("load sphere.mycad");
    let mut g = IdGenerator::new(0);
    let solid =
        build_solid_from_features(&doc.root_component.features, &mut g).expect("build sphere");

    assert_eq!(solid.vertices.len(), 2, "2 poles");
    assert_eq!(solid.edges.len(), 1, "1 seam");
    assert_eq!(solid.faces.len(), 1, "1 face");
    assert_eq!(solid.shells.len(), 1);
}
