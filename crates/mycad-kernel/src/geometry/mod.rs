pub mod curve;
pub mod math;
pub mod pcurve;
pub mod surface;
pub mod tolerance;

use nalgebra::{Point3, Vector3};
use serde::{Deserialize, Serialize};

pub use math::{
    angle_near, length_near, point_near, point_near_scaled, ANGLE_TOLERANCE, LENGTH_TOLERANCE,
    RELATIVE_TOLERANCE,
};

/// A 3D point with f64 precision.
pub type Point = Point3<f64>;

/// A 3D vector with f64 precision.
pub type Vec3 = Vector3<f64>;

/// A plane defined by an origin point and a normal vector.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Plane {
    pub origin: Point,
    pub normal: Vec3,
    pub u_axis: Vec3,
    pub v_axis: Vec3,
}

impl Plane {
    pub fn xy() -> Self {
        Self {
            origin: Point::origin(),
            normal: Vec3::z(),
            u_axis: Vec3::x(),
            v_axis: Vec3::y(),
        }
    }

    pub fn xz() -> Self {
        Self {
            origin: Point::origin(),
            normal: Vec3::y(),
            u_axis: Vec3::x(),
            v_axis: Vec3::z(),
        }
    }

    pub fn yz() -> Self {
        Self {
            origin: Point::origin(),
            normal: Vec3::x(),
            u_axis: Vec3::y(),
            v_axis: Vec3::z(),
        }
    }
}
