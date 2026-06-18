use thiserror::Error;

#[derive(Debug, Error)]
#[non_exhaustive]
pub enum FormatError {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("YAML parse error: {0}")]
    Yaml(#[from] serde_yaml::Error),

    #[error("invalid file extension: expected .engawa, got {0:?}")]
    InvalidExtension(Option<String>),

    #[error("invalid reference {value:?}: {reason}")]
    InvalidReference { value: String, reason: &'static str },

    #[error("invalid name {value:?}: {reason}")]
    InvalidName { value: String, reason: &'static str },

    #[error("duplicate feature_id {id:?} in component {component:?}")]
    DuplicateFeatureId { id: String, component: String },

    #[error("empty provenance in derived reference with op {op:?}")]
    EmptyProvenance { op: String },

    #[error("non-finite position in feature {id:?}: {reason}")]
    InvalidPosition { id: String, reason: &'static str },

    #[error("duplicate ref_plane id {id:?} in component {component:?}")]
    DuplicateRefPlaneId { id: String, component: String },

    #[error("RefPlane '{id}' has non-finite offset (NaN or Inf)")]
    InvalidRefPlaneOffset { id: String },

    #[error("unknown schema_version {found}: this engawa-format supports up to {current}")]
    UnknownSchemaVersion { found: u32, current: u32 },
}
