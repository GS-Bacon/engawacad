use crate::error::ApiError;
use axum::extract::Query;
use axum::Json;
use mycad_build::build_solid_from_features;
use mycad_format::Document;
use mycad_kernel::brep::topology::IdGenerator;
use mycad_kernel::tessellation::{tessellate_solid_with, TessellationOptions, TriangleMesh};
use serde::Deserialize;
use std::path::Path;

const V0_TESSELLATION: TessellationOptions = TessellationOptions {
    angular_segments: 32,
    axial_segments: 1,
};

#[derive(Deserialize)]
pub(crate) struct MeshQuery {
    pub file: String,
}

pub(crate) async fn get_mesh(Query(q): Query<MeshQuery>) -> Result<Json<TriangleMesh>, ApiError> {
    let path = Path::new(&q.file);

    if path.is_relative() {
        return Err(ApiError::BadRequest(
            "relative path not allowed: file must be an absolute path".to_string(),
        ));
    }

    let canonical = std::fs::canonicalize(path).map_err(|e| match e.kind() {
        std::io::ErrorKind::NotFound => {
            ApiError::NotFound(format!("file not found: {}", path.display()))
        }
        _ => ApiError::BadRequest(format!("invalid path: {}", path.display())),
    })?;

    let doc = Document::from_path(&canonical)?;

    let root = &doc.root_component;
    if root.reference.is_some() || !root.children.is_empty() {
        return Err(ApiError::Unprocessable(
            "assembly/reference documents are not supported in v0".to_string(),
        ));
    }
    if root.features.is_empty() {
        return Err(ApiError::Unprocessable(
            "empty part: document has no features".to_string(),
        ));
    }

    let mut gen = IdGenerator::new(0);
    let solid = build_solid_from_features(&root.features, &mut gen)?;
    let mesh = tessellate_solid_with(&solid, &V0_TESSELLATION)?;
    Ok(Json(mesh))
}
