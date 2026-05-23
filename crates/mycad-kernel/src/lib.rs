pub mod brep;
pub mod error;
pub mod features;
pub mod geometry;
pub mod primitives;
pub mod tessellation;

pub use brep::topology;
pub use error::KernelError;
pub use features::build_solid_from_features;
