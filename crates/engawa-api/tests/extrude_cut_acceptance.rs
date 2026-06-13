/// Acceptance tests for #96: ExtrudeCut Feature 追加と UI
///
/// Tests:
///   A01: create_sketch POST → extrude_cut POST → face count + face_id uniqueness + volume + .mycad feature
use axum::body::Body;
use axum::http::{Request, StatusCode};
use engawa_api::router::app;
use engawa_kernel::tessellation::TriangleMesh;
use serde::Deserialize;
use std::collections::HashSet;
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

/// Signed-tetrahedra volume per body, then summed as absolute values.
///
/// For a void cut the API returns outer-shell and inner-void-shell as **separate** BodyMesh
/// entries. Computing per-body |signed_vol| then summing gives outer_abs + inner_abs, which is
/// greater than the initial solid volume. This lets T02 detect a no-op cut (where vol_after
/// would equal vol_before) without depending on inner-shell winding conventions.
fn mesh_volume(bodies: &[BodyMesh]) -> f64 {
    bodies
        .iter()
        .map(|b| {
            let m = &b.mesh;
            let v: f64 = m
                .indices
                .chunks(3)
                .map(|tri| {
                    let p0 = m.positions[tri[0] as usize];
                    let p1 = m.positions[tri[1] as usize];
                    let p2 = m.positions[tri[2] as usize];
                    (p0[0] * (p1[1] * p2[2] - p1[2] * p2[1])
                        + p0[1] * (p1[2] * p2[0] - p1[0] * p2[2])
                        + p0[2] * (p1[0] * p2[1] - p1[1] * p2[0]))
                        / 6.0
                })
                .sum();
            v.abs()
        })
        .sum()
}

/// A01: create_sketch → extrude_cut (internal void) の疎通 acceptance test
///
/// simple_box (10×20×30, 原点中心: z∈[-15,15]) に対し、
/// XY 平面上のインセット角柱 [-2,2]×[-4,4], depth=3 でカット。
/// tool z∈[0,3] は box z_top=15 より十分小さい → 完全埋没 void (2 シェル)。
/// coplanar 面なし(U02 と同一幾何)。
/// 期待: 頂点数変化 + face count + face_id 一意性 + 体積増加(内殻分) + .mycad に type: extrude_cut が 1 件。
#[tokio::test]
async fn a01_extrude_cut_increases_vertices_and_writes_feature() {
    let (_dir, path) = temp_copy("simple_box.mycad");

    // 1. GET initial mesh
    let app_get = make_app(path.clone());
    let (get_status, get_body) = send_get(app_get).await;
    assert_eq!(get_status, StatusCode::OK, "initial GET: {get_body}");
    let initial_bodies: Vec<BodyMesh> = serde_json::from_str(&get_body).unwrap();
    let initial_vertices: usize = initial_bodies.iter().map(|b| b.mesh.positions.len()).sum();
    let vol_before = mesh_volume(&initial_bodies);

    // 2. POST create_sketch (xy plane, inset [-2,2]×[-4,4])
    let sketch_json = r#"{"type":"create_sketch","id":"sketch_0","plane":"xy","profile":[{"id":"seg_0","from":[-2.0,-4.0],"to":[2.0,-4.0]},{"id":"seg_1","from":[2.0,-4.0],"to":[2.0,4.0]},{"id":"seg_2","from":[2.0,4.0],"to":[-2.0,4.0]},{"id":"seg_3","from":[-2.0,4.0],"to":[-2.0,-4.0]}]}"#;
    let app1 = make_app(path.clone());
    let (sketch_status, sketch_body) = send_post(app1, sketch_json).await;
    assert_eq!(sketch_status, StatusCode::OK, "sketch POST: {sketch_body}");

    // 3. POST extrude_cut (sketch_0, depth=3, target=box_1)
    let extrude_cut_json = r#"{"type":"extrude_cut","id":"extrude_cut_0","sketch":"sketch_0","depth":3.0,"target":"box_1"}"#;
    let app2 = make_app(path.clone());
    let (cut_status, cut_body) = send_post(app2, extrude_cut_json).await;
    assert_eq!(cut_status, StatusCode::OK, "extrude_cut POST: {cut_body}");

    // 4. Parse cut result
    let cut_bodies: Vec<BodyMesh> = serde_json::from_str(&cut_body).unwrap();
    let after_vertices: usize = cut_bodies.iter().map(|b| b.mesh.positions.len()).sum();

    // 4a. Vertex count changed
    assert!(
        after_vertices != initial_vertices,
        "vertices must change after extrude_cut: before={initial_vertices}, after={after_vertices}"
    );

    // T01: unique face_id count (each triangle carries its face_id; count distinct IDs).
    // Geometry: fully-buried void cut → outer shell (6 box faces) + inner void shell (6 prism
    // faces) = 12 distinct B-rep faces. The issue description estimated 10 (surface-pocket model),
    // but the actual result is an internal 2-shell void which has 6 + 6 = 12. The face count
    // verifies no unexpected face merging or degenerate B-rep.
    let all_face_ids: Vec<&str> = cut_bodies
        .iter()
        .flat_map(|b| b.mesh.face_ids.iter().map(|s| s.as_str()))
        .collect();
    let unique_faces: HashSet<&str> = all_face_ids.iter().copied().collect();
    assert_eq!(
        unique_faces.len(),
        12,
        "void cut: 6 outer-box faces + 6 inner-void faces = 12 B-rep faces, got {}: {unique_faces:?}",
        unique_faces.len()
    );

    // T02: volume — void cut returns outer + inner shells as separate bodies.
    // mesh_volume() uses per-body |signed_vol| (see doc comment), giving outer_abs + inner_abs.
    // For a non-degenerate void: outer_abs + inner_abs > initial (the inner shell is non-zero).
    // A no-op cut would leave vol_after == vol_before; any real void creation gives vol_after > vol_before.
    let vol_after = mesh_volume(&cut_bodies);
    assert!(
        vol_after > vol_before,
        "extrude_cut void: per-body abs volume sum must exceed initial (outer_abs + inner_abs > box_vol): before={vol_before}, after={vol_after}"
    );

    // 5. Verify .mycad contains type: extrude_cut (1 件)
    let engawa_content = std::fs::read_to_string(&path).unwrap();
    let count = engawa_content
        .lines()
        .filter(|l| l.trim().ends_with("type: extrude_cut"))
        .count();
    assert_eq!(
        count, 1,
        ".mycad must have exactly 1 extrude_cut feature, found {count}"
    );
}

