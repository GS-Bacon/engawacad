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

    #[error("boolean result is empty (no volume remains)")]
    EmptyBooleanResult,

    #[error("non-planar surface in boolean input: {kind}")]
    NonPlanarBooleanInput { kind: &'static str },

    #[error("boolean input solid is not closed manifold")]
    OpenBooleanInput,

    #[error("degenerate boolean intersection (only surface contact, zero volume)")]
    DegenerateBooleanContact,

    #[error("boolean fuse result is disjoint (would produce multiple solids)")]
    DisjointFuseResult,

    #[error(
        "boolean {op} result produced multiple positive-volume shells (would require multi-body)"
    )]
    MultipleOuterShellsResult { op: &'static str },

    #[error("boolean input missing entity name (required for derived name composition)")]
    MissingEntityName,

    #[error("unsupported boolean input: {reason}")]
    UnsupportedBooleanInput { reason: &'static str },

    #[error("boolean internal error: {0}")]
    BooleanInternal(String),
}
