use axum::body::Body;
use axum::http::{Request, StatusCode};
use engawa_api::router::app;
use engawa_kernel::tessellation::TriangleMesh;
use serde::Deserialize;
use std::path::PathBuf;
use std::sync::Arc;
use tower::ServiceExt;

#[derive(Deserialize)]
struct ErrorResponse {
    error: String,
}

#[derive(Deserialize)]
struct BodyMesh {
    #[allow(dead_code)]
    feature_id: String,
    #[allow(dead_code)]
    mesh: TriangleMesh,
}

fn example_path(name: &str) -> PathBuf {
    let dir = std::env::var("CARGO_MANIFEST_DIR").unwrap();
    let path = std::path::Path::new(&dir)
        .join("..")
        .join("..")
        .join("examples")
        .join(name);
    std::fs::canonicalize(&path).unwrap_or_else(|_| panic!("example not found: {:?}", path))
}

fn fixture_path(name: &str) -> PathBuf {
    let dir = std::env::var("CARGO_MANIFEST_DIR").unwrap();
    let path = std::path::Path::new(&dir)
        .join("tests")
        .join("fixtures")
        .join(name);
    std::fs::canonicalize(&path).unwrap_or_else(|_| panic!("fixture not found: {:?}", path))
}

async fn send_mesh_request(file: PathBuf) -> (StatusCode, String) {
    let req = Request::builder()
        .uri("/api/v0/mesh")
        .body(Body::empty())
        .unwrap();
    let resp = app(Arc::new(file)).oneshot(req).await.unwrap();
    let status = resp.status();
    let bytes = axum::body::to_bytes(resp.into_body(), usize::MAX)
        .await
        .unwrap();
    (status, String::from_utf8(bytes.to_vec()).unwrap())
}

/// T03: Assembly API returns HTTP 200 with non-empty bodies.
#[tokio::test]
async fn t03_assembly_api_returns_200_nonempty() {
    let file = example_path("assembly.engawa");
    let (status, body) = send_mesh_request(file).await;
    assert_eq!(status, StatusCode::OK, "body: {body}");
    let bodies: Vec<BodyMesh> = serde_json::from_str(&body).unwrap();
    assert!(!bodies.is_empty(), "bodies must not be empty");
}

/// T04: Empty assembly (no features/children/reference) returns HTTP 422.
#[tokio::test]
async fn t04_boundary_empty_assembly_returns_422() {
    let file = fixture_path("empty_part.engawa");
    let (status, body) = send_mesh_request(file).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "body: {body}");
    let err: ErrorResponse = serde_json::from_str(&body).unwrap();
    assert!(
        err.error.contains("empty") || err.error.contains("no bodies"),
        "expected empty/no bodies message, got: {}",
        err.error
    );
}

// ---------------------------------------------------------------------------
// Edge-case tests (adversarial persona)
// ---------------------------------------------------------------------------

/// Determinism 100x: 100 consecutive API requests for assembly produce identical response.
#[tokio::test]
async fn edge_determinism_100x_assembly_api() {
    let file = example_path("assembly.engawa");
    let (_, reference) = send_mesh_request(file.clone()).await;

    for i in 1..=99 {
        let (_, body) = send_mesh_request(file.clone()).await;
        assert_eq!(body, reference, "API response differs at run {i}");
    }
}

/// Determinism 100x: simple_box also produces identical API responses 100 times.
#[tokio::test]
async fn edge_determinism_100x_simple_box_api() {
    let file = example_path("simple_box.engawa");
    let (_, reference) = send_mesh_request(file.clone()).await;

    for i in 1..=99 {
        let (_, body) = send_mesh_request(file.clone()).await;
        assert_eq!(
            body, reference,
            "simple box API response differs at run {i}"
        );
    }
}

/// Empty children assembly: children exist but no features in any child → 422.
#[tokio::test]
async fn edge_empty_children_assembly_returns_422() {
    let file = fixture_path("empty_children_assembly.engawa");
    let (status, body) = send_mesh_request(file).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "body: {body}");
    let err: ErrorResponse = serde_json::from_str(&body).unwrap();
    assert!(
        err.error.contains("empty") || err.error.contains("no bodies"),
        "expected empty/no bodies message, got: {}",
        err.error
    );
}

