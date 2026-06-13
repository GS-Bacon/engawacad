use axum::body::Body;
use axum::http::{Request, StatusCode};
use engawa_api::router::app;
use std::path::PathBuf;
use std::sync::Arc;
use tower::ServiceExt;

fn test_app() -> axum::Router {
    app(Arc::new(PathBuf::from("/dev/null")))
}

#[tokio::test]
async fn edge_root_no_host_header() {
    let app = test_app();
    let req = Request::builder().uri("/").body(Body::empty()).unwrap();
    let resp = app.oneshot(req).await.unwrap();
    // empty host is allowed by host_guard
    assert_eq!(resp.status(), StatusCode::OK);
}

#[tokio::test]
async fn edge_localhost_host_allowed() {
    let app = test_app();
    let req = Request::builder()
        .uri("/")
        .header("host", "localhost:7878")
        .body(Body::empty())
        .unwrap();
    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
}

#[tokio::test]
async fn edge_traversal_path_serves_index() {
    let app = test_app();
    let req = Request::builder()
        .uri("/../../../etc/passwd")
        .header("host", "127.0.0.1:7878")
        .body(Body::empty())
        .unwrap();
    let resp = app.oneshot(req).await.unwrap();
    // rust-embed strips path traversal; falls back to index.html
    assert_eq!(resp.status(), StatusCode::OK);
}

#[tokio::test]
async fn edge_api_mesh_missing_file() {
    let app = app(Arc::new(PathBuf::from("/nonexistent/path.engawa")));
    let req = Request::builder()
        .uri("/api/v0/mesh")
        .header("host", "127.0.0.1:7878")
        .body(Body::empty())
        .unwrap();
    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn edge_api_mesh_relative_path_rejected() {
    let app = test_app();
    let file_param = urlencoding::encode("relative/path.engawa");
    let req = Request::builder()
        .uri(format!("/api/v0/mesh?file={file_param}"))
        .header("host", "127.0.0.1:7878")
        .body(Body::empty())
        .unwrap();
    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn edge_deterministic_index_html() {
    let make_req = || {
        Request::builder()
            .uri("/")
            .header("host", "127.0.0.1:7878")
            .body(Body::empty())
            .unwrap()
    };

    let app1 = test_app();
    let resp1 = app1.oneshot(make_req()).await.unwrap();
    let body1 = axum::body::to_bytes(resp1.into_body(), usize::MAX)
        .await
        .unwrap();

    let app2 = test_app();
    let resp2 = app2.oneshot(make_req()).await.unwrap();
    let body2 = axum::body::to_bytes(resp2.into_body(), usize::MAX)
        .await
        .unwrap();

    assert_eq!(body1, body2, "index.html should be deterministic");
}

#[tokio::test]
async fn edge_empty_query_file() {
    let app = test_app();
    let req = Request::builder()
        .uri("/api/v0/mesh?file=")
        .header("host", "127.0.0.1:7878")
        .body(Body::empty())
        .unwrap();
    let resp = app.oneshot(req).await.unwrap();
    // Empty string is a relative path → 400
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn edge_host_guard_tailscale_ip_allowed() {
    let app = test_app();
    let req = Request::builder()
        .uri("/")
        .header("host", "10.13.1.1:7878")
        .body(Body::empty())
        .unwrap();
    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
}

#[tokio::test]
async fn edge_host_guard_ipv4_no_port_allowed() {
    let app = test_app();
    let req = Request::builder()
        .uri("/")
        .header("host", "192.168.1.100")
        .body(Body::empty())
        .unwrap();
    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
}

#[tokio::test]
async fn edge_host_guard_ipv6_bracket_allowed() {
    let app = test_app();
    let req = Request::builder()
        .uri("/")
        .header("host", "[::1]:7878")
        .body(Body::empty())
        .unwrap();
    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
}

#[tokio::test]
async fn edge_host_guard_loopback_ip_allowed() {
    let app = test_app();
    let req = Request::builder()
        .uri("/")
        .header("host", "127.0.0.1:7878")
        .body(Body::empty())
        .unwrap();
    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
}

#[tokio::test]
async fn edge_host_guard_domain_still_blocked() {
    let app = test_app();
    let req = Request::builder()
        .uri("/")
        .header("host", "evil.com")
        .body(Body::empty())
        .unwrap();
    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn edge_host_guard_ip_with_subdomain_blocked() {
    // "10.13.evil.com" — contains letters, should be blocked
    let app = test_app();
    let req = Request::builder()
        .uri("/")
        .header("host", "10.13.evil.com")
        .body(Body::empty())
        .unwrap();
    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn edge_host_guard_0_0_0_0_allowed() {
    let app = test_app();
    let req = Request::builder()
        .uri("/")
        .header("host", "0.0.0.0:7878")
        .body(Body::empty())
        .unwrap();
    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
}

#[tokio::test]
async fn edge_host_guard_bare_ip_allowed() {
    // Port-less numeric IP (split gives the IP itself)
    let app = test_app();
    let req = Request::builder()
        .uri("/")
        .header("host", "10.0.0.1")
        .body(Body::empty())
        .unwrap();
    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
}
