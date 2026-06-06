use crate::error::ApiError;
use crate::transport::BodyMesh;
use axum::extract::State;
use axum::Json;
use mycad_build::build_assembly;
use mycad_format::Document;
use mycad_kernel::brep::topology::IdGenerator;
use mycad_kernel::tessellation::{tessellate_solid_with, TessellationError, TessellationOptions};
use std::path::PathBuf;
use std::sync::Arc;

const V0_TESSELLATION: TessellationOptions = TessellationOptions {
    angular_segments: 32,
    axial_segments: 1,
};

pub(crate) async fn get_mesh(
    State(file): State<Arc<PathBuf>>,
) -> Result<Json<Vec<BodyMesh>>, ApiError> {
    let doc = Document::from_path(file.as_path())?;

    let base_dir = file
        .parent()
        .map(|p| p.to_path_buf())
        .unwrap_or_else(|| std::path::PathBuf::from("."));
    let mut gen = IdGenerator::new(0);
    let bodies = build_assembly(&doc, &base_dir, &mut gen)?;
    if bodies.is_empty() {
        return Err(ApiError::Unprocessable(
            "empty assembly: no bodies built".to_string(),
        ));
    }
    let out: Vec<BodyMesh> = bodies
        .iter()
        .map(|b| -> Result<BodyMesh, TessellationError> {
            Ok(BodyMesh {
                feature_id: b.feature_id.clone(),
                mesh: tessellate_solid_with(&b.solid, &V0_TESSELLATION)?,
            })
        })
        .collect::<Result<_, _>>()?;
    Ok(Json(out))
}
