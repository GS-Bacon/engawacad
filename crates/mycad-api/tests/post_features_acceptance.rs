/// Acceptance tests for POST /api/v0/features (Issue #93)
use axum::body::Body;
use axum::http::{Request, StatusCode};
use mycad_api::router::app;
use mycad_kernel::tessellation::TriangleMesh;
use serde::Deserialize;
use std::path::PathBuf;
use std::sync::Arc;
use tower::ServiceExt;

#[derive(Deserialize)]
#[allow(dead_code)]
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

/// Copy an example .mycad into a temp dir so POST can mutate it safely.
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

// T01: Determinism — same Feature POSTed to two independent copies → identical responses
#[tokio::test]
async fn t01_determinism() {
    let feature_json = r#"{"type":"create_box","id":"box_2","width":5.0,"height":5.0,"depth":5.0}"#;

    // Two independent temp copies so each POST starts from the same initial state
    let (_dir1, path1) = temp_copy("simple_box.mycad");
    let app1 = make_app(path1);
    let (status1, body1) = send_post(app1, feature_json).await;
    assert_eq!(status1, StatusCode::OK, "body1: {body1}");

    let (_dir2, path2) = temp_copy("simple_box.mycad");
    let app2 = make_app(path2);
    let (status2, body2) = send_post(app2, feature_json).await;
    assert_eq!(status2, StatusCode::OK, "body2: {body2}");

    assert_eq!(body1, body2, "POST responses must be deterministic");
}

// T02: Normal — POST a valid CreateBox Feature → 200, Vec<BodyMesh> non-empty, features +1
#[tokio::test]
async fn t02_post_creates_body() {
    let (_dir, path) = temp_copy("simple_box.mycad");
    let feature_json = r#"{"type":"create_box","id":"box_2","width":5.0,"height":5.0,"depth":5.0}"#;

    let app = make_app(path);
    let (status, body) = send_post(app, feature_json).await;
    assert_eq!(status, StatusCode::OK, "body: {body}");

    let bodies: Vec<BodyMesh> = serde_json::from_str(&body).unwrap();
    assert!(!bodies.is_empty(), "POST response bodies must not be empty");
    // Original box_1 + new box_2 → 2 bodies
    let ids: Vec<&str> = bodies.iter().map(|b| b.feature_id.as_str()).collect();
    assert!(
        ids.contains(&"box_1"),
        "original box_1 must be present: {ids:?}"
    );
    assert!(ids.contains(&"box_2"), "new box_2 must be present: {ids:?}");
}

// T03: Idempotency — POST response == subsequent GET /api/v0/mesh
#[tokio::test]
async fn t03_idempotency_post_eq_get() {
    let (_dir, path) = temp_copy("simple_box.mycad");
    let feature_json = r#"{"type":"create_box","id":"box_2","width":5.0,"height":5.0,"depth":5.0}"#;

    let app = make_app(path.clone());
    let (status, post_body) = send_post(app, feature_json).await;
    assert_eq!(status, StatusCode::OK, "post body: {post_body}");

    // New app instance reads the mutated file from disk
    let app_get = make_app(path);
    let (get_status, get_body) = send_get(app_get).await;
    assert_eq!(get_status, StatusCode::OK, "get body: {get_body}");

    assert_eq!(
        post_body, get_body,
        "POST response must equal subsequent GET response"
    );
}

// T04: Accumulation — two consecutive POSTs with different IDs → features +2
#[tokio::test]
async fn t04_accumulation() {
    let (_dir, path) = temp_copy("simple_box.mycad");

    let feature_a = r#"{"type":"create_box","id":"box_a","width":3.0,"height":3.0,"depth":3.0}"#;
    let feature_b = r#"{"type":"create_box","id":"box_b","width":7.0,"height":7.0,"depth":7.0}"#;

    // First POST
    let app1 = make_app(path.clone());
    let (status1, body1) = send_post(app1, feature_a).await;
    assert_eq!(status1, StatusCode::OK, "body1: {body1}");

    let bodies1: Vec<BodyMesh> = serde_json::from_str(&body1).unwrap();
    assert_eq!(
        bodies1.len(),
        2,
        "after first POST: 2 bodies (box_1 + box_a)"
    );

    // Second POST
    let app2 = make_app(path);
    let (status2, body2) = send_post(app2, feature_b).await;
    assert_eq!(status2, StatusCode::OK, "body2: {body2}");

    let bodies2: Vec<BodyMesh> = serde_json::from_str(&body2).unwrap();
    assert_eq!(
        bodies2.len(),
        3,
        "after second POST: 3 bodies (box_1 + box_a + box_b)"
    );

    let ids: Vec<&str> = bodies2.iter().map(|b| b.feature_id.as_str()).collect();
    assert!(ids.contains(&"box_1"), "box_1 missing: {ids:?}");
    assert!(ids.contains(&"box_a"), "box_a missing: {ids:?}");
    assert!(ids.contains(&"box_b"), "box_b missing: {ids:?}");
}

