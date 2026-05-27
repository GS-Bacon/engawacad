use mycad_api::router::app;
use std::path::PathBuf;
use std::sync::Arc;

#[tokio::main]
async fn main() {
    let file = std::env::args()
        .nth(1)
        .map(PathBuf::from)
        .and_then(|p| std::fs::canonicalize(p).ok())
        .expect("Usage: mycad-api <path-to-file.mycad>");

    let listener = tokio::net::TcpListener::bind("127.0.0.1:7878")
        .await
        .expect("failed to bind to 127.0.0.1:7878");
    axum::serve(listener, app(Arc::new(file)))
        .await
        .expect("server error");
}
