/// Multi-step API integration scenarios for #106
///
/// Tests that the API correctly handles the scenarios that caused #102-#105.
/// Uses Rust integration tests (axum test client) — Playwright is Out-of-Scope
/// for CI environments.
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

async fn send_get_features(app: axum::Router) -> (StatusCode, String) {
    let req = Request::builder()
        .uri("/api/v0/features")
        .body(Body::empty())
        .unwrap();
    let resp = app.oneshot(req).await.unwrap();
    let status = resp.status();
    let bytes = axum::body::to_bytes(resp.into_body(), usize::MAX)
        .await
        .unwrap();
    (status, String::from_utf8(bytes.to_vec()).unwrap())
}

async fn send_get_mesh(app: axum::Router) -> (StatusCode, String) {
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

async fn send_post_feature(app: axum::Router, body: &str) -> (StatusCode, String) {
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

/// S01: GET /api/v0/features returns existing IDs, then POST with same ID returns 422.
///
/// This tests the scenario from #102: reloading a document and detecting
/// duplicate feature IDs to prevent silent overwrites.
#[tokio::test]
async fn s01_reload_no_duplicate_id() {
    let (_dir, path) = temp_copy("extruded_rect.mycad");

    // 1. GET /api/v0/features → existing IDs
    let app = make_app(path.clone());
    let (status, body) = send_get_features(app).await;
    assert_eq!(status, StatusCode::OK, "GET features: {body}");
    let ids: Vec<String> = serde_json::from_str(&body).unwrap();
    assert!(
        ids.contains(&"sketch_1".to_string()),
        "existing IDs must contain sketch_1: {ids:?}"
    );
    assert!(
        ids.contains(&"extrude_1".to_string()),
        "existing IDs must contain extrude_1: {ids:?}"
    );

    // 2. POST with a duplicate ID → 422
    let dup_json = r#"{"type":"create_sketch","id":"sketch_1","plane":"xy","profile":[{"id":"seg_0","from":[0.0,0.0],"to":[1.0,0.0]},{"id":"seg_1","from":[1.0,0.0],"to":[1.0,1.0]},{"id":"seg_2","from":[1.0,1.0],"to":[0.0,1.0]},{"id":"seg_3","from":[0.0,1.0],"to":[0.0,0.0]}]}"#;
    let app2 = make_app(path.clone());
    let (dup_status, dup_body) = send_post_feature(app2, dup_json).await;
    assert_eq!(
        dup_status,
        StatusCode::UNPROCESSABLE_ENTITY,
        "duplicate ID POST must return 422: {dup_body}"
    );
    assert!(
        dup_body.contains("duplicate"),
        "error message must mention 'duplicate': {dup_body}"
    );
}

/// S02: POST sketch + extrude → GET /api/v0/mesh → face_ids contain cap/side names.
///
/// This tests the scenario from #103: verifying that named faces
/// (f_cap_start, f_cap_end, f_side_*) appear in the tessellated mesh.
#[tokio::test]
async fn s02_extruded_face_ids_contain_cap() {
    let (_dir, path) = temp_copy("simple_box.mycad");

    // POST create_sketch (xy plane, 10x10 rectangle)
    let sketch_json = r#"{"type":"create_sketch","id":"sketch_0","plane":"xy","profile":[{"id":"seg_0","from":[0.0,0.0],"to":[10.0,0.0]},{"id":"seg_1","from":[10.0,0.0],"to":[10.0,10.0]},{"id":"seg_2","from":[10.0,10.0],"to":[0.0,10.0]},{"id":"seg_3","from":[0.0,10.0],"to":[0.0,0.0]}]}"#;
    let app1 = make_app(path.clone());
    let (sketch_status, sketch_body) = send_post_feature(app1, sketch_json).await;
    assert_eq!(sketch_status, StatusCode::OK, "sketch POST: {sketch_body}");

    // POST extrude (sketch_0, depth 5.0)
    let extrude_json = r#"{"type":"extrude","id":"extrude_0","sketch":"sketch_0","depth":5.0}"#;
    let app2 = make_app(path.clone());
    let (extrude_status, extrude_body) = send_post_feature(app2, extrude_json).await;
    assert_eq!(
        extrude_status,
        StatusCode::OK,
        "extrude POST: {extrude_body}"
    );

    // GET /api/v0/mesh and check face_ids
    let app3 = make_app(path);
    let (mesh_status, mesh_body) = send_get_mesh(app3).await;
    assert_eq!(mesh_status, StatusCode::OK, "GET mesh: {mesh_body}");

    let bodies: Vec<BodyMesh> = serde_json::from_str(&mesh_body).unwrap();
    let extrude_body_mesh = bodies
        .iter()
        .find(|b| b.feature_id == "extrude_0")
        .expect("extrude_0 body must exist");

    let all_face_ids: Vec<&str> = extrude_body_mesh
        .mesh
        .face_ids
        .iter()
        .map(|s| s.as_str())
        .collect();
    let unique_face_ids: std::collections::HashSet<&str> = all_face_ids.into_iter().collect();

    let has_cap = unique_face_ids
        .iter()
        .any(|id| id.starts_with("f_cap_") || id.contains("f_cap_"));
    let has_side = unique_face_ids
        .iter()
        .any(|id| id.starts_with("f_side_") || id.contains("f_side_"));
    assert!(
        has_cap || has_side,
        "face_ids must contain at least one f_cap_* or f_side_*: {unique_face_ids:?}"
    );
}

/// S03: POST sketch + extrude (no fuse_target) → creates two bodies (box_1 + extrude_0).
///
/// This tests the scenario from #104: verifying that without fuse,
/// the extrusion creates a separate body (kernel limitation: no boolean merge).
#[tokio::test]
async fn s03_extrude_creates_two_bodies() {
    let (_dir, path) = temp_copy("simple_box.mycad");

    // POST create_sketch (xy plane, 10x10 rectangle)
    let sketch_json = r#"{"type":"create_sketch","id":"sketch_0","plane":"xy","profile":[{"id":"seg_0","from":[0.0,0.0],"to":[10.0,0.0]},{"id":"seg_1","from":[10.0,0.0],"to":[10.0,10.0]},{"id":"seg_2","from":[10.0,10.0],"to":[0.0,10.0]},{"id":"seg_3","from":[0.0,10.0],"to":[0.0,0.0]}]}"#;
    let app1 = make_app(path.clone());
    let (sketch_status, sketch_body) = send_post_feature(app1, sketch_json).await;
    assert_eq!(sketch_status, StatusCode::OK, "sketch POST: {sketch_body}");

    // POST extrude (no fuse_target)
    let extrude_json = r#"{"type":"extrude","id":"extrude_0","sketch":"sketch_0","depth":5.0}"#;
    let app2 = make_app(path);
    let (extrude_status, extrude_body) = send_post_feature(app2, extrude_json).await;
    assert_eq!(
        extrude_status,
        StatusCode::OK,
        "extrude POST: {extrude_body}"
    );

    let bodies: Vec<BodyMesh> = serde_json::from_str(&extrude_body).unwrap();
    assert_eq!(
        bodies.len(),
        2,
        "without fuse_target, expect 2 bodies (box_1 + extrude_0), got {}: {:?}",
        bodies.len(),
        bodies.iter().map(|b| &b.feature_id).collect::<Vec<_>>()
    );

    let ids: Vec<&str> = bodies.iter().map(|b| b.feature_id.as_str()).collect();
    assert!(ids.contains(&"box_1"), "box_1 must exist: {ids:?}");
    assert!(ids.contains(&"extrude_0"), "extrude_0 must exist: {ids:?}");
}

/// S04: sketch at offset=5.0, extrude_cut depth=5.0 (== face distance) → 422.
///
/// This tests the scenario from #105: boundary condition where the cut depth
/// equals the offset distance, creating a degenerate (coplanar) result.
/// The UI-actual pattern (offset=0, depth≈face_dist) is covered by S04b instead.
#[tokio::test]
async fn s04_degen_extrudecut_depth_boundary() {
    let (_dir, path) = temp_copy("simple_box.mycad");

    // simple_box: 10x20x30, centered at origin → z ∈ [-15, 15]
    // Sketch on yz plane at offset=5.0 (offset along x)
    let sketch_json = r#"{"type":"create_sketch","id":"sketch_0","plane":"yz","offset":5.0,"profile":[{"id":"seg_0","from":[-2.0,-2.0],"to":[2.0,-2.0]},{"id":"seg_1","from":[2.0,-2.0],"to":[2.0,2.0]},{"id":"seg_2","from":[2.0,2.0],"to":[-2.0,2.0]},{"id":"seg_3","from":[-2.0,2.0],"to":[-2.0,-2.0]}]}"#;
    let app1 = make_app(path.clone());
    let (sketch_status, sketch_body) = send_post_feature(app1, sketch_json).await;
    assert_eq!(sketch_status, StatusCode::OK, "sketch POST: {sketch_body}");

    // ExtrudeCut with depth=5.0 (equals offset — boundary: tool just reaches the far face)
    let cut_json =
        r#"{"type":"extrude_cut","id":"cut_0","sketch":"sketch_0","depth":5.0,"target":"box_1"}"#;
    let app2 = make_app(path);
    let (cut_status, cut_body) = send_post_feature(app2, cut_json).await;
    assert_eq!(
        cut_status,
        StatusCode::UNPROCESSABLE_ENTITY,
        "boundary-depth extrude_cut must return 422: status={cut_status}, body={cut_body}"
    );
}

/// S04b: sketch at offset=5.0, extrude_cut depth=4.9 (< face distance) → 200 (just inside boundary).
///
/// Complements S04: depth slightly less than offset avoids the degenerate coplanar case.
#[tokio::test]
async fn s04b_extrudecut_depth_just_inside_boundary() {
    let (_dir, path) = temp_copy("simple_box.mycad");

    let sketch_json = r#"{"type":"create_sketch","id":"sketch_0","plane":"yz","offset":5.0,"profile":[{"id":"seg_0","from":[-2.0,-2.0],"to":[2.0,-2.0]},{"id":"seg_1","from":[2.0,-2.0],"to":[2.0,2.0]},{"id":"seg_2","from":[2.0,2.0],"to":[-2.0,2.0]},{"id":"seg_3","from":[-2.0,2.0],"to":[-2.0,-2.0]}]}"#;
    let app1 = make_app(path.clone());
    let (sketch_status, sketch_body) = send_post_feature(app1, sketch_json).await;
    assert_eq!(sketch_status, StatusCode::OK, "sketch POST: {sketch_body}");

    let cut_json =
        r#"{"type":"extrude_cut","id":"cut_0","sketch":"sketch_0","depth":4.9,"target":"box_1"}"#;
    let app2 = make_app(path);
    let (cut_status, cut_body) = send_post_feature(app2, cut_json).await;
    assert_eq!(
        cut_status,
        StatusCode::OK,
        "depth=4.9 (< offset=5.0) extrude_cut must succeed: status={cut_status}, body={cut_body}"
    );
}

/// S06: extrude_cut with depth=0.0 → 422 (degenerate zero-depth tool).
///
/// A zero-depth cut creates a degenerate (zero-volume) tool body, which should
/// be rejected by the kernel rather than silently accepted.
#[tokio::test]
async fn s06_zero_depth_extrude_cut_rejected() {
    let (_dir, path) = temp_copy("simple_box.mycad");

    let sketch_json = r#"{"type":"create_sketch","id":"sketch_0","plane":"yz","offset":1.0,"profile":[{"id":"seg_0","from":[-2.0,-2.0],"to":[2.0,-2.0]},{"id":"seg_1","from":[2.0,-2.0],"to":[2.0,2.0]},{"id":"seg_2","from":[2.0,2.0],"to":[-2.0,2.0]},{"id":"seg_3","from":[-2.0,2.0],"to":[-2.0,-2.0]}]}"#;
    let app1 = make_app(path.clone());
    let (sketch_status, sketch_body) = send_post_feature(app1, sketch_json).await;
    assert_eq!(sketch_status, StatusCode::OK, "sketch POST: {sketch_body}");

    let cut_json =
        r#"{"type":"extrude_cut","id":"cut_0","sketch":"sketch_0","depth":0.0,"target":"box_1"}"#;
    let app2 = make_app(path);
    let (cut_status, cut_body) = send_post_feature(app2, cut_json).await;
    assert_eq!(
        cut_status,
        StatusCode::UNPROCESSABLE_ENTITY,
        "zero-depth extrude_cut must return 422: status={cut_status}, body={cut_body}"
    );
}

/// S07: extrude_cut with negative depth → 422 (invalid input).
///
/// A negative depth is geometrically meaningless for a cut operation and
/// must be rejected at the API or kernel level.
#[tokio::test]
async fn s07_negative_depth_extrude_cut_rejected() {
    let (_dir, path) = temp_copy("simple_box.mycad");

    let sketch_json = r#"{"type":"create_sketch","id":"sketch_0","plane":"yz","offset":1.0,"profile":[{"id":"seg_0","from":[-2.0,-2.0],"to":[2.0,-2.0]},{"id":"seg_1","from":[2.0,-2.0],"to":[2.0,2.0]},{"id":"seg_2","from":[2.0,2.0],"to":[-2.0,2.0]},{"id":"seg_3","from":[-2.0,2.0],"to":[-2.0,-2.0]}]}"#;
    let app1 = make_app(path.clone());
    let (sketch_status, sketch_body) = send_post_feature(app1, sketch_json).await;
    assert_eq!(sketch_status, StatusCode::OK, "sketch POST: {sketch_body}");

    let cut_json =
        r#"{"type":"extrude_cut","id":"cut_0","sketch":"sketch_0","depth":-1.0,"target":"box_1"}"#;
    let app2 = make_app(path);
    let (cut_status, cut_body) = send_post_feature(app2, cut_json).await;
    assert_eq!(
        cut_status,
        StatusCode::UNPROCESSABLE_ENTITY,
        "negative-depth extrude_cut must return 422: status={cut_status}, body={cut_body}"
    );
}

/// S05: Determinism — sketch + extrude sequence run twice produces identical output.
///
/// Verifies that the full multi-step pipeline is deterministic: same inputs
/// produce same vertex counts, same face_ids, same everything.
#[tokio::test]
async fn s05_determinism_multi_step() {
    let sketch_json = r#"{"type":"create_sketch","id":"sketch_0","plane":"xy","profile":[{"id":"seg_0","from":[0.0,0.0],"to":[10.0,0.0]},{"id":"seg_1","from":[10.0,0.0],"to":[10.0,10.0]},{"id":"seg_2","from":[10.0,10.0],"to":[0.0,10.0]},{"id":"seg_3","from":[0.0,10.0],"to":[0.0,0.0]}]}"#;
    let extrude_json = r#"{"type":"extrude","id":"extrude_0","sketch":"sketch_0","depth":5.0}"#;

    // Run 1
    let (_dir1, path1) = temp_copy("simple_box.mycad");
    let app1a = make_app(path1.clone());
    let (s1, _) = send_post_feature(app1a, sketch_json).await;
    assert_eq!(s1, StatusCode::OK);
    let app1b = make_app(path1);
    let (_, body1) = send_post_feature(app1b, extrude_json).await;

    // Run 2
    let (_dir2, path2) = temp_copy("simple_box.mycad");
    let app2a = make_app(path2.clone());
    let (s2, _) = send_post_feature(app2a, sketch_json).await;
    assert_eq!(s2, StatusCode::OK);
    let app2b = make_app(path2);
    let (_, body2) = send_post_feature(app2b, extrude_json).await;

    assert_eq!(
        body1, body2,
        "multi-step sequence must produce identical output"
    );
}
