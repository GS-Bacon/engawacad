use crate::handler::{get_features, get_mesh, post_feature};
use crate::state::AppState;
use crate::static_assets::static_handler;
use crate::transport::ErrorResponse;
use axum::body::Body;
use axum::http::{Request, StatusCode};
use axum::middleware::{self, Next};
use axum::response::Response;
use axum::routing::get;
use axum::Json;
use axum::Router;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

fn is_allowed_host(host_base: &str) -> bool {
    host_base.is_empty()
        || host_base == "localhost"
        || host_base
            .chars()
            .all(|c| c.is_ascii_digit() || c == '.' || c == ':' || c == '[' || c == ']')
}

async fn host_guard(req: Request<Body>, next: Next) -> Result<Response, StatusCode> {
    let host = req
        .headers()
        .get("host")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    let host_base = host.split(':').next().unwrap_or("");
    if is_allowed_host(host_base) {
        Ok(next.run(req).await)
    } else {
        Err(StatusCode::FORBIDDEN)
    }
}

async fn api_not_found() -> (StatusCode, Json<ErrorResponse>) {
    (
        StatusCode::NOT_FOUND,
        Json(ErrorResponse {
            error: "not found".to_string(),
        }),
    )
}

pub fn app(file: Arc<PathBuf>) -> Router {
    let state: Arc<Mutex<AppState>> = Arc::new(Mutex::new(AppState::new((*file).clone())));
    let api = Router::new()
        .route("/mesh", get(get_mesh))
        .route("/features", get(get_features).post(post_feature))
        .fallback(api_not_found)
        .with_state(state);

    Router::new()
        .nest("/api/v0", api)
        .fallback(static_handler)
        .layer(middleware::from_fn(host_guard))
}
