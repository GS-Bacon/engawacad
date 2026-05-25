use std::path::PathBuf;
use std::process::{Command, ExitCode};

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let task = args.first().map(|s| s.as_str()).unwrap_or("help");

    match task {
        "ci" => ci(),
        "gen-ts" => gen_ts(),
        "help" | "--help" | "-h" => {
            println!("Usage: cargo xtask <TASK>");
            println!();
            println!("Tasks:");
            println!("  ci      Run fmt check, clippy, tests, build, and TS drift detection");
            println!("  gen-ts  Generate TypeScript types to web/src/generated/");
            ExitCode::SUCCESS
        }
        other => {
            eprintln!("Unknown task: {other}");
            eprintln!("Run `cargo xtask help` for available tasks.");
            ExitCode::FAILURE
        }
    }
}

fn gen_ts() -> ExitCode {
    use mycad_api::{ErrorResponse, MeshRequest};
    use mycad_format::feature::EntityRef;
    use mycad_format::Document;
    use mycad_kernel::tessellation::TriangleMesh;
    use ts_rs::TS;

    let out_dir = workspace_root().join("web/src/generated");

    if out_dir.exists() {
        if let Err(e) = std::fs::remove_dir_all(&out_dir) {
            eprintln!("Failed to clean output directory: {e}");
            return ExitCode::FAILURE;
        }
    }
    if let Err(e) = std::fs::create_dir_all(&out_dir) {
        eprintln!("Failed to create output directory: {e}");
        return ExitCode::FAILURE;
    }

    std::env::set_var("TS_RS_EXPORT_DIR", &out_dir);
    let cfg = ts_rs::Config::from_env();

    type ExportFn = Box<dyn FnOnce(&ts_rs::Config) -> Result<(), ts_rs::ExportError>>;
    let roots: Vec<(&str, ExportFn)> = vec![
        ("Document", Box::new(Document::export_all)),
        ("EntityRef", Box::new(EntityRef::export_all)),
        ("TriangleMesh", Box::new(TriangleMesh::export_all)),
        ("MeshRequest", Box::new(MeshRequest::export_all)),
        ("ErrorResponse", Box::new(ErrorResponse::export_all)),
    ];

    for (name, export_fn) in roots {
        println!("Exporting {name}...");
        if let Err(e) = export_fn(&cfg) {
            eprintln!("Failed to export {name}: {e}");
            return ExitCode::FAILURE;
        }
    }

    println!("TypeScript types exported to {}", out_dir.display());
    ExitCode::SUCCESS
}

fn ci() -> ExitCode {
    let steps: &[(&str, &[&str])] = &[
        (
            "Checking formatting",
            &["cargo", "fmt", "--all", "--", "--check"],
        ),
        (
            "Running clippy",
            &["cargo", "clippy", "--workspace", "--", "-D", "warnings"],
        ),
        ("Running tests", &["cargo", "test", "--workspace"]),
        ("Building", &["cargo", "build", "--workspace"]),
    ];

    for (label, cmd) in steps {
        println!("\n=== {label} ===");
        let status = Command::new(cmd[0])
            .args(&cmd[1..])
            .status()
            .expect("failed to execute command");

        if !status.success() {
            eprintln!("FAILED: {label}");
            return ExitCode::FAILURE;
        }
    }

    println!("\n=== Generating TypeScript types ===");
    if gen_ts() != ExitCode::SUCCESS {
        eprintln!("FAILED: TypeScript type generation");
        return ExitCode::FAILURE;
    }

    println!("\n=== Checking TS drift ===");
    let tracked = Command::new("git")
        .args(["ls-files", "web/src/generated/"])
        .output()
        .expect("failed to run git ls-files");
    if tracked.stdout.is_empty() {
        println!("No committed TS files found — skipping drift check (run `cargo xtask gen-ts` and commit the results).");
    } else {
        let output = Command::new("git")
            .args([
                "status",
                "--porcelain",
                "--untracked-files=all",
                "--",
                "web/src/generated/",
            ])
            .output()
            .expect("failed to run git status");

        let stdout = String::from_utf8_lossy(&output.stdout);
        if !stdout.trim().is_empty() {
            eprintln!("FAILED: TypeScript types are out of sync with committed versions");
            eprintln!("Run `cargo xtask gen-ts` and commit the results.");
            eprintln!("Drift detected:\n{stdout}");
            return ExitCode::FAILURE;
        }
    }

    println!("\n=== All CI checks passed ===");
    ExitCode::SUCCESS
}

