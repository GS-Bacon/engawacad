use axum::body::Body;
use axum::http::{Request, StatusCode};
use mycad_api::router::app;
use mycad_kernel::tessellation::TriangleMesh;
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
    feature_id: String,
    mesh: TriangleMesh,
}

fn fixture_path(name: &str) -> PathBuf {
    let dir = std::env::var("CARGO_MANIFEST_DIR").unwrap();
    let path = std::path::Path::new(&dir)
        .join("tests")
        .join("fixtures")
        .join(name);
    std::fs::canonicalize(&path).unwrap_or_else(|_| panic!("fixture not found: {:?}", path))
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

fn make_app(file: PathBuf) -> axum::Router {
    app(Arc::new(file))
}

async fn send_mesh_request(file: PathBuf) -> (StatusCode, String) {
    let req = Request::builder()
        .uri("/api/v0/mesh")
        .body(Body::empty())
        .unwrap();
    let app = make_app(file);
    let resp = app.oneshot(req).await.unwrap();
    let status = resp.status();
    let body = axum::body::to_bytes(resp.into_body(), usize::MAX)
        .await
        .unwrap();
    (status, String::from_utf8(body.to_vec()).unwrap())
}

// T01: normal box — now returns Vec<BodyMesh> with len==1
#[tokio::test]
async fn t01_normal_box() {
    let file = example_path("simple_box.mycad");
    let (status, body) = send_mesh_request(file).await;
    assert_eq!(status, StatusCode::OK, "body: {body}");
    let bodies: Vec<BodyMesh> = serde_json::from_str(&body).unwrap();
    assert_eq!(bodies.len(), 1);
    assert_eq!(bodies[0].feature_id, "box_1");
    assert!(
        !bodies[0].mesh.positions.is_empty(),
        "positions must not be empty"
    );
    assert!(
        bodies[0].mesh.indices.len().is_multiple_of(3),
        "indices count must be multiple of 3"
    );
}

// T02: normal cylinder
#[tokio::test]
async fn t02_normal_cylinder() {
    let file = example_path("cylinder.mycad");
    let (status, body) = send_mesh_request(file).await;
    assert_eq!(status, StatusCode::OK, "body: {body}");
    let bodies: Vec<BodyMesh> = serde_json::from_str(&body).unwrap();
    assert_eq!(bodies.len(), 1);
    assert!(
        !bodies[0].mesh.positions.is_empty(),
        "positions must not be empty"
    );
    assert!(
        bodies[0].mesh.indices.len().is_multiple_of(3),
        "indices count must be multiple of 3"
    );
}

// T03: determinism
#[tokio::test]
async fn t03_determinism() {
    let file = example_path("simple_box.mycad");
    let (_, body1) = send_mesh_request(file.clone()).await;
    let (_, body2) = send_mesh_request(file).await;
    assert_eq!(body1, body2, "mesh responses must be deterministic");

    let file2 = example_path("cylinder.mycad");
    let (_, body3) = send_mesh_request(file2.clone()).await;
    let (_, body4) = send_mesh_request(file2).await;
    assert_eq!(
        body3, body4,
        "cylinder mesh responses must be deterministic"
    );
}

// T04: 404 not found — nonexistent file in State triggers error at load time
#[tokio::test]
async fn t04_not_found() {
    let file = PathBuf::from("/tmp/absolutely_nonexistent_file.mycad");
    let req = Request::builder()
        .uri("/api/v0/mesh")
        .body(Body::empty())
        .unwrap();
    let app = app(Arc::new(file));
    let resp = app.oneshot(req).await.unwrap();
    let status = resp.status();
    let body_bytes = axum::body::to_bytes(resp.into_body(), usize::MAX)
        .await
        .unwrap();
    let body = String::from_utf8(body_bytes.to_vec()).unwrap();
    assert_eq!(status, StatusCode::NOT_FOUND, "body: {body}");
    let err: ErrorResponse = serde_json::from_str(&body).unwrap();
    assert!(!err.error.is_empty());
}

// T05: 400 non-.mycad extension
#[tokio::test]
async fn t05_invalid_extension() {
    let dir = std::env::var("CARGO_MANIFEST_DIR").unwrap();
    let path = std::path::Path::new(&dir)
        .join("..")
        .join("..")
        .join("Cargo.toml");
    let abs = std::fs::canonicalize(&path).unwrap();

    let req = Request::builder()
        .uri("/api/v0/mesh")
        .body(Body::empty())
        .unwrap();
    let app = app(Arc::new(abs));
    let resp = app.oneshot(req).await.unwrap();
    let status = resp.status();
    let body_bytes = axum::body::to_bytes(resp.into_body(), usize::MAX)
        .await
        .unwrap();
    let body = String::from_utf8(body_bytes.to_vec()).unwrap();
    assert_eq!(status, StatusCode::BAD_REQUEST, "body: {body}");
    let err: ErrorResponse = serde_json::from_str(&body).unwrap();
    assert!(!err.error.is_empty());
    let lower = err.error.to_lowercase();
    assert!(
        lower.contains("extension") || lower.contains(".mycad") || lower.contains("invalid"),
        "expected extension-related message, got: {}",
        err.error
    );
}

// T06: 403 Host header (DNS rebinding protection)
#[tokio::test]
async fn t06_host_header_rebinding() {
    let file = example_path("simple_box.mycad");
    let req = Request::builder()
        .uri("/api/v0/mesh")
        .header("Host", "evil.com")
        .body(Body::empty())
        .unwrap();
    let resp = make_app(file).oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::FORBIDDEN);
}

