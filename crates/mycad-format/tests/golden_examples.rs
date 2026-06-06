// byte-identical golden tests for all example .mycad files (#24)
use mycad_format::Document;
use std::path::{Path, PathBuf};

fn examples_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples")
}

fn assert_golden(filename: &str, golden: &str) {
    let path = examples_dir().join(filename);
    let doc =
        Document::from_path(&path).unwrap_or_else(|e| panic!("failed to load {filename}: {e}"));
    let yaml = doc.to_yaml().unwrap();
    assert_eq!(yaml, golden, "{filename} YAML golden mismatch");
}

#[test]
fn golden_cylinder() {
    assert_golden(
        "cylinder.mycad",
        "schema_version: 1\nversion: 0.1.0\nroot_component:\n  name: Simple Cylinder\n  features:\n  - type: create_cylinder\n    id: cyl_1\n    radius: 5.0\n    height: 20.0\n",
    );
}

#[test]
fn golden_sphere() {
    assert_golden(
        "sphere.mycad",
        "schema_version: 1\nversion: 0.1.0\nroot_component:\n  name: Simple Sphere\n  features:\n  - type: create_sphere\n    id: sphere_1\n    radius: 5.0\n",
    );
}

#[test]
fn golden_cylinder_offset() {
    assert_golden(
        "cylinder_offset.mycad",
        concat!(
            "schema_version: 1\nversion: 0.1.0\nroot_component:\n  name: Offset Cylinder\n  features:\n",
            "  - type: create_cylinder\n    id: cyl_1\n    radius: 5.0\n    height: 20.0\n    origin:\n    - 0.0\n    - 0.0\n    - -10.0\n",
        ),
    );
}

#[test]
fn golden_sphere_offset() {
    assert_golden(
        "sphere_offset.mycad",
        concat!(
            "schema_version: 1\nversion: 0.1.0\nroot_component:\n  name: Offset Sphere\n  features:\n",
            "  - type: create_sphere\n    id: sphere_1\n    radius: 5.0\n    center:\n    - 2.0\n    - 0.0\n    - 0.0\n",
        ),
    );
}

#[test]
fn golden_boolean_box_cut() {
    assert_golden(
        "boolean_box_cut.mycad",
        concat!(
            "schema_version: 1\nversion: 0.1.0\nroot_component:\n  name: Boolean Box Cut\n  features:\n",
            "  - type: create_box\n    id: target\n    width: 4.0\n    height: 4.0\n    depth: 4.0\n",
            "  - type: create_cylinder\n    id: tool\n    radius: 1.0\n    height: 6.0\n    origin:\n    - 0.0\n    - 0.0\n    - -3.0\n",
            "  - type: cut\n    id: cut1\n    target: target\n    tool: tool\n",
        ),
    );
}

#[test]
fn golden_boolean_box_fuse() {
    assert_golden(
        "boolean_box_fuse.mycad",
        concat!(
            "schema_version: 1\nversion: 0.1.0\nroot_component:\n  name: Boolean Box Fuse\n  features:\n",
            "  - type: create_box\n    id: box_a\n    width: 6.0\n    height: 2.0\n    depth: 2.0\n",
            "  - type: create_box\n    id: box_b\n    width: 2.0\n    height: 6.0\n    depth: 2.0\n",
            "  - type: fuse\n    id: fuse1\n    target: box_a\n    tool: box_b\n",
        ),
    );
}

#[test]
fn golden_boolean_box_intersect() {
    assert_golden(
        "boolean_box_intersect.mycad",
        concat!(
            "schema_version: 1\nversion: 0.1.0\nroot_component:\n  name: Boolean Box Intersect\n  features:\n",
            "  - type: create_box\n    id: box_a\n    width: 4.0\n    height: 4.0\n    depth: 2.0\n",
            "  - type: create_cylinder\n    id: cyl_b\n    radius: 1.5\n    height: 6.0\n    origin:\n    - 0.0\n    - 0.0\n    - -3.0\n",
            "  - type: intersect\n    id: int1\n    target: box_a\n    tool: cyl_b\n",
        ),
    );
}

#[test]
fn golden_boolean_box_void() {
    assert_golden(
        "boolean_box_void.mycad",
        concat!(
            "schema_version: 1\nversion: 0.1.0\nroot_component:\n  name: Boolean Box Void Shell\n  features:\n",
            "  - type: create_box\n    id: outer\n    width: 6.0\n    height: 6.0\n    depth: 6.0\n",
            "  - type: create_sphere\n    id: inner\n    radius: 2.0\n    center:\n    - 0.0\n    - 0.0\n    - 3.5\n",
            "  - type: cut\n    id: cut1\n    target: outer\n    tool: inner\n",
        ),
    );
}

#[test]
fn golden_boolean_cut_cylinder_hole() {
    assert_golden(
        "boolean_cut_cylinder_hole.mycad",
        concat!(
            "schema_version: 1\nversion: 0.1.0\nroot_component:\n  name: Box Cut Cylinder (Blind Hole)\n  features:\n",
            "  - type: create_box\n    id: box1\n    width: 10.0\n    height: 10.0\n    depth: 10.0\n",
            "  - type: create_cylinder\n    id: cyl1\n    radius: 2.0\n    height: 6.0\n",
            "  - type: cut\n    id: cut1\n    target: box1\n    tool: cyl1\n",
        ),
    );
}

