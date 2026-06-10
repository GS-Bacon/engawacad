// Regenerates web/tests/fixtures/*.json using current tessellation code.
// Run with: cargo test -p mycad-build --test regen_viewer_fixtures -- --include-ignored
//
// This test is always #[ignore] in normal CI; invoke manually after tessellation changes.

#[test]
#[ignore = "run manually to regenerate viewer fixtures after tessellation changes"]
fn regen_all_fixtures() {
    use mycad_build::build_assembly;
    use mycad_format::Document;
    use mycad_kernel::brep::topology::IdGenerator;
    use mycad_kernel::tessellation::{tessellate_solid_with, TessellationOptions};
    use std::path::PathBuf;

    const V0_TESSELLATION: TessellationOptions = TessellationOptions {
        angular_segments: 32,
        axial_segments: 1,
    };

    let workspace_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .to_path_buf();

    let examples_dir = workspace_root.join("examples");
    let fixtures_dir = workspace_root.join("web/tests/fixtures");

    let example_names = [
        "boolean_box_cut",
        "boolean_box_fuse",
        "boolean_box_intersect",
        "boolean_box_void",
        "boolean_cut_cylinder_hole",
        "boolean_cut_sphere_dimple",
        "boolean_fuse_box_cyl",
        "boolean_intersect_box_cyl",
        "boolean_intersect_cyl_sphere",
        "cylinder",
        "cylinder_offset",
        "extruded_rect",
        "simple_box",
        "sphere",
        "sphere_offset",
        "two_bodies",
    ];

    for name in &example_names {
        let example_path = examples_dir.join(format!("{}.mycad", name));
        if !example_path.exists() {
            eprintln!("SKIP {name}: example file not found");
            continue;
        }

        let yaml = std::fs::read_to_string(&example_path)
            .unwrap_or_else(|e| panic!("failed to read {}: {}", example_path.display(), e));
        let doc = Document::from_yaml(&yaml)
            .unwrap_or_else(|e| panic!("failed to parse {}: {}", name, e));

        let mut gen = IdGenerator::new(0);
        let bodies = build_assembly(&doc, &examples_dir, &mut gen)
            .unwrap_or_else(|e| panic!("build_assembly failed for {}: {}", name, e));

        let out: Vec<serde_json::Value> = bodies
            .iter()
            .map(|b| {
                let mesh = tessellate_solid_with(&b.solid, &V0_TESSELLATION)
                    .unwrap_or_else(|e| panic!("tessellate failed for {}: {}", name, e));
                serde_json::json!({
                    "feature_id": b.feature_id,
                    "mesh": mesh,
                })
            })
            .collect();

        let json = serde_json::to_string_pretty(&out)
            .unwrap_or_else(|e| panic!("serialize failed for {}: {}", name, e));

        let fixture_path = fixtures_dir.join(format!("{}.json", name));
        std::fs::write(&fixture_path, json)
            .unwrap_or_else(|e| panic!("write failed for {}: {}", fixture_path.display(), e));

        println!("regen OK: {name} ({} bodies)", bodies.len());
    }
}
