use super::{Point, Vec3};

/// Translate a point by the given offset. The sole primitive for translation.
/// Axes, normals, directions, radii, and angles are invariant under translation,
/// so this module provides no vector helpers (those will be added in #77 for rotation).
#[inline]
pub fn translate_point(p: Point, offset: Vec3) -> Point {
    p + offset
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
}
