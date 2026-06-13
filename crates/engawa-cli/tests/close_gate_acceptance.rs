//! Acceptance tests for Issue #78 — Phase 5 close gate.

use std::path::PathBuf;
use std::process::Command;
use std::sync::Arc;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use engawa_api::router::app;
use engawa_kernel::tessellation::TriangleMesh;
use serde::Deserialize;
use tower::ServiceExt;

fn example_path(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("examples")
        .join(name)
}

#[derive(Deserialize)]
struct BodyMesh {
    #[allow(dead_code)]
    feature_id: String,
    #[allow(dead_code)]
    mesh: TriangleMesh,
}

/// T01: `mycad export examples/assembly.engawa` exits with code 0.
#[test]
fn t01_export_exits_zero() {
    let bin = env!("CARGO_BIN_EXE_engawa");
    let input = example_path("assembly.engawa");
    let tmp = tempfile::NamedTempFile::with_suffix(".stl").expect("tempfile");

    let status = Command::new(bin)
        .arg("export")
        .arg(&input)
        .arg("-o")
        .arg(tmp.path())
        .status()
        .expect("run mycad export");

    assert!(status.success(), "assembly export must exit 0");
}

/// T02: Generated STL file is at least 1 KB.
#[test]
fn t02_stl_file_size_gt_1kb() {
    let bin = env!("CARGO_BIN_EXE_engawa");
    let input = example_path("assembly.engawa");
    let tmp = tempfile::NamedTempFile::with_suffix(".stl").expect("tempfile");

    let status = Command::new(bin)
        .arg("export")
        .arg(&input)
        .arg("-o")
        .arg(tmp.path())
        .status()
        .expect("run mycad export");
    assert!(status.success(), "export must succeed before size check");

    let metadata = std::fs::metadata(tmp.path()).expect("stat stl");
    assert!(
        metadata.len() >= 1024,
        "STL must be >= 1024 bytes, got {}",
        metadata.len()
    );
}

/// T03: HTTP server returns 200 at `/api/v0/mesh` with non-empty bodies array.
#[tokio::test]
async fn t03_http_mesh_returns_200() {
    let file = example_path("assembly.engawa");
    let req = Request::builder()
        .uri("/api/v0/mesh")
        .body(Body::empty())
        .unwrap();

    let resp = app(Arc::new(file)).oneshot(req).await.unwrap();
    let status = resp.status();
    assert_eq!(status, StatusCode::OK);

    let bytes = axum::body::to_bytes(resp.into_body(), usize::MAX)
        .await
        .unwrap();
    let bodies: Vec<BodyMesh> = serde_json::from_slice(&bytes).unwrap();
    assert!(!bodies.is_empty(), "bodies array must not be empty");
}

/// T04 (boundary): Exporting a nonexistent file exits with non-zero code.
#[test]
fn t04_boundary_missing_file_nonzero_exit() {
    let bin = env!("CARGO_BIN_EXE_engawa");
    let input = PathBuf::from("/tmp/absolutely_nonexistent_78_close_gate.engawa");
    let tmp = tempfile::NamedTempFile::with_suffix(".stl").expect("tempfile");

    let output = Command::new(bin)
        .arg("export")
        .arg(&input)
        .arg("-o")
        .arg(tmp.path())
        .output()
        .expect("run mycad export");

    assert!(
        !output.status.success(),
        "nonexistent file must cause non-zero exit"
    );
}
