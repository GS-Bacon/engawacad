use crate::handler::get_mesh;
use crate::static_assets::static_handler;
use crate::transport::ErrorResponse;
use axum::body::Body;
use axum::http::{Request, StatusCode};
use axum::middleware::{self, Next};
use axum::response::Response;
use axum::routing::get;
use axum::Json;
use axum::Router;

async fn host_guard(req: Request<Body>, next: Next) -> Result<Response, StatusCode> {
    let host = req
        .headers()
        .get("host")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    let host_base = host.split(':').next().unwrap_or("");
    if matches!(host_base, "127.0.0.1" | "localhost" | "") {
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

pub fn app() -> Router {
    let api = Router::new()
        .route("/mesh", get(get_mesh))
        .fallback(api_not_found);

    Router::new()
        .nest("/api/v0", api)
        .fallback(static_handler)
        .layer(middleware::from_fn(host_guard))
}
