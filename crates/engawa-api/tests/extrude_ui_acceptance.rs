/// Acceptance tests for #95: 選択面からの押出(Extrude) UI — A01
///
/// Tests:
///   A01: create_sketch POST → extrude POST → 頂点数増加 + .engawa に type:extrude 1件
use axum::body::Body;
use axum::http::{Request, StatusCode};
use engawa_api::router::app;
use engawa_kernel::tessellation::TriangleMesh;
use serde::Deserialize;
use std::path::PathBuf;
use std::sync::Arc;
use tower::ServiceExt;

#[derive(Deserialize)]
struct BodyMesh {
    #[allow(dead_code)]
    feature_id: String,
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

fn temp_copy(example_name: &str) -> (tempfile::TempDir, PathBuf) {
    let src = example_path(example_name);
    let dir = tempfile::tempdir().unwrap();
    let dst = dir.path().join(example_name);
    std::fs::copy(&src, &dst).unwrap();
    let canonical = std::fs::canonicalize(&dst).unwrap();
    (dir, canonical)
}

fn make_app(file: PathBuf) -> axum::Router {
    app(Arc::new(file))
}

async fn send_post(app: axum::Router, body: &str) -> (StatusCode, String) {
    let req = Request::builder()
        .method("POST")
        .uri("/api/v0/features")
        .header("content-type", "application/json")
        .body(Body::from(body.to_string()))
        .unwrap();
    let resp = app.oneshot(req).await.unwrap();
    let status = resp.status();
    let bytes = axum::body::to_bytes(resp.into_body(), usize::MAX)
        .await
        .unwrap();
    (status, String::from_utf8(bytes.to_vec()).unwrap())
}

async fn send_get(app: axum::Router) -> (StatusCode, String) {
    let req = Request::builder()
        .uri("/api/v0/mesh")
        .body(Body::empty())
        .unwrap();
    let resp = app.oneshot(req).await.unwrap();
    let status = resp.status();
    let bytes = axum::body::to_bytes(resp.into_body(), usize::MAX)
        .await
        .unwrap();
    (status, String::from_utf8(bytes.to_vec()).unwrap())
}

/// Count total vertices across all bodies in the mesh response
fn total_vertex_count(body: &str) -> usize {
    let bodies: Vec<BodyMesh> = serde_json::from_str(body).unwrap();
    bodies.iter().map(|b| b.mesh.positions.len()).sum()
}

/// A01: create_sketch POST → extrude POST で頂点数が増加し、
///      書き戻された .engawa に type:extrude が1件存在することを検証する。
///
/// 検証内容:
///   1. GET /api/v0/mesh で初期頂点総数を記録する
///   2. POST create_sketch (xy 平面, 10×10 の矩形プロファイル) → 200
///   3. POST extrude (sketch_0 を depth:5.0 で押出) → 200, 頂点数 > 初期
///   4. .engawa の内容を読み、features に type: extrude が1件存在する
#[tokio::test]
async fn a01_extrude_ui_increases_vertices_and_writes_feature() {
    let (_dir, path) = temp_copy("simple_box.engawa");

    // 1. Get initial vertex count
    let app_get = make_app(path.clone());
    let (get_status, get_body) = send_get(app_get).await;
    assert_eq!(get_status, StatusCode::OK, "initial GET: {get_body}");
    let initial_vertices = total_vertex_count(&get_body);

    // 2. POST create_sketch (xy plane, 10×10 rectangle)
    let sketch_json = r#"{"type":"create_sketch","id":"sketch_0","plane":"xy","profile":[{"id":"seg_0","from":[0.0,0.0],"to":[10.0,0.0]},{"id":"seg_1","from":[10.0,0.0],"to":[10.0,10.0]},{"id":"seg_2","from":[10.0,10.0],"to":[0.0,10.0]},{"id":"seg_3","from":[0.0,10.0],"to":[0.0,0.0]}]}"#;
    let app1 = make_app(path.clone());
    let (sketch_status, sketch_body) = send_post(app1, sketch_json).await;
    assert_eq!(sketch_status, StatusCode::OK, "sketch POST: {sketch_body}");

    // 3. POST extrude (sketch_0, depth 5.0)
    let extrude_json = r#"{"type":"extrude","id":"extrude_0","sketch":"sketch_0","depth":5.0}"#;
    let app2 = make_app(path.clone());
    let (extrude_status, extrude_body) = send_post(app2, extrude_json).await;
    assert_eq!(
        extrude_status,
        StatusCode::OK,
        "extrude POST: {extrude_body}"
    );

    // 4. Verify vertex count increased
    let after_vertices = total_vertex_count(&extrude_body);
    assert!(
        after_vertices > initial_vertices,
        "vertices must increase after extrude: before={initial_vertices}, after={after_vertices}"
    );

    // 5. Verify .engawa file contains type: extrude
    let engawa_content = std::fs::read_to_string(&path).unwrap();
    let extrude_count = engawa_content
        .lines()
        .filter(|line| line.trim().ends_with("type: extrude"))
        .count();
    assert_eq!(
        extrude_count, 1,
        ".engawa must contain exactly 1 'type: extrude' feature, found {extrude_count}"
    );
}
