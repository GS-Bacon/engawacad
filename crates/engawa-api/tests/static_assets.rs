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
async fn t03_root_returns_html() {
    let app = test_app();
    let req = Request::builder()
        .uri("/")
        .header("host", "127.0.0.1:7878")
        .body(Body::empty())
        .unwrap();
    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);

    let ct = resp
        .headers()
        .get("content-type")
        .expect("missing content-type")
        .to_str()
        .unwrap();
    assert!(ct.contains("text/html"), "expected text/html, got {ct}");

    let body = axum::body::to_bytes(resp.into_body(), usize::MAX)
        .await
        .unwrap();
    let body_str = String::from_utf8_lossy(&body);

    if body_str.contains("MYCAD frontend not built") {
        // stub path — acceptable
    } else {
        assert!(
            body_str.contains("<script"),
            "real viewer should contain <script> tag, got: {body_str}"
        );
    }
}

#[tokio::test]
async fn t04_missing_asset_falls_back() {
    let app = test_app();
    let req = Request::builder()
        .uri("/assets/nonexistent-file-xyz.js")
        .header("host", "127.0.0.1:7878")
        .body(Body::empty())
        .unwrap();
    let resp = app.oneshot(req).await.unwrap();
    // File-like paths with extension must 404, not SPA fallback
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn t05_mesh_api_still_works() {
    let fixture = std::path::PathBuf::from("../../examples/simple_box.mycad");
    let canonical = std::fs::canonicalize(&fixture).unwrap();

    let app = app(Arc::new(canonical));
    let req = Request::builder()
        .uri("/api/v0/mesh")
        .header("host", "127.0.0.1:7878")
        .body(Body::empty())
        .unwrap();
    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
}

#[tokio::test]
async fn t08_static_assets_host_guard_root() {
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
async fn t08_static_assets_host_guard_assets() {
    let app = test_app();
    let req = Request::builder()
        .uri("/assets/test.js")
        .header("host", "evil.com")
        .body(Body::empty())
        .unwrap();
    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn t09_real_viewer_asset_served() {
    let app1 = test_app();

    let req = Request::builder()
        .uri("/")
        .header("host", "127.0.0.1:7878")
        .body(Body::empty())
        .unwrap();
    let resp = app1.oneshot(req).await.unwrap();
    let body = axum::body::to_bytes(resp.into_body(), usize::MAX)
        .await
        .unwrap();
    let index_html = String::from_utf8_lossy(&body);

    if index_html.contains("MYCAD frontend not built") {
        return;
    }

    let script_src = extract_script_src(&index_html);
    assert!(!script_src.is_empty(), "no script src found in index.html");

    let asset_path = script_src.trim_start_matches("./");
    let app2 = test_app();
    let req = Request::builder()
        .uri(format!("/{asset_path}"))
        .header("host", "127.0.0.1:7878")
        .body(Body::empty())
        .unwrap();
    let resp = app2.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);

    let ct = resp
        .headers()
        .get("content-type")
        .expect("missing content-type")
        .to_str()
        .unwrap();
    assert!(
        ct.contains("javascript"),
        "expected javascript content-type, got {ct}"
    );
}

fn extract_script_src(html: &str) -> String {
    for line in html.lines() {
        if line.contains("script") && line.contains("src=") {
            if let Some(start) = line.find("src=\"") {
                let rest = &line[start + 5..];
                if let Some(end) = rest.find('"') {
                    return rest[..end].to_string();
                }
            }
        }
    }
    String::new()
}
