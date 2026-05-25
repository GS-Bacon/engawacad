use crate::error::ApiError;
use axum::extract::State;
use axum::Json;
use mycad_build::build_solid_from_features;
use mycad_format::Document;
use mycad_kernel::brep::topology::IdGenerator;
use mycad_kernel::tessellation::{tessellate_solid_with, TessellationOptions, TriangleMesh};
use std::path::PathBuf;
use std::sync::Arc;

const V0_TESSELLATION: TessellationOptions = TessellationOptions {
    angular_segments: 32,
    axial_segments: 1,
};

pub(crate) async fn get_mesh(
    State(file): State<Arc<PathBuf>>,
) -> Result<Json<TriangleMesh>, ApiError> {
    let doc = Document::from_path(file.as_path())?;

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
