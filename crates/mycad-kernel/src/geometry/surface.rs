use super::math::orthonormal_basis;
use super::{Point, Vec3};
use serde::{Deserialize, Serialize};

/// A geometric surface in 3D space.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Surface {
    /// An infinite plane.
    Plane {
        origin: Point,
        normal: Vec3,
        u_axis: Vec3,
        v_axis: Vec3,
    },
    /// A cylindrical surface.
    Cylinder {
        origin: Point,
        axis: Vec3,
        radius: f64,
    },
    /// A spherical surface.
    Sphere { center: Point, radius: f64 },
    /// A conical surface.
    Cone {
        apex: Point,
        axis: Vec3,
        half_angle: f64,
    },
}

impl Surface {
    /// Evaluate the surface at parameters (u, v).
    pub fn evaluate(&self, u: f64, v: f64) -> Point {
        match self {
            Surface::Plane {
                origin,
                u_axis,
                v_axis,
                ..
            } => origin + u * u_axis + v * v_axis,

            Surface::Cylinder {
                origin,
                axis,
                radius,
            } => {
                let (bu, bv) = orthonormal_basis(axis);
                origin + *radius * (u.cos() * bu + u.sin() * bv) + v * axis.normalize()
            }

            Surface::Sphere { center, radius } => {
                // u = longitude [0, 2π), v = latitude [-π/2, π/2]
                let x = radius * v.cos() * u.cos();
                let y = radius * v.cos() * u.sin();
                let z = radius * v.sin();
                Point::new(center.x + x, center.y + y, center.z + z)
            }

            Surface::Cone {
                apex,
                axis,
                half_angle,
            } => {
                let (bu, bv) = orthonormal_basis(axis);
                let r = v * half_angle.tan();
                apex + v * axis.normalize() + r * (u.cos() * bu + u.sin() * bv)
            }
        }
    }

    /// Get the normal vector at parameters (u, v).
    pub fn normal_at(&self, u: f64, v: f64) -> Vec3 {
        match self {
            Surface::Plane { normal, .. } => normal.normalize(),

            Surface::Cylinder { axis, .. } => {
                let (bu, bv) = orthonormal_basis(axis);
                (u.cos() * bu + u.sin() * bv).normalize()
            }

            Surface::Sphere { center, .. } => {
                let p = self.evaluate(u, v);
                (p - center).normalize()
            }

            Surface::Cone {
                apex,
                axis,
                half_angle,
            } => {
                let p = self.evaluate(u, v);
                let a = axis.normalize();
                let to_point = p - apex;
                let along_axis = to_point.dot(&a) * a;
                let radial = to_point - along_axis;
                if radial.norm() < 1e-12 {
                    return a;
                }
                let radial_n = radial.normalize();
                (radial_n * half_angle.cos() - a * half_angle.sin()).normalize()
            }
        }
    }
}
