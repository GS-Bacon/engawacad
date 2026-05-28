use thiserror::Error;

#[derive(Debug, Error)]
pub enum KernelError {
    #[error("empty feature list")]
    EmptyFeatureList,

    #[error("referenced body not found: {id}")]
    BodyNotFound { id: String },

    #[error("unsupported feature variant: {kind}")]
    UnsupportedFeature { kind: &'static str },

    #[error("invalid parameter: {kind}")]
    InvalidParameter { kind: &'static str },

    #[error("sketch not found: {sketch}")]
    SketchNotFound { sketch: String },

    #[error("duplicate feature id: {id}")]
    DuplicateFeatureId { id: String },
}
