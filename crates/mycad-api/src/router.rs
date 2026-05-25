use crate::handler::get_mesh;
use axum::body::Body;
use axum::http::{Request, StatusCode};
use axum::middleware::{self, Next};
use axum::response::Response;
use axum::routing::get;
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

pub fn app() -> Router {
    Router::new()
        .nest("/api/v0", Router::new().route("/mesh", get(get_mesh)))
        .layer(middleware::from_fn(host_guard))
}
