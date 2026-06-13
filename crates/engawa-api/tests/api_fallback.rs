use axum::body::Body;
use axum::http::{Request, StatusCode};
use engawa_api::router::app;
use serde::Deserialize;
use std::path::PathBuf;
use std::sync::Arc;
use tower::ServiceExt;

#[derive(Deserialize)]
struct ErrorResponse {
    error: String,
}

fn make_app() -> axum::Router {
    // Fallback tests don't hit /mesh, so a placeholder path is fine
    app(Arc::new(PathBuf::from("/dev/null")))
}

async fn send_request(uri: &str) -> (StatusCode, String, String) {
    let req = Request::builder().uri(uri).body(Body::empty()).unwrap();
    let app = make_app();
    let resp = app.oneshot(req).await.unwrap();
    let status = resp.status();
    let ct = resp
        .headers()
        .get("content-type")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("")
        .to_string();
    let body = axum::body::to_bytes(resp.into_body(), usize::MAX)
        .await
        .unwrap();
    (status, String::from_utf8(body.to_vec()).unwrap(), ct)
}

// T12: GET /api/v0/unknown → 404 JSON
#[tokio::test]
async fn t12_api_unknown_path_json_404() {
    let (status, body, ct) = send_request("/api/v0/unknown").await;
    assert_eq!(status, StatusCode::NOT_FOUND, "body: {body}");
    assert!(
        ct.contains("application/json"),
        "expected json content-type, got: {ct}"
    );
    let err: ErrorResponse = serde_json::from_str(&body).unwrap();
    assert!(!err.error.is_empty(), "error must not be empty");
}

// T13: GET /api/v0/mesh/ → 404 JSON (trailing slash, no match)
#[tokio::test]
async fn t13_api_mesh_trailing_slash_json_404() {
    let (status, body, ct) = send_request("/api/v0/mesh/").await;
    assert_eq!(status, StatusCode::NOT_FOUND, "body: {body}");
    assert!(
        ct.contains("application/json"),
        "expected json content-type, got: {ct}"
    );
    let err: ErrorResponse = serde_json::from_str(&body).unwrap();
    assert!(!err.error.is_empty());
}
