use crate::error::KernelError;
use crate::geometry::math::LENGTH_TOLERANCE;
use serde::{Deserialize, Serialize};

/// Per-entity tolerance for geometric comparisons.
///
/// Construct via `Tolerance::new()` (validated) or `Tolerance::DEFAULT`.
/// YAML/JSON deserialization also validates through a custom `Deserialize` impl.
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd, Serialize)]
pub struct Tolerance(f64);

impl Tolerance {
    pub const DEFAULT: Tolerance = Tolerance(LENGTH_TOLERANCE);

    /// Create a tolerance value. Rejects non-positive, NaN, or infinite values.
    pub fn new(value: f64) -> Result<Tolerance, KernelError> {
        if !value.is_finite() || value <= 0.0 {
            return Err(KernelError::InvalidTolerance { value });
        }
        Ok(Tolerance(value))
    }

    pub fn value(self) -> f64 {
        self.0
    }

    pub fn max(self, other: Tolerance) -> Tolerance {
        if self.0 >= other.0 {
            self
        } else {
            other
        }
    }
}

impl<'de> Deserialize<'de> for Tolerance {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)]
        struct Shadow(f64);

        let shadow = Shadow::deserialize(deserializer)?;
        Tolerance::new(shadow.0).map_err(serde::de::Error::custom)
    }
}

/// True when two lengths differ by at most `tol`.
pub fn length_near_within(a: f64, b: f64, tol: Tolerance) -> bool {
    (a - b).abs() <= tol.value()
}

/// True when two 3D points are within `tol` Euclidean distance.
pub fn point_near_within(
    a: &crate::geometry::Point,
    b: &crate::geometry::Point,
    tol: Tolerance,
) -> bool {
    (a - b).norm() <= tol.value()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::geometry::Point;

    // T01: valid construction
    #[test]
    fn t01_tolerance_new_valid() {
        let t = Tolerance::new(1e-9).unwrap();
        assert_eq!(t.value(), 1e-9);
    }

    // T01b: YAML deserialize rejects invalid values
    #[test]
    fn t01b_yaml_reject_negative() {
        let result = serde_yaml::from_str::<Tolerance>("-1e-9");
        assert!(result.is_err());
    }

    #[test]
    fn t01b_yaml_reject_nan() {
        let result = serde_yaml::from_str::<Tolerance>(".NaN");
        assert!(result.is_err());
    }

    #[test]
    fn t01b_yaml_reject_infinity() {
        let result = serde_yaml::from_str::<Tolerance>(".inf");
        assert!(result.is_err());
    }

    // T02: invalid construction
    #[test]
    fn t02_reject_negative() {
        assert!(matches!(
            Tolerance::new(-1e-9),
            Err(KernelError::InvalidTolerance { .. })
        ));
    }

    #[test]
    fn t02_reject_nan() {
        assert!(matches!(
            Tolerance::new(f64::NAN),
            Err(KernelError::InvalidTolerance { .. })
        ));
    }

    #[test]
    fn t02_reject_infinity() {
        assert!(matches!(
            Tolerance::new(f64::INFINITY),
            Err(KernelError::InvalidTolerance { .. })
        ));
    }

    #[test]
    fn t02_reject_zero() {
        assert!(matches!(
            Tolerance::new(0.0),
            Err(KernelError::InvalidTolerance { .. })
        ));
    }

    // T03: max
    #[test]
    fn t03_max() {
        let a = Tolerance::new(0.1).unwrap();
        let b = Tolerance::new(0.5).unwrap();
        assert_eq!(Tolerance::max(a, b), b);
        assert_eq!(Tolerance::max(b, a), b);
    }

    // T04: comparison helpers
    #[test]
    fn t04_length_near_within_default() {
        assert!(length_near_within(1.0, 1.0 + 5e-10, Tolerance::DEFAULT));
        assert!(!length_near_within(1.0, 1.0 + 1e-6, Tolerance::DEFAULT));
    }

    #[test]
    fn t04_point_near_within() {
        let tol = Tolerance::new(1e-6).unwrap();
        let a = Point::new(0.0, 0.0, 0.0);
        let b = Point::new(5e-7, 0.0, 0.0);
        assert!(point_near_within(&a, &b, tol));

        let c = Point::new(2e-6, 0.0, 0.0);
        assert!(!point_near_within(&a, &c, tol));
    }

    // T01b: YAML roundtrip valid
    #[test]
    fn t01b_yaml_roundtrip_valid() {
        let t = Tolerance::new(1e-9).unwrap();
        let yaml = serde_yaml::to_string(&t).unwrap();
        let t2: Tolerance = serde_yaml::from_str(&yaml).unwrap();
        assert_eq!(t, t2);
    }

    // Adversarial: determinism
    #[test]
    fn test_determinism_100_runs() {
        for _ in 0..100 {
            let t = Tolerance::new(1e-9).unwrap();
            assert_eq!(t.value(), 1e-9);
        }
    }

    #[test]
    fn test_default_value() {
        assert_eq!(Tolerance::DEFAULT.value(), LENGTH_TOLERANCE);
    }

    #[test]
    fn test_partial_ord() {
        let a = Tolerance::new(1e-9).unwrap();
        let b = Tolerance::new(1e-6).unwrap();
        assert!(a < b);
        assert!(b > a);
    }

    #[test]
    fn test_reject_neg_infinity() {
        assert!(matches!(
            Tolerance::new(f64::NEG_INFINITY),
            Err(KernelError::InvalidTolerance { .. })
        ));
    }

    #[test]
    fn test_length_near_boundary() {
        let tol = Tolerance::new(1e-9).unwrap();
        assert!(length_near_within(0.0, 1e-9, tol));
        assert!(!length_near_within(0.0, 1e-9 + 1e-15, tol));
    }
}