fn workspace_root() -> PathBuf {
    let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR not set");
    PathBuf::from(manifest_dir)
        .parent()
        .expect("no parent")
        .parent()
        .expect("no grandparent")
        .to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::sync::{Mutex, OnceLock};

    static TEST_LOCK: OnceLock<Mutex<()>> = OnceLock::new();

    fn export_to_temp_dir() -> PathBuf {
        let _guard = TEST_LOCK.get_or_init(|| Mutex::new(())).lock().unwrap();

        use mycad_api::{ErrorResponse, MeshRequest};
        use mycad_format::feature::EntityRef;
        use mycad_format::Document;
        use mycad_kernel::tessellation::TriangleMesh;
        use ts_rs::TS;

        let dir = tempfile::tempdir().unwrap().keep();
        std::env::set_var("TS_RS_EXPORT_DIR", &dir);
        let cfg = ts_rs::Config::from_env();

        Document::export_all(&cfg).unwrap();
        EntityRef::export_all(&cfg).unwrap();
        TriangleMesh::export_all(&cfg).unwrap();
        MeshRequest::export_all(&cfg).unwrap();
        ErrorResponse::export_all(&cfg).unwrap();

        dir
    }

    fn read_generated(dir: &std::path::Path, name: &str) -> String {
        fs::read_to_string(dir.join(name)).unwrap_or_else(|e| panic!("failed to read {name}: {e}"))
    }

    #[test]
    fn t01_determinism() {
        let dir1 = export_to_temp_dir();
        let dir2 = export_to_temp_dir();

        let files = [
            "Document.ts",
            "Component.ts",
            "Feature.ts",
            "Transform.ts",
            "ComponentRef.ts",
            "EntityRef.ts",
            "TriangleMesh.ts",
            "MeshRequest.ts",
            "ErrorResponse.ts",
        ];

        for name in &files {
            let c1 = read_generated(&dir1, name);
            let c2 = read_generated(&dir2, name);
            assert_eq!(c1, c2, "{name} differs between runs");
        }
    }

    static FEATURE_GOLDEN: &str = r#"// This file was generated by [ts-rs](https://github.com/Aleph-Alpha/ts-rs). Do not edit this file manually.

/**
 * A feature — one step in the operation history.
 */
export type Feature = { "type": "create_box", id: string, width: number, height: number, depth: number, } | { "type": "create_cylinder", id: string, radius: number, height: number, } | { "type": "create_sphere", id: string, radius: number, } | { "type": "extrude", id: string, sketch: string, depth: number, } | { "type": "cut", id: string, target: string, tool: string, } | { "type": "fuse", id: string, target: string, tool: string, } | { "type": "intersect", id: string, target: string, tool: string, };
"#;

    #[test]
    fn t02_feature_tagged_union() {
        let dir = export_to_temp_dir();
        let actual = read_generated(&dir, "Feature.ts");
        assert_eq!(actual, FEATURE_GOLDEN);

        assert!(
            actual.contains(r#""type": "create_box""#),
            "missing create_box tag"
        );
        assert!(
            actual.contains(r#""type": "create_cylinder""#),
            "missing create_cylinder tag"
        );
        assert!(
            actual.contains(r#""type": "create_sphere""#),
            "missing create_sphere tag"
        );
        assert!(
            actual.contains(r#""type": "extrude""#),
            "missing extrude tag"
        );
        assert!(actual.contains(r#""type": "cut""#), "missing cut tag");
        assert!(actual.contains(r#""type": "fuse""#), "missing fuse tag");
        assert!(
            actual.contains(r#""type": "intersect""#),
            "missing intersect tag"
        );
    }

    static TRIANGLE_MESH_GOLDEN: &str = "// This file was generated by [ts-rs](https://github.com/Aleph-Alpha/ts-rs). Do not edit this file manually.\n\n/**\n * A triangle mesh for rendering.\n */\nexport type TriangleMesh = { \n/**\n * Vertex positions (x, y, z).\n */\npositions: Array<[number, number, number]>, \n/**\n * Normal vectors per vertex.\n */\nnormals: Array<[number, number, number]>, \n/**\n * Triangle indices (every 3 indices form a triangle).\n */\nindices: Array<number>, };\n";

    #[test]
    fn t03_triangle_mesh() {
        let dir = export_to_temp_dir();
        let actual = read_generated(&dir, "TriangleMesh.ts");
        assert_eq!(actual, TRIANGLE_MESH_GOLDEN);
    }

    static DOCUMENT_GOLDEN: &str = "// This file was generated by [ts-rs](https://github.com/Aleph-Alpha/ts-rs). Do not edit this file manually.\nimport type { Component } from \"./Component\";\n\n/**\n * The top-level document representing a MyCad design file.\n */\nexport type Document = { \n/**\n * Kernel version that created this document.\n */\nversion: string, \n/**\n * The root component (assembly or single part).\n */\nroot_component: Component, };\n";

    static COMPONENT_GOLDEN: &str = "// This file was generated by [ts-rs](https://github.com/Aleph-Alpha/ts-rs). Do not edit this file manually.\nimport type { ComponentRef } from \"./ComponentRef\";\nimport type { Feature } from \"./Feature\";\nimport type { Transform } from \"./Transform\";\n\n/**\n * A component in the design hierarchy.\n * Can contain features (inline part definition), children (sub-components),\n * or be a reference to an external file/library.\n */\nexport type Component = { \n/**\n * Human-readable name.\n */\nname: string, \n/**\n * Transform relative to the parent component.\n */\ntransform?: Transform, \n/**\n * External reference (if this component is not defined inline).\n */\nref?: ComponentRef | null, \n/**\n * Ordered list of features (the operation history).\n */\nfeatures?: Array<Feature>, \n/**\n * Child components.\n */\nchildren?: Array<Component>, };\n";

    #[test]
    fn t04_document_and_component() {
        let dir = export_to_temp_dir();
        let doc = read_generated(&dir, "Document.ts");
        assert_eq!(doc, DOCUMENT_GOLDEN);

        let comp = read_generated(&dir, "Component.ts");
        assert_eq!(comp, COMPONENT_GOLDEN);

        assert!(comp.contains("transform?:"), "transform should be optional");
        assert!(
            comp.contains("ref?:"),
            "ref field should be renamed and optional"
        );
        assert!(comp.contains("features?:"), "features should be optional");
        assert!(comp.contains("children?:"), "children should be optional");
    }

    static COMPONENT_REF_GOLDEN: &str = r#"// This file was generated by [ts-rs](https://github.com/Aleph-Alpha/ts-rs). Do not edit this file manually.

export type ComponentRef = string;
"#;

    #[test]
    fn t05_component_ref_is_string() {
        let dir = export_to_temp_dir();
        let actual = read_generated(&dir, "ComponentRef.ts");
        assert_eq!(actual, COMPONENT_REF_GOLDEN);
    }

    static MESH_REQUEST_GOLDEN: &str = r#"// This file was generated by [ts-rs](https://github.com/Aleph-Alpha/ts-rs). Do not edit this file manually.

export type MeshRequest = { file: string, };
"#;

    static ERROR_RESPONSE_GOLDEN: &str = r#"// This file was generated by [ts-rs](https://github.com/Aleph-Alpha/ts-rs). Do not edit this file manually.

export type ErrorResponse = { error: string, };
"#;

    #[test]
    fn t09_api_transport_dto() {
        let dir = export_to_temp_dir();
        let mesh_req = read_generated(&dir, "MeshRequest.ts");
        assert_eq!(mesh_req, MESH_REQUEST_GOLDEN);

        let err_resp = read_generated(&dir, "ErrorResponse.ts");
        assert_eq!(err_resp, ERROR_RESPONSE_GOLDEN);
    }

    static ENTITY_REF_GOLDEN: &str = "// This file was generated by [ts-rs](https://github.com/Aleph-Alpha/ts-rs). Do not edit this file manually.\n\n/**\n * A reference to a topological entity (face, edge, vertex) on a feature's result.\n */\nexport type EntityRef = { \n/**\n * The feature that created this entity.\n */\nfeature_id: string, \n/**\n * The role of this entity in the feature (e.g., \"top_face\", \"side_edge_0\").\n */\nrole: string, };\n";

    #[test]
    fn t10_entity_ref_explicit_root() {
        let dir = export_to_temp_dir();
        let actual = read_generated(&dir, "EntityRef.ts");
        assert_eq!(actual, ENTITY_REF_GOLDEN);
    }

    #[test]
    fn t06_gen_ts_produces_all_files() {
        let dir = export_to_temp_dir();
        let expected = [
            "Document.ts",
            "Component.ts",
            "Feature.ts",
            "Transform.ts",
            "ComponentRef.ts",
            "EntityRef.ts",
            "TriangleMesh.ts",
            "MeshRequest.ts",
            "ErrorResponse.ts",
        ];
        for name in &expected {
            let path = dir.join(name);
            assert!(path.exists(), "expected file {name} to be generated");
            let content = fs::read_to_string(&path).unwrap();
            assert!(!content.is_empty(), "{name} should not be empty");
        }
    }

    #[test]
    fn t01_determinism_100_runs() {
        let dir1 = export_to_temp_dir();
        let reference_files: Vec<(String, String)> = [
            "Document.ts",
            "Component.ts",
            "Feature.ts",
            "Transform.ts",
            "ComponentRef.ts",
            "EntityRef.ts",
            "TriangleMesh.ts",
            "MeshRequest.ts",
            "ErrorResponse.ts",
        ]
        .iter()
        .map(|name| (name.to_string(), read_generated(&dir1, name)))
        .collect();

        for i in 0..100 {
            let dir = export_to_temp_dir();
            for (name, expected) in &reference_files {
                let actual = read_generated(&dir, name);
                assert_eq!(actual, *expected, "run {i}: {name} differs");
            }
        }
    }
}
