use super::math::orthonormal_basis;
use super::{Point, Vec3};
use serde::{Deserialize, Serialize};

/// Strategy for tessellating a face on a given surface type.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TessellationStrategy {
    /// Fan from first boundary vertex (convex planar face).
    BoundaryFan,
    /// Full UV grid patch (untrimmed periodic/rectangular face).
    UvGridFullPatch,
    /// UV sphere tessellation with polar fan + latitude bands.
    UvSphere,
    /// Not yet supported by the tessellator.
    Unsupported,
}

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

    /// Inverse of [`Surface::evaluate`]: find the parameters (u, v) whose
    /// evaluation lands on (or nearest to) `p`.
    ///
    /// Used so that geometry-consuming algorithms (e.g. tessellation) can ask
    /// for the surface normal at a concrete 3D point without assuming the
    /// surface is planar / has a constant normal.
    pub fn uv_of(&self, p: &Point) -> (f64, f64) {
        match self {
            Surface::Plane {
                origin,
                u_axis,
                v_axis,
                ..
            } => {
                let d = p - origin;
                (
                    d.dot(u_axis) / u_axis.dot(u_axis),
                    d.dot(v_axis) / v_axis.dot(v_axis),
                )
            }

            Surface::Cylinder { origin, axis, .. } => {
                let a = axis.normalize();
                let (bu, bv) = orthonormal_basis(axis);
                let d = p - origin;
                let v = d.dot(&a);
                let radial = d - v * a;
                (radial.dot(&bv).atan2(radial.dot(&bu)), v)
            }

            Surface::Sphere { center, radius } => {
                let d = p - center;
                let v = (d.z / radius).clamp(-1.0, 1.0).asin();
                let u = d.y.atan2(d.x);
                (u, v)
            }

            Surface::Cone { apex, axis, .. } => {
                let a = axis.normalize();
                let (bu, bv) = orthonormal_basis(axis);
                let d = p - apex;
                let v = d.dot(&a);
                let radial = d - v * a;
                (radial.dot(&bv).atan2(radial.dot(&bu)), v)
            }
        }
    }

    /// Surface normal at a concrete 3D point lying on (or near) the surface.
    ///
    /// Convenience over [`Surface::uv_of`] + [`Surface::normal_at`] so callers
    /// never have to special-case planar vs curved surfaces.
    pub fn normal_at_point(&self, p: &Point) -> Vec3 {
        let (u, v) = self.uv_of(p);
        self.normal_at(u, v)
    }

    /// Return the tessellation strategy for this surface type.
    pub fn tessellation_strategy(&self) -> TessellationStrategy {
        match self {
            Surface::Plane { .. } => TessellationStrategy::BoundaryFan,
            Surface::Cylinder { .. } => TessellationStrategy::UvGridFullPatch,
            Surface::Sphere { .. } => TessellationStrategy::UvSphere,
            Surface::Cone { .. } => TessellationStrategy::Unsupported,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use approx::assert_relative_eq;

    fn assert_uv_roundtrip(surface: &Surface, u: f64, v: f64) {
        let p = surface.evaluate(u, v);
        let (u2, v2) = surface.uv_of(&p);
        assert_relative_eq!(u, u2, epsilon = 1e-9);
        assert_relative_eq!(v, v2, epsilon = 1e-9);
    }

    #[test]
    fn test_uv_of_roundtrip_plane() {
        let surface = Surface::Plane {
            origin: Point::new(1.0, 2.0, 3.0),
            normal: Vec3::z(),
            u_axis: Vec3::x(),
            v_axis: Vec3::y(),
        };
        assert_uv_roundtrip(&surface, 0.7, -1.3);
    }

    #[test]
    fn test_uv_of_roundtrip_cylinder() {
        let surface = Surface::Cylinder {
            origin: Point::new(0.0, 0.0, 0.0),
            axis: Vec3::z(),
            radius: 2.5,
        };
        assert_uv_roundtrip(&surface, 0.9, 1.7);
    }

    #[test]
    fn test_uv_of_roundtrip_sphere() {
        let surface = Surface::Sphere {
            center: Point::new(-1.0, 0.5, 2.0),
            radius: 3.0,
        };
        assert_uv_roundtrip(&surface, 1.1, 0.6);
    }

    #[test]
    fn test_uv_of_roundtrip_cone() {
        let surface = Surface::Cone {
            apex: Point::new(0.0, 0.0, 0.0),
            axis: Vec3::z(),
            half_angle: 0.5,
        };
        assert_uv_roundtrip(&surface, 0.8, 2.0);
    }

    /// A curved surface must report different normals at different points —
    /// the property the tessellator now relies on instead of a single
    /// per-face normal.
    #[test]
    fn test_sphere_normal_varies_per_point() {
        let surface = Surface::Sphere {
            center: Point::origin(),
            radius: 1.0,
        };
        let p1 = surface.evaluate(0.0, 0.0);
        let p2 = surface.evaluate(std::f64::consts::FRAC_PI_2, 0.3);
        let n1 = surface.normal_at_point(&p1);
        let n2 = surface.normal_at_point(&p2);
        // Distinct surface points yield non-parallel outward normals.
        assert!((n1.dot(&n2)).abs() < 0.99);
    }

    #[test]
    fn test_plane_normal_constant_per_point() {
        let surface = Surface::Plane {
            origin: Point::origin(),
            normal: Vec3::z(),
            u_axis: Vec3::x(),
            v_axis: Vec3::y(),
        };
        let n1 = surface.normal_at_point(&Point::new(5.0, -2.0, 0.0));
        let n2 = surface.normal_at_point(&Point::new(-3.0, 4.0, 0.0));
        assert_relative_eq!(n1.dot(&n2), 1.0, epsilon = 1e-12);
    }
}
