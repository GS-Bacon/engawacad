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

    #[error("invalid tolerance value: {value}")]
    InvalidTolerance { value: f64 },

    #[error("invalid pcurve t_range: t_start={t_start}, t_end={t_end}")]
    InvalidPcurveTrange { t_start: f64, t_end: f64 },

    #[error("degenerate pcurve: {reason}")]
    DegeneratePcurve { reason: &'static str },

    #[error("pcurve-surface mismatch at half-edge {he_idx}: deviation={deviation}, tolerance={tolerance}")]
    PcurveSurfaceMismatch {
        he_idx: usize,
        deviation: f64,
        tolerance: f64,
    },

    #[error("missing intersection provenance for op: {op}")]
    MissingIntersectionProvenance { op: String },

    #[error("manifold violation: {reason}")]
    ManifoldViolation { reason: &'static str },

    #[error("unsupported surface intersection: {reason}")]
    UnsupportedSurfaceIntersection { reason: &'static str },
}
