use super::Point;
use super::Vec3;

// ---------------------------------------------------------------------------
// Public tolerance constants (preliminary — Phase 4 will migrate to a
// tolerant model per ADR-004 Decision 3).
// ---------------------------------------------------------------------------

/// Absolute tolerance for length / coordinate comparisons (mm).
pub const LENGTH_TOLERANCE: f64 = 1e-9;

/// Absolute tolerance for angle / parameter comparisons (rad).
pub const ANGLE_TOLERANCE: f64 = 1e-9;

/// Scale-proportional relative tolerance (dimensionless).
pub const RELATIVE_TOLERANCE: f64 = 1e-9;

// ---------------------------------------------------------------------------
// Comparison helpers
// ---------------------------------------------------------------------------

/// True when two lengths differ by at most [`LENGTH_TOLERANCE`].
pub fn length_near(a: f64, b: f64) -> bool {
    (a - b).abs() <= LENGTH_TOLERANCE
}

/// True when two angles differ by at most [`ANGLE_TOLERANCE`].
pub fn angle_near(a: f64, b: f64) -> bool {
    (a - b).abs() <= ANGLE_TOLERANCE
}

/// True when two 3D points are within [`LENGTH_TOLERANCE`] Euclidean distance.
pub fn point_near(a: &Point, b: &Point) -> bool {
    (a - b).norm() <= LENGTH_TOLERANCE
}

/// True when two 3D points are within `scale * [`RELATIVE_TOLERANCE`]` Euclidean distance.
pub fn point_near_scaled(a: &Point, b: &Point, scale: f64) -> bool {
    (a - b).norm() <= scale * RELATIVE_TOLERANCE
}

// ---------------------------------------------------------------------------
// Orthonormal basis
// ---------------------------------------------------------------------------

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

/// Arc-proportional segment count.
///
/// Returns a segment count proportional to the arc's angular span `|t_end - t_start|`
/// relative to a full revolution (2π), using `base_segments` as the target count for 2π.
/// The result is at least 1.
pub fn arc_segment_count(t_start: f64, t_end: f64, base_segments: usize) -> usize {
    let span = (t_end - t_start).abs();
    let n = (base_segments as f64 * span / (2.0 * std::f64::consts::PI)).ceil() as usize;
    n.max(1)
}

