use axum::body::Body;
use axum::http::{Request, StatusCode};
use mycad_api::router::app;
use serde::Deserialize;
use tower::ServiceExt;

#[derive(Deserialize)]
struct ErrorResponse {
    error: String,
}

fn fixture_path(name: &str) -> String {
    let dir = std::env::var("CARGO_MANIFEST_DIR").unwrap();
    let path = std::path::Path::new(&dir)
        .join("tests")
        .join("fixtures")
        .join(name);
    std::fs::canonicalize(&path)
        .unwrap_or_else(|_| panic!("fixture not found: {:?}", path))
        .to_str()
        .unwrap()
        .to_string()
}

fn example_path(name: &str) -> String {
    let dir = std::env::var("CARGO_MANIFEST_DIR").unwrap();
    let path = std::path::Path::new(&dir)
        .join("..")
        .join("..")
        .join("examples")
        .join(name);
    std::fs::canonicalize(&path)
        .unwrap_or_else(|_| panic!("example not found: {:?}", path))
        .to_str()
        .unwrap()
        .to_string()
}

async fn send_mesh_request(uri: &str) -> (StatusCode, String) {
    let req = Request::builder().uri(uri).body(Body::empty()).unwrap();
    let app = app();
    let resp = app.oneshot(req).await.unwrap();
    let status = resp.status();
    let body = axum::body::to_bytes(resp.into_body(), usize::MAX)
        .await
        .unwrap();
    (status, String::from_utf8(body.to_vec()).unwrap())
}

// T01: normal box
#[tokio::test]
async fn t01_normal_box() {
    let file = example_path("simple_box.mycad");
    let uri = format!("/api/v0/mesh?file={}", urlencoding(&file));
    let (status, body) = send_mesh_request(&uri).await;
    assert_eq!(status, StatusCode::OK, "body: {body}");
    let mesh: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert!(mesh["positions"].as_array().unwrap().len() > 0);
    assert!(mesh["indices"].as_array().unwrap().len() % 3 == 0);
}

// T02: normal cylinder
#[tokio::test]
async fn t02_normal_cylinder() {
    let file = example_path("cylinder.mycad");
    let uri = format!("/api/v0/mesh?file={}", urlencoding(&file));
    let (status, body) = send_mesh_request(&uri).await;
    assert_eq!(status, StatusCode::OK, "body: {body}");
    let mesh: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert!(mesh["positions"].as_array().unwrap().len() > 0);
}

// T03: determinism
#[tokio::test]
async fn t03_determinism() {
    let file = example_path("simple_box.mycad");
    let uri = format!("/api/v0/mesh?file={}", urlencoding(&file));

    let (_, body1) = send_mesh_request(&uri).await;
    let (_, body2) = send_mesh_request(&uri).await;
    assert_eq!(body1, body2, "mesh responses must be deterministic");

    let file2 = example_path("cylinder.mycad");
    let uri2 = format!("/api/v0/mesh?file={}", urlencoding(&file2));
    let (_, body3) = send_mesh_request(&uri2).await;
    let (_, body4) = send_mesh_request(&uri2).await;
    assert_eq!(
        body3, body4,
        "cylinder mesh responses must be deterministic"
    );
}

// T04: 404 not found
#[tokio::test]
async fn t04_not_found() {
    let uri = "/api/v0/mesh?file=/tmp/absolutely_nonexistent_file.mycad";
    let (status, body) = send_mesh_request(uri).await;
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
    let abs = std::fs::canonicalize(&path)
        .unwrap()
        .to_str()
        .unwrap()
        .to_string();
    let uri = format!("/api/v0/mesh?file={}", urlencoding(&abs));
    let (status, body) = send_mesh_request(&uri).await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "body: {body}");
}

// T06: 403 Host header (DNS rebinding protection)
#[tokio::test]
async fn t06_host_header_rebinding() {
    let file = example_path("simple_box.mycad");
    let uri = format!("/api/v0/mesh?file={}", urlencoding(&file));
    let req = Request::builder()
        .uri(&uri)
        .header("Host", "evil.com")
        .body(Body::empty())
        .unwrap();
    let app = app();
    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::FORBIDDEN);
}

// T07: 422 unsupported feature (sphere)
#[tokio::test]
async fn t07_unsupported_feature() {
    let file = fixture_path("create_sphere.mycad");
    let uri = format!("/api/v0/mesh?file={}", urlencoding(&file));
    let (status, body) = send_mesh_request(&uri).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "body: {body}");
}

// T08: 422 degenerate dimension (zero-width box)
#[tokio::test]
async fn t08_degenerate_dimension() {
    let file = fixture_path("zero_box.mycad");
    let uri = format!("/api/v0/mesh?file={}", urlencoding(&file));
    let (status, body) = send_mesh_request(&uri).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "body: {body}");
}

// T09: 422 assembly unsupported
#[tokio::test]
async fn t09_assembly_unsupported() {
    let file = fixture_path("assembly.mycad");
    let uri = format!("/api/v0/mesh?file={}", urlencoding(&file));
    let (status, body) = send_mesh_request(&uri).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "body: {body}");
    let err: ErrorResponse = serde_json::from_str(&body).unwrap();
    assert!(
        err.error.contains("assembly") || err.error.contains("reference"),
        "expected assembly/reference message, got: {}",
        err.error
    );
}

// T10: 422 empty part
#[tokio::test]
async fn t10_empty_part() {
    let file = fixture_path("empty_part.mycad");
    let uri = format!("/api/v0/mesh?file={}", urlencoding(&file));
    let (status, body) = send_mesh_request(&uri).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "body: {body}");
    let err: ErrorResponse = serde_json::from_str(&body).unwrap();
    assert!(
        err.error.contains("empty part") || err.error.contains("no features"),
        "expected empty part message, got: {}",
        err.error
    );
}

// T11: 400 relative path rejected
#[tokio::test]
async fn t11_relative_path_rejected() {
    let uri = "/api/v0/mesh?file=examples/simple_box.mycad";
    let (status, body) = send_mesh_request(uri).await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "body: {body}");
    let err: ErrorResponse = serde_json::from_str(&body).unwrap();
    assert!(
        err.error.contains("relative"),
        "expected relative path error, got: {}",
        err.error
    );
}

fn urlencoding(s: &str) -> String {
    s.replace('/', "%2F")
}
