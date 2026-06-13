use engawa_kernel::tessellation::TriangleMesh;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct BodyMesh {
    pub feature_id: String,
    pub mesh: TriangleMesh,
}