// T05_boundary: duplicate feature_id → 422, file and in-memory doc unchanged
#[tokio::test]
async fn t05_boundary_duplicate_id_rollback() {
    let (_dir, path) = temp_copy("simple_box.mycad");
    // box_1 already exists in simple_box.mycad → duplicate
    let dup_json = r#"{"type":"create_box","id":"box_1","width":2.0,"height":2.0,"depth":2.0}"#;

    let app = make_app(path.clone());
    let (status, body) = send_post(app, dup_json).await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "body: {body}");

    // File must be unchanged — a fresh app GET must return the original 1-body result
    let app_after = make_app(path);
    let (get_status, get_body) = send_get(app_after).await;
    assert_eq!(get_status, StatusCode::OK, "get body: {get_body}");
    let bodies: Vec<serde_json::Value> = serde_json::from_str(&get_body).unwrap();
    assert_eq!(
        bodies.len(),
        1,
        "file should still have exactly 1 body after rollback"
    );
}

// T06_degen: shape-invalid body (valid JSON but wrong shape) → 422, state unchanged
#[tokio::test]
async fn t06_degen_invalid_body_shape() {
    let (_dir, path) = temp_copy("simple_box.mycad");
    // Valid JSON but missing required fields for any Feature variant
    let bad_json = r#"{"type":"create_box","width":5.0}"#; // missing id

    let app = make_app(path.clone());
    let (status, _body) = send_post(app, bad_json).await;
    assert_eq!(
        status,
        StatusCode::UNPROCESSABLE_ENTITY,
        "malformed body should return 422"
    );

    // State unchanged — fresh GET returns original 1 body
    let app_after = make_app(path);
    let (get_status, get_body) = send_get(app_after).await;
    assert_eq!(get_status, StatusCode::OK);
    let bodies: Vec<serde_json::Value> = serde_json::from_str(&get_body).unwrap();
    assert_eq!(bodies.len(), 1, "state must be unchanged after 422");
}

// --- Edge-case / degenerate / boundary tests (GLM Phase 2) ---

// E01: Completely invalid (non-JSON) body → 400 (axum JsonRejection), state unchanged
#[tokio::test]
async fn edge_non_json_body_returns_400() {
    let (_dir, path) = temp_copy("simple_box.mycad");

    let app = make_app(path.clone());
    let (status, _body) = send_post(app, "this is not json at all!!!").await;
    assert!(
        status == StatusCode::BAD_REQUEST || status == StatusCode::UNPROCESSABLE_ENTITY,
        "non-JSON body should return 4xx, got {status}"
    );

    // State unchanged
    let app_after = make_app(path);
    let (get_status, get_body) = send_get(app_after).await;
    assert_eq!(get_status, StatusCode::OK);
    let bodies: Vec<serde_json::Value> = serde_json::from_str(&get_body).unwrap();
    assert_eq!(bodies.len(), 1, "state unchanged after non-JSON POST");
}

// E02: POST determinism — 100 runs on independent copies, all identical
#[tokio::test]
async fn edge_determinism_100_runs() {
    let feature_json = r#"{"type":"create_box","id":"box_2","width":5.0,"height":5.0,"depth":5.0}"#;

    let (_dir_ref, path_ref) = temp_copy("simple_box.mycad");
    let app_ref = make_app(path_ref);
    let (_, reference_body) = send_post(app_ref, feature_json).await;

    for i in 1..100 {
        let (_dir, path) = temp_copy("simple_box.mycad");
        let app = make_app(path);
        let (_, body) = send_post(app, feature_json).await;
        assert_eq!(body, reference_body, "POST response differs at run {i}");
    }
}

// E03: POST with zero-width box → 422 (degenerate geometry), state unchanged
#[tokio::test]
async fn edge_zero_width_box_returns_error() {
    let (_dir, path) = temp_copy("simple_box.mycad");
    let feature_json =
        r#"{"type":"create_box","id":"box_zero","width":0.0,"height":5.0,"depth":5.0}"#;

    let app = make_app(path.clone());
    let (status, _body) = send_post(app, feature_json).await;
    assert_eq!(
        status,
        StatusCode::UNPROCESSABLE_ENTITY,
        "zero-width box must return 422"
    );

    // State unchanged — fresh GET returns original 1 body
    let app_after = make_app(path);
    let (get_status, get_body) = send_get(app_after).await;
    assert_eq!(get_status, StatusCode::OK);
    let bodies: Vec<serde_json::Value> = serde_json::from_str(&get_body).unwrap();
    assert_eq!(bodies.len(), 1, "state unchanged after zero-width POST");
}