#[test]
fn golden_boolean_cut_sphere_dimple() {
    assert_golden(
        "boolean_cut_sphere_dimple.mycad",
        concat!(
            "schema_version: 1\nversion: 0.1.0\nroot_component:\n  name: Box Cut Sphere (Dimple)\n  features:\n",
            "  - type: create_box\n    id: box1\n    width: 10.0\n    height: 10.0\n    depth: 10.0\n",
            "  - type: create_sphere\n    id: sphere1\n    radius: 3.0\n    center:\n    - 0.0\n    - 0.0\n    - 6.0\n",
            "  - type: cut\n    id: cut1\n    target: box1\n    tool: sphere1\n",
        ),
    );
}

#[test]
fn golden_boolean_fuse_box_cyl() {
    assert_golden(
        "boolean_fuse_box_cyl.mycad",
        concat!(
            "schema_version: 1\nversion: 0.1.0\nroot_component:\n  name: Boolean Fuse Box + Cylinder\n  features:\n",
            "  - type: create_box\n    id: box1\n    width: 10.0\n    height: 10.0\n    depth: 10.0\n",
            "  - type: create_cylinder\n    id: cyl1\n    radius: 2.0\n    height: 15.0\n    origin:\n    - 0.0\n    - 0.0\n    - -7.5\n",
            "  - type: fuse\n    id: result\n    target: box1\n    tool: cyl1\n",
        ),
    );
}

#[test]
fn golden_boolean_intersect_box_cyl() {
    assert_golden(
        "boolean_intersect_box_cyl.mycad",
        concat!(
            "schema_version: 1\nversion: 0.1.0\nroot_component:\n  name: Boolean Intersect Box + Cylinder\n  features:\n",
            "  - type: create_box\n    id: box1\n    width: 10.0\n    height: 10.0\n    depth: 10.0\n",
            "  - type: create_cylinder\n    id: cyl1\n    radius: 2.0\n    height: 15.0\n",
            "  - type: intersect\n    id: result\n    target: box1\n    tool: cyl1\n",
        ),
    );
}

#[test]
fn golden_boolean_intersect_cyl_sphere() {
    assert_golden(
        "boolean_intersect_cyl_sphere.mycad",
        concat!(
            "schema_version: 1\nversion: 0.1.0\nroot_component:\n  name: Boolean Intersect Cylinder + Sphere\n  features:\n",
            "  - type: create_cylinder\n    id: cyl1\n    radius: 3.0\n    height: 20.0\n    origin:\n    - 0.0\n    - 0.0\n    - -10.0\n",
            "  - type: create_sphere\n    id: sph1\n    radius: 5.0\n",
            "  - type: intersect\n    id: result\n    target: cyl1\n    tool: sph1\n",
        ),
    );
}

#[test]
fn golden_two_bodies() {
    assert_golden(
        "two_bodies.mycad",
        concat!(
            "schema_version: 1\nversion: 0.1.0\nroot_component:\n  name: Two Bodies\n  features:\n",
            "  - type: create_sketch\n    id: sketch_a\n    plane: xy\n    profile:\n",
            "    - id: sa1\n      from:\n      - 0.0\n      - 0.0\n      to:\n      - 10.0\n      - 0.0\n",
            "    - id: sa2\n      from:\n      - 10.0\n      - 0.0\n      to:\n      - 10.0\n      - 10.0\n",
            "    - id: sa3\n      from:\n      - 10.0\n      - 10.0\n      to:\n      - 0.0\n      - 10.0\n",
            "    - id: sa4\n      from:\n      - 0.0\n      - 10.0\n      to:\n      - 0.0\n      - 0.0\n",
            "  - type: create_sketch\n    id: sketch_b\n    plane: xy\n    profile:\n",
            "    - id: sb1\n      from:\n      - 50.0\n      - 0.0\n      to:\n      - 60.0\n      - 0.0\n",
            "    - id: sb2\n      from:\n      - 60.0\n      - 0.0\n      to:\n      - 60.0\n      - 10.0\n",
            "    - id: sb3\n      from:\n      - 60.0\n      - 10.0\n      to:\n      - 50.0\n      - 10.0\n",
            "    - id: sb4\n      from:\n      - 50.0\n      - 10.0\n      to:\n      - 50.0\n      - 0.0\n",
            "  - type: extrude\n    id: body_a\n    sketch: sketch_a\n    depth: 10.0\n",
            "  - type: extrude\n    id: body_b\n    sketch: sketch_b\n    depth: 10.0\n",
        ),
    );
}

#[test]
fn golden_assembly() {
    assert_golden(
        "assembly.mycad",
        concat!(
            "schema_version: 1\nversion: 0.1.0\nroot_component:\n  name: Simple Assembly\n  children:\n",
            "  - name: Base Plate\n    features:\n",
            "    - type: create_box\n      id: plate_1\n      width: 100.0\n      height: 5.0\n      depth: 100.0\n",
            "  - name: Pillar\n    transform:\n      position:\n      - 0.0\n      - 25.0\n      - 0.0\n      rotation:\n      - 0.0\n      - 0.0\n      - 0.0\n    features:\n",
            "    - type: create_box\n      id: pillar_1\n      width: 10.0\n      height: 50.0\n      depth: 10.0\n",
            "  - name: Bolt\n    transform:\n      position:\n      - 20.0\n      - 5.0\n      - 20.0\n      rotation:\n      - 0.0\n      - 0.0\n      - 0.0\n    ref: stdlib://fasteners/jis_b1176/M5x20\n",
        ),
    );
}
