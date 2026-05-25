use axum::body::Body;
use axum::http::{Request, StatusCode};
use mycad_api::router::app;
use serde::Deserialize;
use tower::ServiceExt;

#[derive(Deserialize)]
struct ErrorResponse {
    #[allow(dead_code)]
    error: String,
}

async fn send_get(uri: &str) -> (StatusCode, String, String) {
    let req = Request::builder().uri(uri).body(Body::empty()).unwrap();
    let app = app();
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

async fn send_method(method: &str, uri: &str) -> (StatusCode, String, String) {
    let req = Request::builder()
        .method(method)
        .uri(uri)
        .body(Body::empty())
        .unwrap();
    let app = app();
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

fn assert_json_404(status: StatusCode, body: &str, ct: &str, label: &str) {
    assert_eq!(
        status,
        StatusCode::NOT_FOUND,
        "{label}: expected 404, got {status}, body: {body}"
    );
    assert!(
        ct.contains("application/json"),
        "{label}: expected json content-type, got: {ct}"
    );
    let _: ErrorResponse = serde_json::from_str(body).unwrap_or_else(|e| {
        panic!("{label}: body is not valid JSON: {e}\nbody: {body}");
    });
}

// E01: deeply nested unknown path
#[tokio::test]
async fn e01_deeply_nested_unknown() {
    let (status, body, ct) = send_get("/api/v0/a/b/c/d/e").await;
    assert_json_404(status, &body, &ct, "deeply_nested");
}

// E02: unknown path with query string
#[tokio::test]
async fn e02_unknown_with_query() {
    let (status, body, ct) = send_get("/api/v0/unknown?foo=bar&baz=1").await;
    assert_json_404(status, &body, &ct, "with_query");
}

// E03: POST to unknown api path → still JSON 404
#[tokio::test]
async fn e03_post_unknown_api_path() {
    let (status, body, ct) = send_method("POST", "/api/v0/unknown").await;
    assert_json_404(status, &body, &ct, "post_unknown");
}

// E04: PUT to unknown api path
#[tokio::test]
async fn e04_put_unknown_api_path() {
    let (status, body, ct) = send_method("PUT", "/api/v0/unknown").await;
    assert_json_404(status, &body, &ct, "put_unknown");
}

// E05: DELETE to unknown api path
#[tokio::test]
async fn e05_delete_unknown_api_path() {
    let (status, body, ct) = send_method("DELETE", "/api/v0/unknown").await;
    assert_json_404(status, &body, &ct, "delete_unknown");
}

// E06: /api/v0 root → JSON 404 (no index route)
#[tokio::test]
async fn e06_api_v0_root() {
    let (status, body, ct) = send_get("/api/v0").await;
    assert_json_404(status, &body, &ct, "api_v0_root");
}

// E07: /api/v0/ with trailing slash — Axum nest("/api/v0") does NOT match "/api/v0/"
// so it falls through to outer static_handler. This is an Axum routing quirk, not our bug.
// Verify it does NOT return JSON 404 (it returns static HTML instead).
#[tokio::test]
async fn e07_api_v0_root_trailing_slash_is_static() {
    let (status, _body, ct) = send_get("/api/v0/").await;
    assert_ne!(
        status,
        StatusCode::NOT_FOUND,
        "trailing slash should use static fallback, not API 404"
    );
    assert!(
        !ct.contains("application/json"),
        "trailing slash should use static fallback, not JSON. ct={ct}"
    );
}

// E08: /api/v0/mesh with no query → handler error (not static fallback HTML)
#[tokio::test]
async fn e08_mesh_no_query_not_static() {
    let (status, _body, ct) = send_get("/api/v0/mesh").await;
    // The mesh handler returns its own error (text/plain or JSON), never static HTML
    assert!(
        !ct.contains("text/html"),
        "mesh without query should not serve HTML static fallback. ct={ct}"
    );
    assert_ne!(status, StatusCode::OK, "should not return 200");
}

// E09: special characters in unknown path
#[tokio::test]
async fn e09_special_chars_unknown() {
    let (status, body, ct) = send_get("/api/v0/%00null").await;
    assert_json_404(status, &body, &ct, "special_chars");
}

// E10: static assets still work for non-api paths
#[tokio::test]
async fn e10_static_assets_still_serve() {
    let (status, _body, _ct) = send_get("/").await;
    // Root should still serve static content (200 or whatever static_handler returns)
    // It should NOT be a JSON 404
    assert_ne!(status, StatusCode::NOT_FOUND, "root should not be API 404");
}

// E11: /api (without v0) should use static fallback, not JSON 404
#[tokio::test]
async fn e11_api_without_v0_not_json_404() {
    let (status, body, ct) = send_get("/api").await;
    // This is outside /api/v0 so static_handler handles it
    assert!(
        !ct.contains("application/json") || status != StatusCode::NOT_FOUND,
        "/api should use static fallback, not JSON 404. status={status}, ct={ct}, body={body}"
    );
}
