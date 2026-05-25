use axum::body::Body;
use axum::http::{Request, StatusCode};
use mycad_api::router::app;
use tower::ServiceExt;

#[tokio::test]
async fn edge_root_no_host_header() {
    let app = app();
    let req = Request::builder().uri("/").body(Body::empty()).unwrap();
    let resp = app.oneshot(req).await.unwrap();
    // empty host is allowed by host_guard
    assert_eq!(resp.status(), StatusCode::OK);
}

#[tokio::test]
async fn edge_localhost_host_allowed() {
    let app = app();
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
    let app = app();
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
    let app = app();
    let file_param = urlencoding::encode("/nonexistent/path.mycad");
    let req = Request::builder()
        .uri(format!("/api/v0/mesh?file={file_param}"))
        .header("host", "127.0.0.1:7878")
        .body(Body::empty())
        .unwrap();
    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn edge_api_mesh_relative_path_rejected() {
    let app = app();
    let file_param = urlencoding::encode("relative/path.mycad");
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

    let app1 = app();
    let resp1 = app1.oneshot(make_req()).await.unwrap();
    let body1 = axum::body::to_bytes(resp1.into_body(), usize::MAX)
        .await
        .unwrap();

    let app2 = app();
    let resp2 = app2.oneshot(make_req()).await.unwrap();
    let body2 = axum::body::to_bytes(resp2.into_body(), usize::MAX)
        .await
        .unwrap();

    assert_eq!(body1, body2, "index.html should be deterministic");
}

#[tokio::test]
async fn edge_empty_query_file() {
    let app = app();
    let req = Request::builder()
        .uri("/api/v0/mesh?file=")
        .header("host", "127.0.0.1:7878")
        .body(Body::empty())
        .unwrap();
    let resp = app.oneshot(req).await.unwrap();
    // Empty string is a relative path → 400
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
}
