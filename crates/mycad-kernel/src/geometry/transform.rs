use super::{Point, Vec3};

/// Translate a point by the given offset. The sole primitive for translation.
/// Axes, normals, directions, radii, and angles are invariant under translation,
/// so this module provides no vector helpers (those will be added in #77 for rotation).
#[inline]
pub fn translate_point(p: Point, offset: Vec3) -> Point {
    p + offset
}

/// Snap near-zero / near-one / near-minus-one values for exact 90°/180°/270° matrices.
#[inline]
fn snap(v: f64) -> f64 {
    if v.abs() < 1e-15 {
        0.0
    } else if (v - 1.0).abs() < 1e-15 {
        1.0
    } else if (v + 1.0).abs() < 1e-15 {
        -1.0
    } else {
        v
    }
}

/// Euler angles (degrees) → 3×3 rotation matrix (ZYX order: R = Rx(rx) * Ry(ry) * Rz(rz)).
pub fn euler_to_matrix(rx_deg: f64, ry_deg: f64, rz_deg: f64) -> [[f64; 3]; 3] {
    let to_rad = std::f64::consts::PI / 180.0;
    let (sx, cx) = (snap((rx_deg * to_rad).sin()), snap((rx_deg * to_rad).cos()));
    let (sy, cy) = (snap((ry_deg * to_rad).sin()), snap((ry_deg * to_rad).cos()));
    let (sz, cz) = (snap((rz_deg * to_rad).sin()), snap((rz_deg * to_rad).cos()));
    [
        [cy * cz, -cy * sz, sy],
        [sx * sy * cz + cx * sz, -sx * sy * sz + cx * cz, -sx * cy],
        [-cx * sy * cz + sx * sz, cx * sy * sz + sx * cz, cx * cy],
    ]
}

/// Rotate a point around `pivot` using the given rotation matrix.
pub fn rotate_point(p: Point, matrix: [[f64; 3]; 3], pivot: Point) -> Point {
    let dp = p - pivot;
    let [r0, r1, r2] = matrix;
    let x = r0[0] * dp.x + r0[1] * dp.y + r0[2] * dp.z;
    let y = r1[0] * dp.x + r1[1] * dp.y + r1[2] * dp.z;
    let z = r2[0] * dp.x + r2[1] * dp.y + r2[2] * dp.z;
    pivot + Vec3::new(x, y, z)
}

/// Rotate a vector (normal, axis, direction) using the given rotation matrix (no translation).
pub fn rotate_vec(v: Vec3, matrix: [[f64; 3]; 3]) -> Vec3 {
    let [r0, r1, r2] = matrix;
    Vec3::new(
        r0[0] * v.x + r0[1] * v.y + r0[2] * v.z,
        r1[0] * v.x + r1[1] * v.y + r1[2] * v.z,
        r2[0] * v.x + r2[1] * v.y + r2[2] * v.z,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_translate_point_identity() {
        let p = Point::new(1.0, 2.0, 3.0);
        let result = translate_point(p, Vec3::new(0.0, 0.0, 0.0));
        assert_eq!(result, p);
    }

    #[test]
    fn test_translate_point_basic() {
        let p = Point::new(1.0, 2.0, 3.0);
        let offset = Vec3::new(10.0, 20.0, 30.0);
        let result = translate_point(p, offset);
        assert_eq!(result, Point::new(11.0, 22.0, 33.0));
    }

    /// T04: euler_to_matrix(0,0,0) → identity matrix (exact).
    #[test]
    fn t04_euler_identity() {
        let m = euler_to_matrix(0.0, 0.0, 0.0);
        let identity: [[f64; 3]; 3] = [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]];
        assert_eq!(m, identity);
    }

    /// T05: euler_to_matrix(90,0,0) produces snap-cleaned values.
    #[test]
    fn t05_euler_90_snap() {
        let m = euler_to_matrix(90.0, 0.0, 0.0);
        // Rx(90°) = [[1,0,0],[0,0,-1],[0,1,0]]
        assert_eq!(m[0], [1.0, 0.0, 0.0]);
        assert_eq!(m[1], [0.0, 0.0, -1.0]);
        assert_eq!(m[2], [0.0, 1.0, 0.0]);
    }

    /// Row norms = 1 (rotation is length-preserving).
    #[test]
    fn t_rotation_is_isometry() {
        let m = euler_to_matrix(30.0, 45.0, 60.0);
        for row in &m {
            let norm = (row[0] * row[0] + row[1] * row[1] + row[2] * row[2]).sqrt();
            assert!((norm - 1.0).abs() < 1e-12, "row norm must be 1, got {norm}");
        }
    }

    /// Rows are mutually orthogonal.
    #[test]
    fn t_rotation_rows_orthogonal() {
        let m = euler_to_matrix(30.0, 45.0, 60.0);
        for i in 0..3 {
            for j in (i + 1)..3 {
                let dot = m[i][0] * m[j][0] + m[i][1] * m[j][1] + m[i][2] * m[j][2];
                assert!(
                    dot.abs() < 1e-12,
                    "rows {i} and {j} not orthogonal, dot={dot}"
                );
            }
        }
    }
}
