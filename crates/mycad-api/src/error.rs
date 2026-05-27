use crate::transport::ErrorResponse;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use mycad_format::error::FormatError;
use mycad_kernel::error::KernelError;
use mycad_kernel::tessellation::TessellationError;

#[derive(Debug, thiserror::Error)]
pub enum ApiError {
    #[error("{0}")]
    BadRequest(String),
    #[error("{0}")]
    NotFound(String),
    #[error("{0}")]
    Unprocessable(String),
    #[error("{0}")]
    Internal(String),
}

impl From<FormatError> for ApiError {
    fn from(err: FormatError) -> Self {
        match &err {
            FormatError::InvalidExtension(_) | FormatError::Yaml(_) => {
                ApiError::BadRequest(err.to_string())
            }
            FormatError::Io(io_err) => match io_err.kind() {
                std::io::ErrorKind::NotFound => ApiError::NotFound(err.to_string()),
                _ => ApiError::Internal(err.to_string()),
            },
            FormatError::InvalidReference { .. } => ApiError::Unprocessable(err.to_string()),
            FormatError::InvalidName { .. }
            | FormatError::DuplicateFeatureId { .. }
            | FormatError::EmptyProvenance { .. } => ApiError::Unprocessable(err.to_string()),
        }
    }
}

impl From<KernelError> for ApiError {
    fn from(err: KernelError) -> Self {
        ApiError::Unprocessable(err.to_string())
    }
}

impl From<TessellationError> for ApiError {
    fn from(err: TessellationError) -> Self {
        ApiError::Unprocessable(err.to_string())
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let (status, msg) = match &self {
            ApiError::BadRequest(_) => (StatusCode::BAD_REQUEST, self.to_string()),
            ApiError::NotFound(_) => (StatusCode::NOT_FOUND, self.to_string()),
            ApiError::Unprocessable(_) => (StatusCode::UNPROCESSABLE_ENTITY, self.to_string()),
            ApiError::Internal(_) => (StatusCode::INTERNAL_SERVER_ERROR, self.to_string()),
        };
        let body = ErrorResponse { error: msg };
        (status, Json(body)).into_response()
    }
}
