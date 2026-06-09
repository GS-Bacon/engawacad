/// Acceptance tests for GET /api/v0/features (Issue #102 / #100)
use axum::body::Body;
use axum::http::{Request, StatusCode};
use mycad_api::router::app;
use std::path::PathBuf;
use std::sync::Arc;
use tower::ServiceExt;

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

async fn send_features_request(file: PathBuf) -> (StatusCode, String) {
    let req = Request::builder()
        .uri("/api/v0/features")
        .body(Body::empty())
        .unwrap();
    let app = app(Arc::new(file));
    let resp = app.oneshot(req).await.unwrap();
    let status = resp.status();
    let body = axum::body::to_bytes(resp.into_body(), usize::MAX)
        .await
        .unwrap();
    (status, String::from_utf8(body.to_vec()).unwrap())
}

/// T01: Empty features — GET /api/v0/features returns 200 + [] for a .mycad with no features.
#[tokio::test]
async fn t01_degen_empty_features() {
    let file = fixture_path("empty_part.mycad");
    let (status, body) = send_features_request(file).await;
    assert_eq!(status, StatusCode::OK, "body: {body}");
    let ids: Vec<String> = serde_json::from_str(&body).unwrap();
    assert!(ids.is_empty(), "expected empty array, got: {body}");
}

/// T02: Normal list — extruded_rect.mycad has sketch_1 + extrude_1.
#[tokio::test]
async fn t02_normal_list_features() {
    let file = example_path("extruded_rect.mycad");
    let (status, body) = send_features_request(file).await;
    assert_eq!(status, StatusCode::OK, "body: {body}");
    let ids: Vec<String> = serde_json::from_str(&body).unwrap();
    assert_eq!(ids, vec!["sketch_1", "extrude_1"]);
}

/// T03: Determinism — same request produces identical response.
#[tokio::test]
async fn t03_features_determinism() {
    let file = example_path("extruded_rect.mycad");
    let (_, body1) = send_features_request(file.clone()).await;
    let (_, body2) = send_features_request(file).await;
    assert_eq!(body1, body2, "features response must be deterministic");
}

/// T04: Determinism 100x — same request 100 times produces identical response.
#[tokio::test]
async fn t04_features_determinism_100x() {
    let file = example_path("extruded_rect.mycad");
    let (_, first_body) = send_features_request(file.clone()).await;
    for _ in 0..99 {
        let (_, body) = send_features_request(file.clone()).await;
        assert_eq!(
            body, first_body,
            "features response must be deterministic across 100 calls"
        );
    }
}

/// T05: Nonexistent file → 404 for GET /api/v0/features.
#[tokio::test]
async fn t05_features_not_found() {
    let file = PathBuf::from("/tmp/absolutely_nonexistent_file_for_features.mycad");
    let (status, body) = send_features_request(file).await;
    assert_eq!(status, StatusCode::NOT_FOUND, "body: {body}");
    let err: serde_json::Value = serde_json::from_str(&body).unwrap();
    assert!(
        !err["error"].as_str().unwrap_or("").is_empty(),
        "error message must be non-empty"
    );
}

/// T06: Single feature — simple_box.mycad has only box_1.
#[tokio::test]
async fn t06_features_single_feature() {
    let file = example_path("simple_box.mycad");
    let (status, body) = send_features_request(file).await;
    assert_eq!(status, StatusCode::OK, "body: {body}");
    let ids: Vec<String> = serde_json::from_str(&body).unwrap();
    assert_eq!(ids, vec!["box_1"]);
}

/// T07: Feature ordering preserved — extruded_rect.mycad lists sketch_1 then extrude_1.
#[tokio::test]
async fn t07_features_ordering_preserved() {
    let file = example_path("extruded_rect.mycad");
    let (status, body) = send_features_request(file).await;
    assert_eq!(status, StatusCode::OK, "body: {body}");
    let ids: Vec<String> = serde_json::from_str(&body).unwrap();
    assert!(ids.len() >= 2, "expected at least 2 features");
    // Verify insertion order is preserved (sketch before extrude)
    let sketch_pos = ids
        .iter()
        .position(|id| id == "sketch_1")
        .expect("sketch_1 must exist");
    let extrude_pos = ids
        .iter()
        .position(|id| id == "extrude_1")
        .expect("extrude_1 must exist");
    assert!(
        sketch_pos < extrude_pos,
        "sketch must appear before extrude in feature list"
    );
}

/// T08: Empty features 100x determinism — empty array consistently returned.
#[tokio::test]
async fn t08_empty_features_determinism_100x() {
    let file = fixture_path("empty_part.mycad");
    let (status, first_body) = send_features_request(file.clone()).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(first_body, "[]");
    for _ in 0..99 {
        let (_, body) = send_features_request(file.clone()).await;
        assert_eq!(body, "[]", "empty features must consistently return []");
    }
}

/// T09: No duplicate IDs — all feature IDs must be unique.
#[tokio::test]
async fn t09_features_no_duplicate_ids() {
    let file = example_path("extruded_rect.mycad");
    let (status, body) = send_features_request(file).await;
    assert_eq!(status, StatusCode::OK, "body: {body}");
    let ids: Vec<String> = serde_json::from_str(&body).unwrap();
    let unique: std::collections::HashSet<_> = ids.iter().collect();
    assert_eq!(
        ids.len(),
        unique.len(),
        "feature IDs must not contain duplicates"
    );
}
