use mycad_build::build_bodies_from_features;
use mycad_format::Document;
use mycad_kernel::brep::topology::IdGenerator;

fn smoke(yaml: &str) {
    let doc: Document = serde_yaml::from_str(yaml).expect("YAML parse failed");
    if doc.root_component.features.is_empty() {
        return;
    }
    let mut gen = IdGenerator::new(0);
    build_bodies_from_features(&doc.root_component.features, &mut gen)
        .expect("build_bodies_from_features failed");
}

#[test]
fn simple_box() {
    smoke(include_str!("../../../examples/simple_box.mycad"));
}

#[test]
fn cylinder() {
    smoke(include_str!("../../../examples/cylinder.mycad"));
}

#[test]
fn cylinder_offset() {
    smoke(include_str!("../../../examples/cylinder_offset.mycad"));
}

#[test]
fn sphere() {
    smoke(include_str!("../../../examples/sphere.mycad"));
}

#[test]
fn sphere_offset() {
    smoke(include_str!("../../../examples/sphere_offset.mycad"));
}

#[test]
fn extruded_rect() {
    smoke(include_str!("../../../examples/extruded_rect.mycad"));
}

#[test]
fn two_bodies() {
    smoke(include_str!("../../../examples/two_bodies.mycad"));
}

#[test]
fn boolean_box_cut() {
    smoke(include_str!("../../../examples/boolean_box_cut.mycad"));
}

#[test]
fn boolean_box_fuse() {
    smoke(include_str!("../../../examples/boolean_box_fuse.mycad"));
}

#[test]
fn boolean_box_intersect() {
    smoke(include_str!("../../../examples/boolean_box_intersect.mycad"));
}

#[test]
fn boolean_box_void() {
    smoke(include_str!("../../../examples/boolean_box_void.mycad"));
}

#[test]
fn boolean_cut_cylinder_hole() {
    smoke(include_str!("../../../examples/boolean_cut_cylinder_hole.mycad"));
}

#[test]
fn boolean_cut_sphere_dimple() {
    smoke(include_str!("../../../examples/boolean_cut_sphere_dimple.mycad"));
}

#[test]
#[ignore = "known bug: #55 — Fuse(box, cylinder) manifold violation"]
fn boolean_fuse_box_cyl() {
    smoke(include_str!("../../../examples/boolean_fuse_box_cyl.mycad"));
}

#[test]
fn boolean_intersect_box_cyl() {
    smoke(include_str!("../../../examples/boolean_intersect_box_cyl.mycad"));
}

#[test]
fn boolean_intersect_cyl_sphere() {
    smoke(include_str!("../../../examples/boolean_intersect_cyl_sphere.mycad"));
}

#[test]
fn assembly_children_only_skipped() {
    smoke(include_str!("../../../examples/assembly.mycad"));
}
