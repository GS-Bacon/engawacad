use thiserror::Error;

#[derive(Debug, Error)]
pub enum KernelError {
    #[error("unsupported feature variant: {0}")]
    UnsupportedFeature(String),
}