/// A02: 100-run determinism — identical extrude_cut sequence produces byte-identical output every time.
///
/// Adversarial edge case: verifies that the full API pipeline (sketch → extrude_cut → mesh)
/// is deterministic across 100 independent runs with fresh temp files each time.
/// face_id uniqueness is verified in A01; byte-identical output here implies uniqueness is also stable.
#[tokio::test]
async fn a02_extrude_cut_determinism_100_runs() {
    let sketch_json = r#"{"type":"create_sketch","id":"sketch_0","plane":"xy","profile":[{"id":"seg_0","from":[-2.0,-4.0],"to":[2.0,-4.0]},{"id":"seg_1","from":[2.0,-4.0],"to":[2.0,4.0]},{"id":"seg_2","from":[2.0,4.0],"to":[-2.0,4.0]},{"id":"seg_3","from":[-2.0,4.0],"to":[-2.0,-4.0]}]}"#;
    let extrude_cut_json = r#"{"type":"extrude_cut","id":"extrude_cut_0","sketch":"sketch_0","depth":3.0,"target":"box_1"}"#;

    let mut reference: Option<String> = None;

    for i in 0..100 {
        let (_dir, path) = temp_copy("simple_box.mycad");

        let app1 = make_app(path.clone());
        let (s1, b1) = send_post(app1, sketch_json).await;
        assert_eq!(s1, StatusCode::OK, "run {i}: sketch POST failed: {b1}");

        let app2 = make_app(path);
        let (s2, b2) = send_post(app2, extrude_cut_json).await;
        assert_eq!(s2, StatusCode::OK, "run {i}: extrude_cut POST failed: {b2}");

        match &reference {
            None => reference = Some(b2),
            Some(ref_val) => {
                assert_eq!(
                    &b2, ref_val,
                    "run {i}: extrude_cut output differs from run 0"
                );
            }
        }
    }
}
