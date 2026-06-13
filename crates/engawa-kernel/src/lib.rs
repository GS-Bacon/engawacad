pub mod booleans;
pub mod brep;
pub mod error;
pub mod geometry;
pub mod primitives;
pub mod tessellation;

pub use booleans::BooleanOp;
pub use brep::topology;
pub use error::KernelError;
pub use geometry::pcurve::{Curve2D, Pcurve};
pub use geometry::tolerance::Tolerance;
pub use geometry::{ANGLE_TOLERANCE, LENGTH_TOLERANCE, RELATIVE_TOLERANCE};
