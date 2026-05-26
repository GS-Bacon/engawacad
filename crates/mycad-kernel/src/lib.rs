pub mod brep;
pub mod error;
pub mod geometry;
pub mod primitives;
pub mod tessellation;

pub use brep::topology;
pub use error::KernelError;
pub use geometry::{ANGLE_TOLERANCE, LENGTH_TOLERANCE, RELATIVE_TOLERANCE};