/// Unwrap periodic UV coordinates so consecutive values are continuous.
/// Adjusts values to avoid jumps larger than π between consecutive entries.
pub fn unwrap_periodic_uv(u_list: &mut [f64]) {
    let pi = std::f64::consts::PI;
    for i in 1..u_list.len() {
        while u_list[i] - u_list[i - 1] > pi {
            u_list[i] -= 2.0 * pi;
        }
        while u_list[i] - u_list[i - 1] < -pi {
            u_list[i] += 2.0 * pi;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use approx::assert_relative_eq;

    // --- Orthonormal basis tests (pre-existing) ---

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

    // --- T01: Constant values ---

    #[test]
    fn test_t01_constant_values() {
        assert_eq!(LENGTH_TOLERANCE, 1e-9);
        assert_eq!(ANGLE_TOLERANCE, 1e-9);
        assert_eq!(RELATIVE_TOLERANCE, 1e-9);
    }

    // --- T02: Invariants (positive, small) ---

    #[test]
    fn test_t02_constants_are_small_positive() {
        assert!(LENGTH_TOLERANCE > 0.0 && LENGTH_TOLERANCE < 1e-3);
        assert!(ANGLE_TOLERANCE > 0.0 && ANGLE_TOLERANCE < 1e-3);
        assert!(RELATIVE_TOLERANCE > 0.0 && RELATIVE_TOLERANCE < 1e-3);
    }

    // --- T03: Helper boundary (<= tol → true, just above → false) ---

    #[test]
    fn test_t03_length_near_boundary() {
        assert!(length_near(0.0, LENGTH_TOLERANCE));
        assert!(!length_near(0.0, LENGTH_TOLERANCE * 1.000_001));
        // Symmetry
        assert!(length_near(LENGTH_TOLERANCE, 0.0));
    }

    #[test]
    fn test_t03_angle_near_boundary() {
        assert!(angle_near(0.0, ANGLE_TOLERANCE));
        assert!(!angle_near(0.0, ANGLE_TOLERANCE * 1.000_001));
    }

    #[test]
    fn test_t03_point_near_boundary() {
        let a = Point::origin();
        let b = Point::new(LENGTH_TOLERANCE, 0.0, 0.0);
        assert!(point_near(&a, &b));
        let c = Point::new(LENGTH_TOLERANCE * 1.000_001, 0.0, 0.0);
        assert!(!point_near(&a, &c));
    }

    // --- T04: Scaled boundary ---

    #[test]
    fn test_t04_point_near_scaled_boundary() {
        let a = Point::origin();
        let scale = 1000.0;
        let threshold = scale * RELATIVE_TOLERANCE;

        // Exactly at threshold → true (<=)
        let b = Point::new(threshold, 0.0, 0.0);
        assert!(point_near_scaled(&a, &b, scale));

        // Just above threshold → false
        let c = Point::new(threshold * 1.000_001, 0.0, 0.0);
        assert!(!point_near_scaled(&a, &c, scale));
    }

    // --- T05: Re-export reachability (compile-time check) ---

    #[test]
    fn test_t05_crate_root_reexport() {
        let _ = crate::LENGTH_TOLERANCE;
        let _ = crate::ANGLE_TOLERANCE;
        let _ = crate::RELATIVE_TOLERANCE;
    }

    // --- Edge-case tests (adversarial persona) ---

    #[test]
    fn test_length_near_identical() {
        assert!(length_near(0.0, 0.0));
        assert!(length_near(1e-30, 1e-30));
    }

    #[test]
    fn test_angle_near_negative_values() {
        assert!(angle_near(-ANGLE_TOLERANCE, 0.0));
        assert!(angle_near(-1.0, -1.0 + ANGLE_TOLERANCE * 0.5));
    }

    #[test]
    fn test_point_near_coincident() {
        let p = Point::new(1.0, 2.0, 3.0);
        assert!(point_near(&p, &p));
    }

    #[test]
    fn test_point_near_scaled_zero_scale() {
        let a = Point::origin();
        let b = Point::new(0.0, 0.0, 0.0);
        assert!(point_near_scaled(&a, &b, 0.0));
        // Non-zero distance with zero scale → always false
        let c = Point::new(1e-20, 0.0, 0.0);
        assert!(!point_near_scaled(&a, &c, 0.0));
    }

    #[test]
    fn test_length_near_large_values() {
        let large = 1e15;
        assert!(length_near(large, large));
        assert!(!length_near(large, large + 1.0));
    }

    #[test]
    fn test_determinism_100_runs() {
        for _ in 0..100 {
            assert!(length_near(0.0, LENGTH_TOLERANCE));
            assert!(!length_near(0.0, LENGTH_TOLERANCE * 1.000_001));
            assert!(point_near(
                &Point::origin(),
                &Point::new(LENGTH_TOLERANCE, 0.0, 0.0)
            ));
        }
    }

    // T10: unwrap_periodic_uv — seam crossing produces continuous output
    #[test]
    fn t10_unwrap_periodic_uv_seam_crossing() {
        use std::f64::consts::PI;
        // Values crossing the ±π seam
        let mut u = vec![2.0, 2.5, 3.0, -3.0, -2.5];
        unwrap_periodic_uv(&mut u);
        // After unwrap, consecutive values should differ by less than π
        for i in 1..u.len() {
            let diff = (u[i] - u[i - 1]).abs();
            assert!(diff < PI, "diff at {i}: {diff} >= π");
        }
    }

    #[test]
    fn t10_unwrap_periodic_uv_no_change_when_continuous() {
        let mut u = vec![0.0, 0.5, 1.0, 1.5, 2.0];
        let expected = u.clone();
        unwrap_periodic_uv(&mut u);
        for (i, (a, b)) in u.iter().zip(expected.iter()).enumerate() {
            assert!((a - b).abs() < 1e-12, "mismatch at {i}: {a} != {b}");
        }
    }

    #[test]
    fn t10_unwrap_periodic_uv_single_element() {
        let mut u = vec![3.14];
        unwrap_periodic_uv(&mut u);
        assert!((u[0] - 3.14).abs() < 1e-12);
    }

    #[test]
    fn t10_unwrap_periodic_uv_empty() {
        let mut u: Vec<f64> = vec![];
        unwrap_periodic_uv(&mut u);
        assert!(u.is_empty());
    }

    // --- arc_segment_count tests (T05) ---

    use std::f64::consts::PI;

    #[test]
    fn t05_arc_segment_count_full_circle() {
        assert_eq!(arc_segment_count(0.0, 2.0 * PI, 32), 32);
    }

    #[test]
    fn t05_arc_segment_count_quarter_circle() {
        assert_eq!(arc_segment_count(0.0, PI / 2.0, 32), 8);
    }

    #[test]
    fn t05_arc_segment_count_tiny_arc() {
        // 2π/64 arc with base=32 → ceil(32/64) = 1
        assert_eq!(arc_segment_count(0.0, 2.0 * PI / 64.0, 32), 1);
    }

    #[test]
    fn t05_arc_segment_count_zero_span() {
        assert_eq!(arc_segment_count(0.0, 0.0, 32), 1);
    }

    #[test]
    fn t05_arc_segment_count_determinism() {
        for _ in 0..100 {
            assert_eq!(arc_segment_count(0.0, 2.0 * PI, 32), 32);
            assert_eq!(arc_segment_count(0.0, PI / 2.0, 32), 8);
        }
    }

    // --- Edge-case tests for arc_segment_count (adversarial persona) ---

    /// base_segments = 0: ceil(0·span/2π) = 0, max(1) = 1
    #[test]
    fn edge_arc_base_segments_zero() {
        assert_eq!(arc_segment_count(0.0, PI, 0), 1);
    }

    /// Reversed range (t_start > t_end) produces same result as forward.
    #[test]
    fn edge_arc_reversed_range() {
        let fwd = arc_segment_count(0.0, PI, 32);
        let rev = arc_segment_count(PI, 0.0, 32);
        assert_eq!(fwd, rev);
    }

    /// NaN inputs must not panic; result >= 1.
    #[test]
    fn edge_arc_nan_no_panic() {
        let r = arc_segment_count(f64::NAN, 0.0, 32);
        assert!(r >= 1, "NaN start: expected >= 1, got {r}");
        let r2 = arc_segment_count(0.0, f64::NAN, 32);
        assert!(r2 >= 1, "NaN end: expected >= 1, got {r2}");
    }

    /// Infinity inputs must not panic; result >= 1.
    #[test]
    fn edge_arc_inf_no_panic() {
        let r = arc_segment_count(0.0, f64::INFINITY, 32);
        assert!(r >= 1, "Inf end: expected >= 1, got {r}");
        let r2 = arc_segment_count(f64::NEG_INFINITY, 0.0, 32);
        assert!(r2 >= 1, "NegInf start: expected >= 1, got {r2}");
    }

    /// f64::MIN_POSITIVE span: too small to produce even 1 segment → 1.
    #[test]
    fn edge_arc_min_positive_span() {
        assert_eq!(arc_segment_count(0.0, f64::MIN_POSITIVE, 32), 1);
    }

    /// -0.0 to +0.0: zero span → 1.
    #[test]
    fn edge_arc_neg_zero() {
        assert_eq!(arc_segment_count(-0.0, 0.0, 32), 1);
    }

    /// Span > 2π → proportional (4π with base=32 → 64).
    #[test]
    fn edge_arc_span_over_2pi() {
        assert_eq!(arc_segment_count(0.0, 4.0 * PI, 32), 64);
    }

    /// f64::MAX span: must not panic, result >= 1.
    #[test]
    fn edge_arc_f64_max_span() {
        let r = arc_segment_count(0.0, f64::MAX, 32);
        assert!(r >= 1, "f64::MAX span: expected >= 1, got {r}");
    }

    /// 100-run determinism for edge-case inputs.
    #[test]
    fn edge_arc_determinism_100_runs() {
        for _ in 0..100 {
            assert_eq!(arc_segment_count(0.0, 2.0 * PI, 32), 32);
            assert_eq!(arc_segment_count(0.0, PI / 2.0, 32), 8);
            assert_eq!(arc_segment_count(0.0, 0.0, 32), 1);
            assert_eq!(arc_segment_count(-0.0, 0.0, 32), 1);
            assert_eq!(arc_segment_count(0.0, f64::MIN_POSITIVE, 32), 1);
            assert_eq!(arc_segment_count(0.0, 4.0 * PI, 32), 64);
        }
    }
}