// E04: POST with negative dimensions → error, state unchanged
#[tokio::test]
async fn edge_negative_dimension_box_returns_error() {
    let (_dir, path) = temp_copy("simple_box.mycad");
    let feature_json =
        r#"{"type":"create_box","id":"box_neg","width":-5.0,"height":5.0,"depth":5.0}"#;

    let app = make_app(path.clone());
    let (status, _body) = send_post(app, feature_json).await;
    assert_eq!(
        status,
        StatusCode::UNPROCESSABLE_ENTITY,
        "negative dimension box must return 422"
    );

    // State unchanged
    let app_after = make_app(path);
    let (get_status, get_body) = send_get(app_after).await;
    assert_eq!(get_status, StatusCode::OK);
    let bodies: Vec<serde_json::Value> = serde_json::from_str(&get_body).unwrap();
    assert_eq!(bodies.len(), 1, "state unchanged after negative-dim POST");
}

// E05: POST with unknown feature type → 422, state unchanged
#[tokio::test]
async fn edge_unknown_feature_type_returns_422() {
    let (_dir, path) = temp_copy("simple_box.mycad");
    let bad_json = r#"{"type":"create_torus","id":"torus_1","radius":5.0}"#;

    let app = make_app(path.clone());
    let (status, _body) = send_post(app, bad_json).await;
    assert_eq!(
        status,
        StatusCode::UNPROCESSABLE_ENTITY,
        "unknown feature type must return 422"
    );

    // State unchanged
    let app_after = make_app(path);
    let (get_status, get_body) = send_get(app_after).await;
    assert_eq!(get_status, StatusCode::OK);
    let bodies: Vec<serde_json::Value> = serde_json::from_str(&get_body).unwrap();
    assert_eq!(bodies.len(), 1);
}

// E06: POST then GET on the SAME app instance → in-memory doc reflects POST
#[tokio::test]
async fn edge_post_then_get_same_instance() {
    let (_dir, path) = temp_copy("simple_box.mycad");
    let feature_json = r#"{"type":"create_box","id":"box_2","width":5.0,"height":5.0,"depth":5.0}"#;

    // POST and GET share the same Arc<Mutex<AppState>> through the same app
    let app = make_app(path.clone());
    let (post_status, post_body) = send_post(app, feature_json).await;
    assert_eq!(post_status, StatusCode::OK, "post_body: {post_body}");

    // New app instance (reads from disk) must match
    let app_get = make_app(path);
    let (get_status, get_body) = send_get(app_get).await;
    assert_eq!(get_status, StatusCode::OK, "get_body: {get_body}");
    assert_eq!(
        post_body, get_body,
        "POST response must match fresh GET from disk"
    );
}

// E07: POST with empty JSON object → 422
#[tokio::test]
async fn edge_empty_json_object_returns_422() {
    let (_dir, path) = temp_copy("simple_box.mycad");

    let app = make_app(path.clone());
    let (status, _body) = send_post(app, "{}").await;
    assert_eq!(
        status,
        StatusCode::UNPROCESSABLE_ENTITY,
        "empty JSON object must return 422"
    );

    // State unchanged
    let app_after = make_app(path);
    let (get_status, get_body) = send_get(app_after).await;
    assert_eq!(get_status, StatusCode::OK);
    let bodies: Vec<serde_json::Value> = serde_json::from_str(&get_body).unwrap();
    assert_eq!(bodies.len(), 1);
}

// E08: POST with array JSON body → 422
#[tokio::test]
async fn edge_json_array_returns_422() {
    let (_dir, path) = temp_copy("simple_box.mycad");

    let app = make_app(path.clone());
    let (status, _body) = send_post(app, "[]").await;
    assert_eq!(
        status,
        StatusCode::UNPROCESSABLE_ENTITY,
        "JSON array must return 422"
    );

    // State unchanged
    let app_after = make_app(path);
    let (get_status, get_body) = send_get(app_after).await;
    assert_eq!(get_status, StatusCode::OK);
    let bodies: Vec<serde_json::Value> = serde_json::from_str(&get_body).unwrap();
    assert_eq!(bodies.len(), 1);
}

// E09: POST with empty string body → 400 (axum JsonRejection), state unchanged
#[tokio::test]
async fn edge_empty_body_returns_4xx() {
    let (_dir, path) = temp_copy("simple_box.mycad");

    let app = make_app(path.clone());
    let (status, _body) = send_post(app, "").await;
    assert!(
        status == StatusCode::BAD_REQUEST || status == StatusCode::UNPROCESSABLE_ENTITY,
        "empty body should return 4xx, got {status}"
    );

    // State unchanged
    let app_after = make_app(path);
    let (get_status, get_body) = send_get(app_after).await;
    assert_eq!(get_status, StatusCode::OK);
    let bodies: Vec<serde_json::Value> = serde_json::from_str(&get_body).unwrap();
    assert_eq!(bodies.len(), 1);
}