// T07: Extrude success — 200 + mesh with 12 triangles.
#[tokio::test]
async fn t07_extrude_success() {
    let file = fixture_path("extrude.mycad");
    let (status, body) = send_mesh_request(file).await;
    assert_eq!(status, StatusCode::OK, "body: {body}");
    let bodies: Vec<BodyMesh> = serde_json::from_str(&body).unwrap();
    assert_eq!(bodies.len(), 1);
    assert_eq!(
        bodies[0].mesh.indices.len() / 3,
        12,
        "extrude should produce 12 triangles (2 caps × 2 + 4 sides × 2)"
    );
}

// T08: 422 degenerate dimension (zero-width box)
#[tokio::test]
async fn t08_degenerate_dimension() {
    let file = fixture_path("zero_box.mycad");
    let (status, body) = send_mesh_request(file).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "body: {body}");
    let err: ErrorResponse = serde_json::from_str(&body).unwrap();
    assert!(!err.error.is_empty(), "error message must not be empty");
}

// T09: Assembly now supported — API returns 200 with non-empty bodies.
#[tokio::test]
async fn t09_assembly_supported() {
    let file = fixture_path("assembly.mycad");
    let (status, body) = send_mesh_request(file).await;
    assert_eq!(status, StatusCode::OK, "body: {body}");
    let bodies: Vec<BodyMesh> = serde_json::from_str(&body).unwrap();
    assert!(!bodies.is_empty(), "bodies must not be empty");
}

// T10: 422 empty part — bodies.is_empty() guard returns "empty assembly: no bodies built"
#[tokio::test]
async fn t10_empty_part() {
    let file = fixture_path("empty_part.mycad");
    let (status, body) = send_mesh_request(file).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "body: {body}");
    let err: ErrorResponse = serde_json::from_str(&body).unwrap();
    assert!(
        err.error.contains("empty") || err.error.contains("no bodies"),
        "expected empty/no bodies message, got: {}",
        err.error
    );
}

// T12: API multi-body — two_bodies.mycad returns Vec<BodyMesh> with exact order
#[tokio::test]
async fn t12_multi_body() {
    let file = example_path("two_bodies.mycad");
    let (status, body) = send_mesh_request(file).await;
    assert_eq!(status, StatusCode::OK, "body: {body}");
    let bodies: Vec<BodyMesh> = serde_json::from_str(&body).unwrap();
    assert_eq!(bodies.len(), 2);
    assert_eq!(bodies[0].feature_id, "body_a");
    assert_eq!(bodies[1].feature_id, "body_b");
    assert!(
        !bodies[0].mesh.positions.is_empty(),
        "body_a mesh non-empty"
    );
    assert!(
        !bodies[1].mesh.positions.is_empty(),
        "body_b mesh non-empty"
    );
}

// T13: API single body — existing examples return len==1 with correct feature_id
#[tokio::test]
async fn t13_single_body() {
    let file = example_path("simple_box.mycad");
    let (status, body) = send_mesh_request(file).await;
    assert_eq!(status, StatusCode::OK, "body: {body}");
    let bodies: Vec<BodyMesh> = serde_json::from_str(&body).unwrap();
    assert_eq!(bodies.len(), 1);
    assert_eq!(bodies[0].feature_id, "box_1");
    assert_eq!(bodies[0].mesh.indices.len() / 3, 12);
}

// T14: API determinism — same request produces identical response body
#[tokio::test]
async fn t14_api_determinism() {
    let file = example_path("two_bodies.mycad");
    let (_, body1) = send_mesh_request(file.clone()).await;
    let (_, body2) = send_mesh_request(file).await;
    assert_eq!(body1, body2, "API responses must be deterministic");
}

// T17: Sphere API success — 200 + mesh with 960 triangles.
#[tokio::test]
async fn t17_sphere_success() {
    let file = example_path("sphere.mycad");
    let (status, body) = send_mesh_request(file).await;
    assert_eq!(status, StatusCode::OK, "body: {body}");
    let bodies: Vec<BodyMesh> = serde_json::from_str(&body).unwrap();
    assert_eq!(bodies.len(), 1);
    assert_eq!(
        bodies[0].mesh.indices.len() / 3,
        960,
        "sphere should produce 960 triangles"
    );
}

// T18: Extrude determinism — two requests produce identical mesh.
#[tokio::test]
async fn t18_extrude_determinism() {
    let file = fixture_path("extrude.mycad");
    let (_, body1) = send_mesh_request(file.clone()).await;
    let (_, body2) = send_mesh_request(file).await;
    assert_eq!(body1, body2, "extrude mesh responses must be deterministic");
}

// T19: Boolean cut — API returns only the live result body (consumed bodies excluded).
// Regression guard for #51 (handler.rs was using bodies.all() instead of bodies.live()).
#[tokio::test]
async fn t19_boolean_cut_live_bodies_only() {
    let file = example_path("boolean_box_cut.mycad");
    let (status, body) = send_mesh_request(file).await;
    assert_eq!(status, StatusCode::OK, "body: {body}");
    let bodies: Vec<BodyMesh> = serde_json::from_str(&body).unwrap();
    // boolean_box_cut.mycad: target + tool + cut1 → consumed=2, live=1
    assert_eq!(
        bodies.len(),
        1,
        "only the live result body should be returned"
    );
    assert_eq!(bodies[0].feature_id, "cut1");
    assert!(
        !bodies[0].mesh.positions.is_empty(),
        "result mesh non-empty"
    );
}
