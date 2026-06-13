/// Fuzz harness for POST /api/v0/features (Issue #124)
/// Run with: cargo xtask acceptance --fuzz
use axum::body::Body;
use axum::http::{Request, StatusCode};
use engawa_api::router::app;
use rand::Rng;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tower::ServiceExt;

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

// ---- Random JSON generation ----

fn random_string(rng: &mut impl Rng, max_len: usize) -> String {
    let len = rng.random_range(0..max_len);
    (0..len)
        .map(|_| {
            let code = rng.random_range(0x20u32..0x7f);
            char::from_u32(code).unwrap_or('x')
        })
        .collect()
}

fn random_json_value(rng: &mut impl Rng, depth: u32) -> serde_json::Value {
    if depth > 3 {
        return serde_json::Value::Null;
    }
    match rng.random_range(0..7) {
        0 => serde_json::Value::Null,
        1 => serde_json::Value::Bool(rng.random()),
        2 => {
            let n: i64 = rng.random();
            serde_json::Value::Number(serde_json::Number::from(n))
        }
        3 => {
            let f: f64 = rng.random();
            serde_json::Number::from_f64(f)
                .map(serde_json::Value::Number)
                .unwrap_or(serde_json::Value::Null)
        }
        4 => serde_json::Value::String(random_string(rng, 10)),
        5 => {
            let len = rng.random_range(0..5);
            let arr: Vec<serde_json::Value> = (0..len)
                .map(|_| random_json_value(rng, depth + 1))
                .collect();
            serde_json::Value::Array(arr)
        }
        _ => {
            let len = rng.random_range(0..8);
            let obj: serde_json::Map<String, serde_json::Value> = (0..len)
                .map(|_| (random_string(rng, 8), random_json_value(rng, depth + 1)))
                .collect();
            serde_json::Value::Object(obj)
        }
    }
}

fn random_feature_json(rng: &mut impl Rng) -> serde_json::Value {
    let feature_types = [
        "create_box",
        "create_cylinder",
        "create_sphere",
        "create_sketch",
        "extrude",
        "extrude_cut",
        "cut",
        "fuse",
        "intersect",
        "",
        "unknown_type",
        "CREATE_BOX",
    ];

    let mut obj = serde_json::Map::new();

    let type_val = feature_types[rng.random_range(0..feature_types.len())];
    obj.insert(
        "type".to_string(),
        serde_json::Value::String(type_val.to_string()),
    );

    // id field — sometimes valid string, sometimes random value
    if rng.random::<bool>() {
        obj.insert(
            "id".to_string(),
            serde_json::Value::String(random_string(rng, 10)),
        );
    } else {
        obj.insert("id".to_string(), random_json_value(rng, 0));
    }

    // Numeric fields with boundary values
    for field in &["width", "height", "depth", "radius"] {
        if rng.random::<bool>() {
            let val = match rng.random_range(0..10) {
                0 => 0.0_f64,
                1 => 1e15_f64,
                2 => -1e15_f64,
                3 => 1e-15_f64,
                4 => f64::MIN,
                5 => f64::MAX,
                _ => rng.random::<f64>(),
            };
            if let Some(n) = serde_json::Number::from_f64(val) {
                obj.insert(field.to_string(), serde_json::Value::Number(n));
            } else {
                // NaN / Infinity — send null to test graceful handling
                obj.insert(field.to_string(), serde_json::Value::Null);
            }
        }
    }

    // Extra random keys
    if rng.random::<bool>() {
        let extra = rng.random_range(1..5);
        for _ in 0..extra {
            obj.insert(random_string(rng, 8), random_json_value(rng, 1));
        }
    }

    serde_json::Value::Object(obj)
}

// T01: Determinism — same payload to two independent apps → matching status and body
#[test]
#[ignore = "fuzz: run with cargo xtask acceptance --fuzz"]
fn t01_determinism() {
    let rt = tokio::runtime::Runtime::new().unwrap();
    rt.block_on(async {
        let payloads = [
            r#"{"type":"create_box","id":"box_2","width":5.0,"height":5.0,"depth":5.0}"#,
            r#"{"type":"unknown","id":"x","width":1.0}"#,
            r#"{"type":"create_box","id":"box_1","width":0.0,"height":0.0,"depth":0.0}"#,
            "null",
            "[]",
            "{}",
            "42",
            r#"{"type":null,"id":null}"#,
            r#"{"type":"extrude","id":"e1","sketch":"nonexistent","depth":5.0}"#,
        ];

        for payload in &payloads {
            let (_dir1, path1) = temp_copy("simple_box.mycad");
            let app1 = make_app(path1);
            let (status1, body1) = send_post(app1, payload).await;

            let (_dir2, path2) = temp_copy("simple_box.mycad");
            let app2 = make_app(path2);
            let (status2, body2) = send_post(app2, payload).await;

            assert_eq!(
                status1, status2,
                "determinism violation (status) for payload={payload}: {status1} != {status2}"
            );
            assert_eq!(
                body1, body2,
                "determinism violation (body) for payload={payload}"
            );
        }
    });
}

