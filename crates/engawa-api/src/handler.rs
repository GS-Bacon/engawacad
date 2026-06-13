use crate::error::ApiError;
use crate::state::AppState;
use crate::transport::BodyMesh;
use axum::extract::rejection::JsonRejection;
use axum::extract::State;
use axum::Json;
use engawa_build::build_assembly;
use engawa_format::{Document, Feature};
use engawa_kernel::brep::topology::IdGenerator;
use engawa_kernel::tessellation::{tessellate_solid_with, TessellationError, TessellationOptions};
use std::path::Path;
use std::sync::{Arc, Mutex};

const V0_TESSELLATION: TessellationOptions = TessellationOptions {
    angular_segments: 32,
    axial_segments: 1,
};

type SharedState = Arc<Mutex<AppState>>;

/// GET/POST 双方が通る唯一の build→tessellate 経路（冪等性を構造保証）
fn assemble_and_tessellate(doc: &Document, base_dir: &Path) -> Result<Vec<BodyMesh>, ApiError> {
    let mut gen = IdGenerator::new(0);
    let bodies = build_assembly(doc, base_dir, &mut gen)?;
    if bodies.is_empty() {
        return Err(ApiError::Unprocessable(
            "empty assembly: no bodies built".to_string(),
        ));
    }
    bodies
        .iter()
        .map(|b| {
            Ok(BodyMesh {
                feature_id: b.feature_id.clone(),
                mesh: tessellate_solid_with(&b.solid, &V0_TESSELLATION)?,
            })
        })
        .collect::<Result<_, TessellationError>>()
        .map_err(Into::into)
}

/// Atomic write: write to tmp file then rename (same-FS atomic).
/// On rename failure, best-effort remove the tmp file (AM02).
fn write_atomic(path: &std::path::Path, content: &str) -> Result<(), ApiError> {
    let tmp = path.with_extension("engawa.tmp");
    std::fs::write(&tmp, content)?;
    if let Err(e) = std::fs::rename(&tmp, path) {
        let _ = std::fs::remove_file(&tmp);
        return Err(ApiError::Internal(e.to_string()));
    }
    Ok(())
}

pub(crate) async fn get_mesh(
    State(state): State<SharedState>,
) -> Result<Json<Vec<BodyMesh>>, ApiError> {
    let mut g = state.lock().unwrap();
    let base_dir = g.base_dir();
    let doc = g.ensure_loaded()?;
    Ok(Json(assemble_and_tessellate(doc, &base_dir)?))
}

pub(crate) async fn get_features(
    State(state): State<SharedState>,
) -> Result<Json<Vec<String>>, ApiError> {
    let mut g = state.lock().unwrap();
    let doc = g.ensure_loaded()?;
    let ids: Vec<String> = doc
        .root_component
        .features
        .iter()
        .map(|f| f.id().to_string())
        .collect();
    Ok(Json(ids))
}

pub(crate) async fn post_feature(
    State(state): State<SharedState>,
    feature_result: Result<Json<Feature>, JsonRejection>,
) -> Result<Json<Vec<BodyMesh>>, ApiError> {
    let Json(feature) = feature_result.map_err(|e| ApiError::Unprocessable(e.body_text()))?;
    let mut g = state.lock().unwrap();
    let base_dir = g.base_dir();
    let path = g.path.clone();
    g.ensure_loaded()?;

    // 順序不変条件 (IN01): validate → assemble_and_tessellate → write_atomic → commit
    // 失敗時はディスク・メモリとも未変更
    let mut candidate = g.snapshot()?;
    candidate.root_component.features.push(feature);
    candidate.validate()?;
    let meshes = assemble_and_tessellate(&candidate, &base_dir)?;

    let yaml = candidate.to_yaml()?;
    write_atomic(&path, &yaml)?;

    // 全成功時のみ in-memory 反映
    g.commit(candidate);
    Ok(Json(meshes))
}
