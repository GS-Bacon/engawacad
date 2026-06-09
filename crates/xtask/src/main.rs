use std::path::PathBuf;
use std::process::{Command, ExitCode};

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let task = args.first().map(|s| s.as_str()).unwrap_or("help");

    match task {
        "ci" => ci(),
        "web" => web(),
        "gen-ts" => gen_ts(),
        "acceptance" => acceptance(),
        "help" | "--help" | "-h" => {
            println!("Usage: cargo xtask <TASK>");
            println!();
            println!("Tasks:");
            println!("  ci          Run web build, then fmt check, clippy, tests, build, TS drift, web checks, release verification");
            println!("  web         Generate TS types and build web frontend (vite build)");
            println!("  gen-ts      Generate TypeScript types to web/src/generated/");
            println!("  acceptance  Run acceptance tests (add --fuzz for fuzz harness)");
            ExitCode::SUCCESS
        }
        other => {
            eprintln!("Unknown task: {other}");
            eprintln!("Run `cargo xtask help` for available tasks.");
            ExitCode::FAILURE
        }
    }
}

fn web() -> ExitCode {
    let web_dir = workspace_root().join("web");

    println!("=== Generating TypeScript types ===");
    if gen_ts() != ExitCode::SUCCESS {
        eprintln!("FAILED: TypeScript type generation");
        return ExitCode::FAILURE;
    }

    let lockfile = web_dir.join("package-lock.json");
    if !lockfile.exists() {
        eprintln!("FAILED: package-lock.json not found in web/");
        return ExitCode::FAILURE;
    }

    let npm_steps: &[(&str, &[&str])] = &[
        ("Installing web dependencies", &["npm", "ci"]),
        ("TypeScript type check", &["npx", "tsc", "--noEmit"]),
        ("Running vitest", &["npx", "vitest", "run"]),
        (
            "Building web frontend (vite build)",
            &["npx", "vite", "build"],
        ),
    ];

    for (label, cmd) in npm_steps {
        println!("\n--- {label} ---");
        let status = Command::new(cmd[0])
            .args(&cmd[1..])
            .current_dir(&web_dir)
            .status()
            .expect("failed to execute command");

        if !status.success() {
            eprintln!("FAILED: {label}");
            return ExitCode::FAILURE;
        }
    }

    println!("\n=== Web build complete ===");
    ExitCode::SUCCESS
}

fn gen_ts() -> ExitCode {
    gen_ts_to(&workspace_root().join("web/src/generated"))
}