// T02: Fuzz — random JSON injection for FUZZ_DURATION_SECS, assert 0 HTTP 500s
#[test]
#[ignore = "fuzz: run with cargo xtask acceptance --fuzz"]
fn t02_fuzz_no_http_500() {
    let rt = tokio::runtime::Runtime::new().unwrap();
    rt.block_on(async {
        let duration_secs: u64 = std::env::var("FUZZ_DURATION_SECS")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(30);

        let mut rng = rand::rng();
        let deadline = Instant::now() + Duration::from_secs(duration_secs);
        let mut total = 0u64;
        let mut http_500_count = 0u64;
        let mut errors: Vec<String> = Vec::new();

        while Instant::now() < deadline {
            let (_dir, path) = temp_copy("simple_box.mycad");
            let app = make_app(path);

            let payload = match rng.random_range(0..10) {
                0..=5 => random_feature_json(&mut rng).to_string(),
                6..=7 => random_json_value(&mut rng, 0).to_string(),
                8 => "null".to_string(),
                _ => "{}".to_string(),
            };

            let (status, body) = send_post(app, &payload).await;
            total += 1;

            if status == StatusCode::INTERNAL_SERVER_ERROR {
                http_500_count += 1;
                errors.push(format!("HTTP 500 — payload: {payload}\n  response: {body}"));
            }
        }

        eprintln!(
            "\n=== Fuzz Results ===\n\
             Total requests: {total}\n\
             HTTP 500 count: {http_500_count}"
        );
        for err in &errors {
            eprintln!("{err}");
        }

        assert_eq!(
            http_500_count, 0,
            "fuzz found {http_500_count} HTTP 500 responses out of {total} requests"
        );
    });
}

// T03: NaN/Infinity を depth に入れても 500 を返さない（有効 JSON は non-500、無効 JSON は 422）
#[test]
#[ignore = "fuzz: run with cargo xtask acceptance --fuzz"]
fn t03_degen_special_float_values() {
    let rt = tokio::runtime::Runtime::new().unwrap();
    rt.block_on(async {
        // 有効な JSON 数値（extreme values）— 500 でなければ OK
        let valid_json_payloads = [
            r#"{"type":"create_box","id":"box_big","width":1e15,"height":1e15,"depth":1e15}"#,
            r#"{"type":"create_box","id":"box_neg","width":-1e15,"height":-1e15,"depth":-1e15}"#,
            r#"{"type":"create_box","id":"box_tiny","width":1e-15,"height":1e-15,"depth":1e-15}"#,
            r#"{"type":"create_box","id":"box_min","width":-1.7976931348623157e308,"height":1.0,"depth":1.0}"#,
        ];
        for payload in &valid_json_payloads {
            let (_dir, path) = temp_copy("simple_box.mycad");
            let app = make_app(path);
            let (status, body) = send_post(app, payload).await;
            assert_ne!(
                status,
                StatusCode::INTERNAL_SERVER_ERROR,
                "extreme float payload caused HTTP 500 — payload={payload}, body={body}"
            );
        }

        // 生バイトで送る NaN/Infinity（JSON 仕様外）— 422 Unprocessable を期待
        let raw_invalid_payloads = [
            r#"{"type":"create_box","id":"nan_d","width":1.0,"height":1.0,"depth":NaN}"#,
            r#"{"type":"create_box","id":"inf_d","width":1.0,"height":1.0,"depth":Infinity}"#,
            r#"{"type":"create_box","id":"neginf_w","width":-Infinity,"height":1.0,"depth":1.0}"#,
        ];
        for payload in &raw_invalid_payloads {
            let (_dir, path) = temp_copy("simple_box.mycad");
            let app = make_app(path);
            let (status, _body) = send_post(app, payload).await;
            assert!(
                status.is_client_error(),
                "NaN/Infinity raw bytes should return 4xx — payload={payload}, got={status}"
            );
        }
    });
}

// T04: 未知の type 文字列は 422 Unprocessable を返す
#[test]
#[ignore = "fuzz: run with cargo xtask acceptance --fuzz"]
fn t04_degen_unknown_type() {
    let rt = tokio::runtime::Runtime::new().unwrap();
    rt.block_on(async {
        let unknown_payloads = [
            r#"{"type":"totally_unknown","id":"x"}"#,
            r#"{"type":"","id":"x"}"#,
            r#"{"type":"CREATE_BOX","id":"x","width":1.0,"height":1.0,"depth":1.0}"#,
            r#"{"type":"null","id":"x"}"#,
        ];

        for payload in &unknown_payloads {
            let (_dir, path) = temp_copy("simple_box.mycad");
            let app = make_app(path);
            let (status, body) = send_post(app, payload).await;
            assert_eq!(
                status,
                StatusCode::UNPROCESSABLE_ENTITY,
                "unknown type should return 422 — got {status} for payload={payload}, body={body}"
            );
        }
    });
}

// T05: null body / 非オブジェクト body で 4xx を返す（500 でない）
#[test]
#[ignore = "fuzz: run with cargo xtask acceptance --fuzz"]
fn t05_degen_null_body() {
    let rt = tokio::runtime::Runtime::new().unwrap();
    rt.block_on(async {
        let invalid_payloads = ["null", "[]", "42", r#""string""#, "true", "{}"];

        for payload in &invalid_payloads {
            let (_dir, path) = temp_copy("simple_box.mycad");
            let app = make_app(path);
            let (status, body) = send_post(app, payload).await;
            assert!(
                status.is_client_error(),
                "non-object body should return 4xx — got {status} for payload={payload}, body={body}"
            );
        }
    });
}