/// Deep nested assembly: 3 levels of nesting with a leaf box → 200 + non-empty.
#[tokio::test]
async fn edge_deep_nested_assembly_returns_200() {
    let file = fixture_path("deep_nested_assembly.engawa");
    let (status, body) = send_mesh_request(file).await;
    assert_eq!(status, StatusCode::OK, "body: {body}");
    let bodies: Vec<BodyMesh> = serde_json::from_str(&body).unwrap();
    assert_eq!(bodies.len(), 1, "deep nested should produce 1 body");
    assert!(
        !bodies[0].mesh.positions.is_empty(),
        "deep nested mesh must not be empty"
    );
}

/// Deep nested determinism: two requests produce identical output.
#[tokio::test]
async fn edge_deep_nested_determinism() {
    let file = fixture_path("deep_nested_assembly.engawa");
    let (_, body1) = send_mesh_request(file.clone()).await;
    let (_, body2) = send_mesh_request(file).await;
    assert_eq!(
        body1, body2,
        "deep nested API response must be deterministic"
    );
}

/// Sibling assembly: two sibling components with transform → 200 + 2 bodies.
#[tokio::test]
async fn edge_siblings_assembly_returns_200_with_two_bodies() {
    let file = fixture_path("siblings_assembly.engawa");
    let (status, body) = send_mesh_request(file).await;
    assert_eq!(status, StatusCode::OK, "body: {body}");
    let bodies: Vec<BodyMesh> = serde_json::from_str(&body).unwrap();
    assert_eq!(bodies.len(), 2, "sibling assembly should produce 2 bodies");
    assert!(
        !bodies[0].mesh.positions.is_empty(),
        "first body mesh non-empty"
    );
    assert!(
        !bodies[1].mesh.positions.is_empty(),
        "second body mesh non-empty"
    );
}

/// Negative dimension box in assembly → 422 (kernel error, not panic).
#[tokio::test]
async fn edge_negative_box_assembly_returns_error() {
    let file = fixture_path("negative_box_assembly.engawa");
    let (status, body) = send_mesh_request(file).await;
    assert_eq!(
        status,
        StatusCode::UNPROCESSABLE_ENTITY,
        "negative box should return 422, body: {body}"
    );
    let err: ErrorResponse = serde_json::from_str(&body).unwrap();
    assert!(!err.error.is_empty(), "error message must not be empty");
}

/// Assembly YAML roundtrip: load → serialize → deserialize → API produces same result.
#[tokio::test]
async fn edge_assembly_yaml_roundtrip_api() {
    use engawa_format::Document;

    let file = example_path("assembly.engawa");
    let doc = Document::from_path(&file).unwrap();

    // Roundtrip: serialize → deserialize
    let yaml = doc.to_yaml().unwrap();
    let doc2 = Document::from_yaml(&yaml).unwrap();

    // Write roundtripped doc to temp file
    let tmp = tempfile::NamedTempFile::with_suffix(".engawa").expect("tempfile");
    std::fs::write(tmp.path(), doc2.to_yaml().unwrap()).expect("write roundtrip yaml");

    // Both original and roundtripped produce identical API responses
    let (_, body_orig) = send_mesh_request(file).await;
    let (_, body_rt) = send_mesh_request(tmp.path().to_path_buf()).await;
    assert_eq!(
        body_orig, body_rt,
        "roundtripped assembly must produce identical API response"
    );
}

/// Simple box YAML roundtrip through API produces identical result.
#[tokio::test]
async fn edge_simple_box_yaml_roundtrip_api() {
    use engawa_format::Document;

    let file = example_path("simple_box.engawa");
    let doc = Document::from_path(&file).unwrap();
    let yaml = doc.to_yaml().unwrap();
    let doc2 = Document::from_yaml(&yaml).unwrap();

    let tmp = tempfile::NamedTempFile::with_suffix(".engawa").expect("tempfile");
    std::fs::write(tmp.path(), doc2.to_yaml().unwrap()).expect("write roundtrip yaml");

    let (_, body_orig) = send_mesh_request(file).await;
    let (_, body_rt) = send_mesh_request(tmp.path().to_path_buf()).await;
    assert_eq!(
        body_orig, body_rt,
        "roundtripped simple_box must produce identical API response"
    );
}