pub(crate) fn gen_ts_to(out_dir: &std::path::Path) -> ExitCode {
    use mycad_api::transport::BodyMesh;
    use mycad_api::ErrorResponse;
    use mycad_format::feature::EntityRef;
    use mycad_format::Document;
    use mycad_kernel::tessellation::TriangleMesh;
    use ts_rs::TS;

    if out_dir.exists() {
        if let Err(e) = std::fs::remove_dir_all(out_dir) {
            eprintln!("Failed to clean output directory: {e}");
            return ExitCode::FAILURE;
        }
    }
    if let Err(e) = std::fs::create_dir_all(out_dir) {
        eprintln!("Failed to create output directory: {e}");
        return ExitCode::FAILURE;
    }

    std::env::set_var("TS_RS_EXPORT_DIR", out_dir);
    let cfg = ts_rs::Config::from_env();

    type ExportFn = Box<dyn FnOnce(&ts_rs::Config) -> Result<(), ts_rs::ExportError>>;
    let roots: Vec<(&str, ExportFn)> = vec![
        ("Document", Box::new(Document::export_all)),
        ("EntityRef", Box::new(EntityRef::export_all)),
        ("TriangleMesh", Box::new(TriangleMesh::export_all)),
        ("BodyMesh", Box::new(BodyMesh::export_all)),
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

fn acceptance() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(2).collect();
    let fuzz = args.iter().any(|a| a == "--fuzz");
    let record = args.iter().any(|a| a == "--record");

    // --workers N  or  --workers=N
    let workers: String = args
        .windows(2)
        .find(|w| w[0] == "--workers")
        .map(|w| w[1].clone())
        .or_else(|| {
            args.iter()
                .find(|a| a.starts_with("--workers="))
                .map(|a| a.trim_start_matches("--workers=").to_owned())
        })
        .unwrap_or_else(|| "1".to_owned());

    println!("=== Running acceptance tests ===");
    let status = Command::new("cargo")
        .args([
            "test",
            "-p",
            "mycad-api",
            "--test",
            "post_features_acceptance",
        ])
        .status()
        .expect("failed to execute cargo test");
    if !status.success() {
        eprintln!("FAILED: acceptance tests");
        return ExitCode::FAILURE;
    }

    if fuzz {
        println!("\n=== Running fuzz tests ===");
        let status = Command::new("cargo")
            .args([
                "test",
                "-p",
                "mycad-api",
                "--test",
                "fuzz_features",
                "--",
                "--include-ignored",
            ])
            .status()
            .expect("failed to execute fuzz tests");
        if !status.success() {
            eprintln!("FAILED: fuzz tests");
            return ExitCode::FAILURE;
        }
    }

    let record_label = if record { " [recording]" } else { "" };
    println!("\n=== Running Playwright E2E tests (workers={workers}){record_label} ===");
    if which("npx").is_some() {
        let pw_web_dir = workspace_root().join("web");
        let mut cmd = Command::new("npx");
        cmd.args(["playwright", "test", "--workers", &workers])
            .current_dir(&pw_web_dir);
        if record {
            cmd.env("PLAYWRIGHT_VIDEO", "1");
        }
        let status = cmd.status().expect("failed to execute playwright test");
        if !status.success() {
            eprintln!("FAILED: Playwright E2E tests");
            return ExitCode::FAILURE;
        }
        if record {
            tile_videos(&pw_web_dir);
        }
    } else {
        eprintln!("WARNING: npx not found — skipping Playwright E2E tests (install Node.js >= 20)");
    }

    println!("\n=== Acceptance tests passed ===");
    ExitCode::SUCCESS
}

fn tile_videos(web_dir: &std::path::Path) {
    let Some(_ffmpeg) = which("ffmpeg") else {
        eprintln!("WARNING: ffmpeg が見つかりません — タイル合成をスキップします");
        return;
    };

    let results_dir = web_dir.join("test-results");
    let videos = collect_webm_files(&results_dir);

    if videos.is_empty() {
        eprintln!("WARNING: --record が指定されましたが動画ファイルが見つかりませんでした");
        return;
    }

    let output = results_dir.join("acceptance-tiled.mp4");
    println!("\n=== Tiling {} video(s) with FFmpeg ===", videos.len());

    let status = if videos.len() == 1 {
        Command::new("ffmpeg")
            .args([
                "-y",
                "-i",
                videos[0].to_str().unwrap(),
                output.to_str().unwrap(),
            ])
            .status()
    } else {
        let mut ffmpeg_args: Vec<String> = vec!["-y".into()];
        for v in &videos {
            ffmpeg_args.push("-i".into());
            ffmpeg_args.push(v.to_str().unwrap().to_owned());
        }
        let filter = build_xstack_filter(videos.len());
        ffmpeg_args.extend(["-filter_complex".into(), filter, "-vcodec".into(), "libx264".into()]);
        ffmpeg_args.push(output.to_str().unwrap().to_owned());
        Command::new("ffmpeg").args(&ffmpeg_args).status()
    };

    match status {
        Ok(s) if s.success() => {
            println!("Tiled video saved to {}", output.display());
        }
        Ok(_) => eprintln!("WARNING: ffmpeg タイル合成に失敗しました"),
        Err(e) => eprintln!("WARNING: ffmpeg 実行エラー: {e}"),
    }
}

fn collect_webm_files(dir: &std::path::Path) -> Vec<std::path::PathBuf> {
    let mut videos = Vec::new();
    if !dir.exists() {
        return videos;
    }
    if let Ok(read_dir) = std::fs::read_dir(dir) {
        for entry in read_dir.flatten() {
            let path = entry.path();
            if path.is_dir() {
                videos.extend(collect_webm_files(&path));
            } else if path.extension().and_then(|e| e.to_str()) == Some("webm") {
                videos.push(path);
            }
        }
    }
    videos.sort();
    videos
}

fn build_xstack_filter(n: usize) -> String {
    const COLS: usize = 4;
    let mut positions: Vec<String> = Vec::with_capacity(n);
    for i in 0..n {
        let col = i % COLS;
        let row = i / COLS;
        let x = match col {
            0 => "0".to_owned(),
            1 => "w0".to_owned(),
            2 => "w0+w1".to_owned(),
            _ => "w0+w1+w2".to_owned(),
        };
        // xstack では N*h0 形式の乗算が効かないため h0+h1+...hN-1 で累積する
        let y = if row == 0 {
            "0".to_owned()
        } else {
            (0..row).map(|r| format!("h{}", r * COLS)).collect::<Vec<_>>().join("+")
        };
        positions.push(format!("{x}_{y}"));
    }
    format!("xstack=inputs={}:layout={}:fill=black", n, positions.join("|"))
}

fn ci() -> ExitCode {
    let Some(_node) = which("node") else {
        eprintln!("FAILED: Node.js (>=20) and npm are required for CI.");
        eprintln!("Install from https://nodejs.org/ or via nvm, then re-run `cargo xtask ci`.");
        return ExitCode::FAILURE;
    };

    // Web build first so cargo steps embed real assets
    println!("\n=== Building web frontend ===");
    if web() != ExitCode::SUCCESS {
        eprintln!("FAILED: Web frontend build");
        return ExitCode::FAILURE;
    }

    let cargo_steps: &[(&str, &[&str])] = &[
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

    for (label, cmd) in cargo_steps {
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

    // Release verification: ensure release build embeds real assets
    println!("\n=== Release verification ===");
    println!("Building release binary...");
    let status = Command::new("cargo")
        .args(["build", "-p", "mycad-cli", "--release"])
        .status()
        .expect("failed to execute cargo build --release");
    if !status.success() {
        eprintln!("FAILED: Release build");
        return ExitCode::FAILURE;
    }

    println!("Running release smoke test...");
    let status = Command::new("cargo")
        .args(["test", "-p", "mycad-api", "--release", "static_assets"])
        .status()
        .expect("failed to execute release smoke test");
    if !status.success() {
        eprintln!("FAILED: Release smoke test");
        return ExitCode::FAILURE;
    }

    // Playwright end-to-end tests
    println!("\n=== Running Playwright tests ===");
    let Some(_npx) = which("npx") else {
        eprintln!("FAILED: 'npx' not found. Ensure Node.js >= 20 is installed.");
        return ExitCode::FAILURE;
    };
    let pw_web_dir = workspace_root().join("web");
    let pw_status = Command::new("npx")
        .args(["playwright", "test"])
        .current_dir(&pw_web_dir)
        .status()
        .expect("failed to execute playwright test");
    if !pw_status.success() {
        eprintln!("FAILED: Playwright tests");
        return ExitCode::FAILURE;
    }

    // 3ai シェルスクリプトのユニットテスト
    println!("\n=== Running 3ai shell tests ===");
    let Some(bats) = which("bats") else {
        eprintln!("FAILED: 'bats' not found. Install with: sudo apt install bats");
        return ExitCode::FAILURE;
    };
    let tests_dir = workspace_root().join("tests/3ai");
    let status = Command::new(bats)
        .args(["dispatch-glm.bats", "state.bats", "guard-crates.bats"])
        .current_dir(&tests_dir)
        .status()
        .expect("failed to execute bats");
    if !status.success() {
        eprintln!("FAILED: 3ai shell tests");
        return ExitCode::FAILURE;
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

fn which(name: &str) -> Option<PathBuf> {
    let paths = std::env::var("PATH").ok()?;
    for dir in std::env::split_paths(&paths) {
        let candidate = dir.join(name);
        if candidate.is_file() {
            return Some(candidate);
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::sync::{Mutex, OnceLock};

    static TEST_LOCK: OnceLock<Mutex<()>> = OnceLock::new();

    fn export_to_temp_dir() -> PathBuf {
        let _guard = TEST_LOCK.get_or_init(|| Mutex::new(())).lock().unwrap();

        use mycad_api::transport::BodyMesh;
        use mycad_api::ErrorResponse;
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
        BodyMesh::export_all(&cfg).unwrap();
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
            "SketchPlane.ts",
            "SketchSegment.ts",
            "Transform.ts",
            "ComponentRef.ts",
            "EntityKind.ts",
            "EntityRef.ts",
            "TriangleMesh.ts",
            "BodyMesh.ts",
            "ErrorResponse.ts",
        ];

        for name in &files {
            let c1 = read_generated(&dir1, name);
            let c2 = read_generated(&dir2, name);
            assert_eq!(c1, c2, "{name} differs between runs");
        }
    }

    static FEATURE_GOLDEN: &str = r#"// This file was generated by [ts-rs](https://github.com/Aleph-Alpha/ts-rs). Do not edit this file manually.
import type { SketchPlane } from "./SketchPlane";
import type { SketchSegment } from "./SketchSegment";

/**
 * A feature — one step in the operation history.
 */
export type Feature = { "type": "create_box", id: string, width: number, height: number, depth: number, } | { "type": "create_cylinder", id: string, radius: number, height: number, origin?: [number, number, number], } | { "type": "create_sphere", id: string, radius: number, center?: [number, number, number], } | { "type": "create_sketch", id: string, plane: SketchPlane, offset?: number, profile: Array<SketchSegment>, } | { "type": "extrude", id: string, sketch: string, depth: number, fuse_target?: string | null, } | { "type": "extrude_cut", id: string, sketch: string, depth: number, target: string, } | { "type": "cut", id: string, target: string, tool: string, } | { "type": "fuse", id: string, target: string, tool: string, } | { "type": "intersect", id: string, target: string, tool: string, };
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
            actual.contains(r#""type": "create_sketch""#),
            "missing create_sketch tag"
        );
        assert!(
            actual.contains(r#""type": "extrude""#),
            "missing extrude tag"
        );
        assert!(
            actual.contains(r#""type": "extrude_cut""#),
            "missing extrude_cut tag"
        );
        assert!(actual.contains(r#""type": "cut""#), "missing cut tag");
        assert!(actual.contains(r#""type": "fuse""#), "missing fuse tag");
        assert!(
            actual.contains(r#""type": "intersect""#),
            "missing intersect tag"
        );
    }

    static TRIANGLE_MESH_GOLDEN: &str = "// This file was generated by [ts-rs](https://github.com/Aleph-Alpha/ts-rs). Do not edit this file manually.\n\n/**\n * A triangle mesh for rendering.\n */\nexport type TriangleMesh = { \n/**\n * Vertex positions (x, y, z).\n */\npositions: Array<[number, number, number]>, \n/**\n * Normal vectors per vertex.\n */\nnormals: Array<[number, number, number]>, \n/**\n * Triangle indices (every 3 indices form a triangle).\n */\nindices: Array<number>, \n/**\n * Per-triangle face id string. Length always equals `triangle_count()`.\n * Unnamed faces (`Face.name == None`) produce an empty string.\n */\nface_ids: Array<string>, };\n";

    #[test]
    fn t03_triangle_mesh() {
        let dir = export_to_temp_dir();
        let actual = read_generated(&dir, "TriangleMesh.ts");
        assert_eq!(actual, TRIANGLE_MESH_GOLDEN);
    }

    static DOCUMENT_GOLDEN: &str = "// This file was generated by [ts-rs](https://github.com/Aleph-Alpha/ts-rs). Do not edit this file manually.\nimport type { Component } from \"./Component\";\n\n/**\n * The top-level document representing a MyCad design file.\n */\nexport type Document = { \n/**\n * Format schema version. Increment when the .mycad file format changes in a breaking way.\n */\nschema_version: number, \n/**\n * Kernel version that created this document.\n */\nversion: string, \n/**\n * The root component (assembly or single part).\n */\nroot_component: Component, };\n";

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

    static ERROR_RESPONSE_GOLDEN: &str = r#"// This file was generated by [ts-rs](https://github.com/Aleph-Alpha/ts-rs). Do not edit this file manually.

export type ErrorResponse = { error: string, };
"#;

    #[test]
    fn t09_api_transport_dto() {
        let dir = export_to_temp_dir();
        let err_resp = read_generated(&dir, "ErrorResponse.ts");
        assert_eq!(err_resp, ERROR_RESPONSE_GOLDEN);
    }

    static ENTITY_REF_GOLDEN: &str = "// This file was generated by [ts-rs](https://github.com/Aleph-Alpha/ts-rs). Do not edit this file manually.\nimport type { EntityKind } from \"./EntityKind\";\n\n/**\n * A reference to a topological entity, either directly named or derived from an operation.\n */\nexport type EntityRef = { \"ref\": \"named\", feature_id: string, kind: EntityKind, role: string, } | { \"ref\": \"derived\", kind: EntityKind, op: string, from: Array<EntityRef>, selector: string, };\n";

    #[test]
    fn t10_entity_ref_explicit_root() {
        let dir = export_to_temp_dir();
        let actual = read_generated(&dir, "EntityRef.ts");
        assert_eq!(actual, ENTITY_REF_GOLDEN);
    }
    static BODY_MESH_GOLDEN: &str = "// This file was generated by [ts-rs](https://github.com/Aleph-Alpha/ts-rs). Do not edit this file manually.\nimport type { TriangleMesh } from \"./TriangleMesh\";\n\nexport type BodyMesh = { feature_id: string, mesh: TriangleMesh, };\n";

    #[test]
    fn t15_body_mesh_golden() {
        let dir = export_to_temp_dir();
        let actual = read_generated(&dir, "BodyMesh.ts");
        assert_eq!(actual, BODY_MESH_GOLDEN);
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
            "EntityKind.ts",
            "EntityRef.ts",
            "TriangleMesh.ts",
            "BodyMesh.ts",
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
            "SketchPlane.ts",
            "SketchSegment.ts",
            "Transform.ts",
            "ComponentRef.ts",
            "EntityKind.ts",
            "EntityRef.ts",
            "TriangleMesh.ts",
            "ErrorResponse.ts",
            "BodyMesh.ts",
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

    /// T07: gen_ts_to() が workspace_root / web/src/generated を経由せず
    /// 指定ディレクトリへ実際にファイルを書き出すことを検証する（実経路テスト）。
    #[test]
    fn t07_real_path_gen_ts_to() {
        let _guard = TEST_LOCK.get_or_init(|| Mutex::new(())).lock().unwrap();
        let dir = tempfile::tempdir().unwrap();
        let code = gen_ts_to(dir.path());
        assert_eq!(code, ExitCode::SUCCESS, "gen_ts_to should succeed");
        for name in &[
            "Document.ts",
            "Feature.ts",
            "TriangleMesh.ts",
            "BodyMesh.ts",
            "ErrorResponse.ts",
        ] {
            assert!(
                dir.path().join(name).exists(),
                "{name} missing from gen_ts_to output"
            );
        }
    }
}
