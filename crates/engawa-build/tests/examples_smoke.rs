use engawa_build::build_bodies_from_features;
use engawa_format::Document;
use engawa_kernel::brep::topology::IdGenerator;
use engawa_kernel::tessellation::tessellate_solid;

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
    smoke(include_str!("../../../examples/simple_box.engawa"));
}

#[test]
fn cylinder() {
    smoke(include_str!("../../../examples/cylinder.engawa"));
}

#[test]
fn cylinder_offset() {
    smoke(include_str!("../../../examples/cylinder_offset.engawa"));
}

#[test]
fn sphere() {
    smoke(include_str!("../../../examples/sphere.engawa"));
}

#[test]
fn sphere_offset() {
    smoke(include_str!("../../../examples/sphere_offset.engawa"));
}

#[test]
fn extruded_rect() {
    smoke(include_str!("../../../examples/extruded_rect.engawa"));
}

#[test]
fn two_bodies() {
    smoke(include_str!("../../../examples/two_bodies.engawa"));
}

#[test]
fn m5x20_bolt() {
    smoke(include_str!(
        "../../../stdlib/fasteners/jis_b1176/M5x20.engawa"
    ));
}

#[test]
fn boolean_box_cut() {
    smoke(include_str!("../../../examples/boolean_box_cut.engawa"));
}

#[test]
fn boolean_box_fuse() {
    smoke(include_str!("../../../examples/boolean_box_fuse.engawa"));
}

#[test]
fn boolean_box_intersect() {
    smoke(include_str!(
        "../../../examples/boolean_box_intersect.engawa"
    ));
}

#[test]
fn boolean_box_void() {
    smoke(include_str!("../../../examples/boolean_box_void.engawa"));
}

#[test]
fn boolean_cut_cylinder_hole() {
    smoke(include_str!(
        "../../../examples/boolean_cut_cylinder_hole.engawa"
    ));
}

#[test]
fn boolean_cut_sphere_dimple() {
    smoke(include_str!(
        "../../../examples/boolean_cut_sphere_dimple.engawa"
    ));
}

/// Issue #50: build + tessellate must both succeed (was: TrimmedFaceUnsupported).
#[test]
fn boolean_cut_sphere_dimple_tessellate() {
    let yaml = include_str!("../../../examples/boolean_cut_sphere_dimple.engawa");
    let doc: Document = serde_yaml::from_str(yaml).expect("YAML parse failed");
    let mut gen = IdGenerator::new(0);
    let bodies =
        build_bodies_from_features(&doc.root_component.features, &mut gen).expect("build failed");
    for body in bodies.live() {
        tessellate_solid(&body.solid).expect("tessellate failed for boolean_cut_sphere_dimple");
    }
}

#[test]
fn boolean_fuse_box_cyl() {
    smoke(include_str!(
        "../../../examples/boolean_fuse_box_cyl.engawa"
    ));
}

#[test]
fn boolean_intersect_box_cyl() {
    smoke(include_str!(
        "../../../examples/boolean_intersect_box_cyl.engawa"
    ));
}

#[test]
fn boolean_intersect_cyl_sphere() {
    smoke(include_str!(
        "../../../examples/boolean_intersect_cyl_sphere.engawa"
    ));
}

#[test]
fn assembly_children_only_skipped() {
    smoke(include_str!("../../../examples/assembly.engawa"));
}
