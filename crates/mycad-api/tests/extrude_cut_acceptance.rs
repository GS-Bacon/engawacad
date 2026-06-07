/// Acceptance tests for #96: ExtrudeCut Feature 追加と UI
///
/// Tests:
///   A01: create_sketch POST → extrude_cut POST → 頂点数変化 + .mycad に type:extrude_cut 1件
use axum::body::Body;
use axum::http::{Request, StatusCode};
use mycad_api::router::app;
use mycad_kernel::tessellation::TriangleMesh;
use serde::Deserialize;
use std::path::PathBuf;
use std::sync::Arc;
use tower::ServiceExt;

#[derive(Debug, Deserialize)]
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

async fn send_post(app: axum::Router, body: &str) -> (StatusCode, String) {
    let req = Request::builder()
        .method("POST")
        .uri("/api/v0/features")
        .header("content-type", "application/json")
        .body(Body::from(body.to_owned()))
        .unwrap();
    let resp = app.oneshot(req).await.unwrap();
    let status = resp.status();
    let bytes = axum::body::to_bytes(resp.into_body(), usize::MAX)
        .await
        .unwrap();
    (status, String::from_utf8(bytes.to_vec()).unwrap())
}

fn total_vertex_count(body: &str) -> usize {
    let bodies: Vec<BodyMesh> = serde_json::from_str(body).unwrap();
    bodies.iter().map(|b| b.mesh.positions.len()).sum()
}

/// A01: create_sketch → extrude_cut (internal void) の疎通 acceptance test
///
/// simple_box (10×20×30, 原点中心: z∈[-15,15]) に対し、
/// XY 平面上のインセット角柱 [-2,2]×[-4,4], depth=3 でカット。
/// tool z∈[0,3] は box z_top=15 より十分小さい → 完全埋没 void (2 シェル)。
/// coplanar 面なし(U02 と同一幾何)。
/// 期待: 頂点数変化 + .mycad に type: extrude_cut が 1 件。
#[tokio::test]
async fn a01_extrude_cut_increases_vertices_and_writes_feature() {
    let (_dir, path) = temp_copy("simple_box.mycad");

    // 1. GET initial vertex count
    let app_get = make_app(path.clone());
    let (get_status, get_body) = send_get(app_get).await;
    assert_eq!(get_status, StatusCode::OK, "initial GET: {get_body}");
    let initial_vertices = total_vertex_count(&get_body);

    // 2. POST create_sketch (xy plane, inset [-2,2]×[-4,4])
    let sketch_json = r#"{"type":"create_sketch","id":"sketch_0","plane":"xy","profile":[{"id":"seg_0","from":[-2.0,-4.0],"to":[2.0,-4.0]},{"id":"seg_1","from":[2.0,-4.0],"to":[2.0,4.0]},{"id":"seg_2","from":[2.0,4.0],"to":[-2.0,4.0]},{"id":"seg_3","from":[-2.0,4.0],"to":[-2.0,-4.0]}]}"#;
    let app1 = make_app(path.clone());
    let (sketch_status, sketch_body) = send_post(app1, sketch_json).await;
    assert_eq!(sketch_status, StatusCode::OK, "sketch POST: {sketch_body}");

    // 3. POST extrude_cut (sketch_0, depth=20, target=box_1)
    let extrude_cut_json = r#"{"type":"extrude_cut","id":"extrude_cut_0","sketch":"sketch_0","depth":3.0,"target":"box_1"}"#;
    let app2 = make_app(path.clone());
    let (cut_status, cut_body) = send_post(app2, extrude_cut_json).await;
    assert_eq!(cut_status, StatusCode::OK, "extrude_cut POST: {cut_body}");

    // 4. Verify vertex count changed
    let after_vertices = total_vertex_count(&cut_body);
    assert!(
        after_vertices != initial_vertices,
        "vertices must change after extrude_cut: before={initial_vertices}, after={after_vertices}"
    );

    // 5. Verify .mycad contains type: extrude_cut (1 件)
    let mycad_content = std::fs::read_to_string(&path).unwrap();
    let count = mycad_content
        .lines()
        .filter(|l| l.trim().ends_with("type: extrude_cut"))
        .count();
    assert_eq!(
        count, 1,
        ".mycad must have exactly 1 extrude_cut feature, found {count}"
    );
}
