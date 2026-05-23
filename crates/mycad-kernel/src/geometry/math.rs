use super::Vec3;

/// Build an orthonormal basis (u, v) from a normal vector.
///
/// Given a normal vector, computes two perpendicular unit vectors
/// that together with the normal form a right-handed coordinate system.
pub fn orthonormal_basis(normal: &Vec3) -> (Vec3, Vec3) {
    let n = normal.normalize();
    // Choose a vector not parallel to n
    let not_parallel = if n.x.abs() < 0.9 {
        Vec3::x()
    } else {
        Vec3::y()
    };
    let u = n.cross(&not_parallel).normalize();
    let v = n.cross(&u);
    (u, v)
}

#[cfg(test)]
mod tests {
    use super::*;
    use approx::assert_relative_eq;

    #[test]
    fn test_orthonormal_basis_z_axis() {
        let (u, v) = orthonormal_basis(&Vec3::z());
        assert_relative_eq!(u.norm(), 1.0, epsilon = 1e-12);
        assert_relative_eq!(v.norm(), 1.0, epsilon = 1e-12);
        assert_relative_eq!(u.dot(&v), 0.0, epsilon = 1e-12);
        assert_relative_eq!(u.dot(&Vec3::z()), 0.0, epsilon = 1e-12);
    }

    #[test]
    fn test_orthonormal_basis_x_axis() {
        let (u, v) = orthonormal_basis(&Vec3::x());
        assert_relative_eq!(u.norm(), 1.0, epsilon = 1e-12);
        assert_relative_eq!(v.norm(), 1.0, epsilon = 1e-12);
        assert_relative_eq!(u.dot(&v), 0.0, epsilon = 1e-12);
        assert_relative_eq!(u.dot(&Vec3::x()), 0.0, epsilon = 1e-12);
    }
}
