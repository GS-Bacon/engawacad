use axum::http::{header, StatusCode, Uri};
use axum::response::{Html, IntoResponse, Response};
use rust_embed::RustEmbed;

#[derive(RustEmbed)]
#[folder = "$CARGO_MANIFEST_DIR/../../web/dist"]
struct WebAssets;

pub(crate) async fn static_handler(uri: Uri) -> Response {
    let path = uri.path().trim_start_matches('/');

    let path = if path.is_empty() { "index.html" } else { path };

    match WebAssets::get(path) {
        Some(asset) => {
            let mime = mime_guess::from_path(path).first_or_octet_stream();
            if path == "index.html" {
                let body = String::from_utf8_lossy(&asset.data).into_owned();
                (
                    StatusCode::OK,
                    [(header::CONTENT_TYPE, "text/html; charset=utf-8")],
                    body,
                )
                    .into_response()
            } else {
                (
                    StatusCode::OK,
                    [(header::CONTENT_TYPE, mime.as_ref())],
                    asset.data.to_vec(),
                )
                    .into_response()
            }
        }
        None => {
            let is_file_request = std::path::Path::new(path).extension().is_some();
            if is_file_request {
                StatusCode::NOT_FOUND.into_response()
            } else {
                match WebAssets::get("index.html") {
                    Some(html) => {
                        Html(String::from_utf8_lossy(&html.data).into_owned()).into_response()
                    }
                    None => StatusCode::NOT_FOUND.into_response(),
                }
            }
        }
    }
}
