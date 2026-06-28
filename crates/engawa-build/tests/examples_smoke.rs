use engawa_build::build_bodies_from_features;
use engawa_format::Document;
use engawa_kernel::brep::topology::IdGenerator;
use engawa_kernel::tessellation::tessellate_solid;

fn smoke(yaml: &str) {
    // Use Document::from_yaml so v1 → v2 migration runs (ADR-017 §4, #274).
    let doc = Document::from_yaml(yaml).expect("YAML parse failed");
    if doc.root_component.features.is_empty() {
        return;
    }
    let mut gen = IdGenerator::new(0);
    build_bodies_from_features(
        &doc.root_component.features,
        &doc.root_component.ref_planes,
        &mut gen,
    )
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
    let doc = Document::from_yaml(yaml).expect("YAML parse failed");
    let mut gen = IdGenerator::new(0);
    let bodies = build_bodies_from_features(
        &doc.root_component.features,
        &doc.root_component.ref_planes,
        &mut gen,
    )
    .expect("build failed");
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

#[test]
fn sketch_via_refplane() {
    smoke(include_str!("../../../examples/sketch_via_refplane.engawa"));
}

/// Issue #215: Face EntityRef 経路の smoke テスト
#[test]
fn sketch_via_face_entity_ref() {
    smoke(include_str!(
        "../../../examples/sketch_via_face_entity_ref.engawa"
    ));
}

/// Issue #216: ModelFace 上のスケッチ描画 e2e (8 角形 polygon profile) の smoke テスト
#[test]
fn sketch_circle_on_face() {
    smoke(include_str!(
        "../../../examples/sketch_circle_on_face.engawa"
    ));
}

/// Issue #217: ModelFace Sketch を Extrude で押出 (柱) の smoke テスト
#[test]
fn sketch_extrude_pillar() {
    smoke(include_str!(
        "../../../examples/sketch_extrude_pillar.engawa"
    ));
}

/// Issue #218: ModelFace Sketch を ExtrudeCut で穴あけ の smoke テスト
#[test]
fn sketch_extrudecut_hole() {
    smoke(include_str!(
        "../../../examples/sketch_extrudecut_hole.engawa"
    ));
}

/// Issue #273: Phase 10 Circle / Arc smoke テスト
#[test]
fn circle_arc() {
    smoke(include_str!("../../../examples/circle_arc.engawa"));
}

/// Issue #274: Phase 10 Ellipse / Conic smoke テスト
#[test]
fn ellipse_conic() {
    smoke(include_str!("../../../examples/ellipse_conic.engawa"));
}
