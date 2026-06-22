//! 2D sketch element tessellation (polyline generation).
//!
//! Converts `SketchElement` variants (Line, Circle, Arc) into 2D point sequences
//! for downstream extrusion/profile operations.

use crate::error::KernelError;
use crate::geometry::math::{arc_segment_count, ANGLE_TOLERANCE, LENGTH_TOLERANCE};
use engawa_format::SketchElement;

/// Tessellate a sketch element into a 2D polyline.
///
/// - `Line`: returns `vec![from]` (single point, compatible with existing dispatcher behavior)
/// - `Circle`: returns `n` points uniformly spaced on full circle (CCW from +X, no duplicate)
/// - `Arc`: returns `n` points from `start_angle` to `end_angle` (CCW, endpoint excluded for chaining)
///
/// # Errors
/// - `DegenerateSketchElement` if `radius < LENGTH_TOLERANCE` or `|end_angle - start_angle| < ANGLE_TOLERANCE`
pub fn tessellate_sketch_element(
    elem: &SketchElement,
    base_segments: usize,
) -> Result<Vec<[f64; 2]>, KernelError> {
    match elem {
        SketchElement::Line { from, .. } => Ok(vec![*from]),
        SketchElement::Circle { id, center, radius } => {
            if *radius < LENGTH_TOLERANCE {
                return Err(KernelError::DegenerateSketchElement {
                    element_id: id.clone(),
                    reason: "radius < ε_radius",
                });
            }
            let n = arc_segment_count(0.0, std::f64::consts::TAU, base_segments);
            Ok((0..n)
                .map(|i| {
                    let t = (i as f64) * std::f64::consts::TAU / (n as f64);
                    [center[0] + radius * t.cos(), center[1] + radius * t.sin()]
                })
                .collect())
        }
        SketchElement::Arc {
            id,
            center,
            radius,
            start_angle,
            end_angle,
        } => {
            if *radius < LENGTH_TOLERANCE {
                return Err(KernelError::DegenerateSketchElement {
                    element_id: id.clone(),
                    reason: "radius < ε_radius",
                });
            }
            let sweep = end_angle - start_angle;
            if sweep.abs() < ANGLE_TOLERANCE {
                return Err(KernelError::DegenerateSketchElement {
                    element_id: id.clone(),
                    reason: "|end_angle - start_angle| < ε_angle",
                });
            }
            let n = arc_segment_count(*start_angle, *end_angle, base_segments);
            // Generate n points from start_angle (inclusive) towards end_angle (exclusive)
            // so that adjacent elements chain without duplicate vertices.
            Ok((0..n)
                .map(|i| {
                    let t = start_angle + (i as f64) * sweep / (n as f64);
                    [center[0] + radius * t.cos(), center[1] + radius * t.sin()]
                })
                .collect())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// T01: Determinism — same element produces same polyline on repeated calls.
    #[test]
    fn t01_tessellate_deterministic() {
        let circle = SketchElement::Circle {
            id: "c1".to_string(),
            center: [0.0, 0.0],
            radius: 1.0,
        };
        let pts1 = tessellate_sketch_element(&circle, 32).unwrap();
        let pts2 = tessellate_sketch_element(&circle, 32).unwrap();
        assert_eq!(pts1, pts2);

        let arc = SketchElement::Arc {
            id: "a1".to_string(),
            center: [0.0, 0.0],
            radius: 1.0,
            start_angle: 0.0,
            end_angle: std::f64::consts::PI / 2.0,
        };
        let pts1 = tessellate_sketch_element(&arc, 32).unwrap();
        let pts2 = tessellate_sketch_element(&arc, 32).unwrap();
        assert_eq!(pts1, pts2);
    }

    /// T02_circle: Circle tessellation produces points on unit circle.
    #[test]
    fn t02_circle_tessellation() {
        let circle = SketchElement::Circle {
            id: "c1".to_string(),
            center: [0.0, 0.0],
            radius: 1.0,
        };
        let pts = tessellate_sketch_element(&circle, 32).unwrap();
        assert_eq!(pts.len(), 32);
        for &p in &pts {
            let dist_sq = p[0] * p[0] + p[1] * p[1];
            assert!(
                (dist_sq - 1.0).abs() < 1e-9,
                "point not on unit circle: {p:?}"
            );
        }
    }

    /// T02_arc: Arc tessellation produces correct start/end points and count.
    #[test]
    fn t02_arc_tessellation() {
        let arc = SketchElement::Arc {
            id: "a1".to_string(),
            center: [0.0, 0.0],
            radius: 1.0,
            start_angle: 0.0,
            end_angle: std::f64::consts::PI / 2.0,
        };
        let pts = tessellate_sketch_element(&arc, 32).unwrap();
        // ceil(32 * (π/2) / (2π)) = ceil(8) = 8
        assert_eq!(pts.len(), 8);
        // First point ≈ (1, 0)
        assert!((pts[0][0] - 1.0).abs() < 1e-9);
        assert!(pts[0][1].abs() < 1e-9);
        // Last point is at angle ≈ π/2 * (7/8) = 7π/16 ≈ 0.4375π ≈ 0.137 rad before π/2
        // This is the expected behavior: polyline excludes end_angle for chaining.
        let last = pts.last().unwrap();
        // The last point should be close to (0, 1) but not exactly at π/2
        // Expected angle: start_angle + sweep * (n-1) / n = 0 + π/2 * 7/8 = 7π/16
        // Expected coords: (cos(7π/16), sin(7π/16)) ≈ (0.3827, 0.9239)
        // Verify it's near π/2 but before it (within angular tolerance)
        let expected_angle = std::f64::consts::PI / 2.0 * 7.0 / 8.0;
        let actual_x = last[0];
        let actual_y = last[1];
        let expected_x = expected_angle.cos();
        let expected_y = expected_angle.sin();
        assert!(
            (actual_x - expected_x).abs() < 1e-9,
            "x mismatch: {actual_x} vs {expected_x}"
        );
        assert!(
            (actual_y - expected_y).abs() < 1e-9,
            "y mismatch: {actual_y} vs {expected_y}"
        );
    }

    /// T_DEG_zero_radius: Zero-radius circle errors.
    #[test]
    fn t_degenerate_zero_radius() {
        let circle = SketchElement::Circle {
            id: "c0".to_string(),
            center: [0.0, 0.0],
            radius: 0.0,
        };
        let err = tessellate_sketch_element(&circle, 32).unwrap_err();
        assert!(matches!(
            err,
            KernelError::DegenerateSketchElement {
                element_id,
                reason: "radius < ε_radius"
            } if element_id == "c0"
        ));
    }

    /// T_DEG_zero_angle: Zero-sweep arc errors.
    #[test]
    fn t_degenerate_zero_angle() {
        let arc = SketchElement::Arc {
            id: "a0".to_string(),
            center: [0.0, 0.0],
            radius: 1.0,
            start_angle: 0.5,
            end_angle: 0.5,
        };
        let err = tessellate_sketch_element(&arc, 32).unwrap_err();
        assert!(matches!(
            err,
            KernelError::DegenerateSketchElement {
                element_id,
                reason: "|end_angle - start_angle| < ε_angle"
            } if element_id == "a0"
        ));
    }

    /// T_BOUNDARY_full_circle: Full-circle arc (0→2π) produces identical polyline to Circle.
    #[test]
    fn t_boundary_full_circle_matches_circle() {
        let circle = SketchElement::Circle {
            id: "c1".to_string(),
            center: [0.0, 0.0],
            radius: 1.0,
        };
        let arc = SketchElement::Arc {
            id: "a1".to_string(),
            center: [0.0, 0.0],
            radius: 1.0,
            start_angle: 0.0,
            end_angle: std::f64::consts::TAU,
        };
        let circle_pts = tessellate_sketch_element(&circle, 32).unwrap();
        let arc_pts = tessellate_sketch_element(&arc, 32).unwrap();
        assert_eq!(circle_pts.len(), arc_pts.len());
        for (c, a) in circle_pts.iter().zip(arc_pts.iter()) {
            let dist = ((c[0] - a[0]).powi(2) + (c[1] - a[1]).powi(2)).sqrt();
            assert!(dist < LENGTH_TOLERANCE, "points differ: {c:?} vs {a:?}");
        }
    }

    // === 敵対ペルソナ追加: エッジケース・数値境界テスト ===

    /// T_EDGE_nan_radius: NaN radius produces NaN points without panic (NaN < ε == false).
    #[test]
    fn t_edge_nan_radius_no_panic() {
        let circle = SketchElement::Circle {
            id: "nan_circle".to_string(),
            center: [0.0, 0.0],
            radius: f64::NAN,
        };
        // NaN < LENGTH_TOLERANCE は false なので退化判定されない
        // Tessellation は成功し、NaN を含む点列が返る（panic しないことを確認）
        let pts = tessellate_sketch_element(&circle, 32).unwrap();
        assert_eq!(pts.len(), 32);
        // 全ての点が NaN を含むことを確認
        for p in &pts {
            assert!(
                p[0].is_nan() || p[1].is_nan(),
                "expected NaN in point: {:?}",
                p
            );
        }
    }

    /// T_EDGE_inf_radius: Inf radius passes degeneracy check (Inf > LENGTH_TOLERANCE).
    #[test]
    fn t_edge_inf_radius_succeeds() {
        let circle = SketchElement::Circle {
            id: "inf_circle".to_string(),
            center: [0.0, 0.0],
            radius: f64::INFINITY,
        };
        // Tessellation は成功する（結果は Inf を含む点列）
        let pts = tessellate_sketch_element(&circle, 32).unwrap();
        // 最初の点は (Inf + cos*Inf) = Inf (cos は有限)
        assert!(pts[0][0].is_infinite());
    }

    /// T_EDGE_negative_zero_radius: -0.0 radius fails (| -0.0 | < LENGTH_TOLERANCE).
    #[test]
    fn t_edge_negative_zero_radius_fails() {
        let circle = SketchElement::Circle {
            id: "neg_zero_circle".to_string(),
            center: [0.0, 0.0],
            radius: -0.0,
        };
        let err = tessellate_sketch_element(&circle, 32).unwrap_err();
        assert!(matches!(
            err,
            KernelError::DegenerateSketchElement {
                element_id,
                reason: "radius < ε_radius"
            } if element_id == "neg_zero_circle"
        ));
    }

    /// T_EDGE_negative_radius: Negative radius fails degeneracy check.
    #[test]
    fn t_edge_negative_radius_fails() {
        let circle = SketchElement::Circle {
            id: "neg_circle".to_string(),
            center: [0.0, 0.0],
            radius: -1.0,
        };
        let err = tessellate_sketch_element(&circle, 32).unwrap_err();
        assert!(matches!(
            err,
            KernelError::DegenerateSketchElement {
                element_id,
                reason: "radius < ε_radius"
            } if element_id == "neg_circle"
        ));
    }

    /// T_EDGE_min_positive_radius: f64::MIN_POSITIVE fails (< LENGTH_TOLERANCE).
    #[test]
    fn t_edge_min_positive_radius_fails() {
        let circle = SketchElement::Circle {
            id: "tiny_circle".to_string(),
            center: [0.0, 0.0],
            radius: f64::MIN_POSITIVE,
        };
        let err = tessellate_sketch_element(&circle, 32).unwrap_err();
        assert!(matches!(
            err,
            KernelError::DegenerateSketchElement {
                element_id,
                reason: "radius < ε_radius"
            } if element_id == "tiny_circle"
        ));
    }

    /// T_EDGE_length_tolerance_boundary: radius = LENGTH_TOLERANCE passes.
    #[test]
    fn t_edge_length_tolerance_boundary_passes() {
        let circle = SketchElement::Circle {
            id: "eps_circle".to_string(),
            center: [0.0, 0.0],
            radius: LENGTH_TOLERANCE,
        };
        // ε 以上は成功
        let pts = tessellate_sketch_element(&circle, 32).unwrap();
        assert_eq!(pts.len(), 32);
    }

    /// T_EDGE_angle_tolerance_boundary: sweep = ANGLE_TOLERANCE passes.
    #[test]
    fn t_edge_angle_tolerance_boundary_passes() {
        let arc = SketchElement::Arc {
            id: "eps_arc".to_string(),
            center: [0.0, 0.0],
            radius: 1.0,
            start_angle: 0.0,
            end_angle: ANGLE_TOLERANCE,
        };
        // ε 以上は成功
        let pts = tessellate_sketch_element(&arc, 32).unwrap();
        // arc_segment_count(0, ε, 32) = ceil(32 * ε / 2π). ε = 1e-9 のとき 1
        assert_eq!(pts.len(), 1);
    }

    /// T_EDGE_angle_tolerance_minus_one: sweep = ANGLE_TOLERANCE - 1e-15 fails.
    #[test]
    fn t_edge_angle_tolerance_minus_one_fails() {
        let arc = SketchElement::Arc {
            id: "sub_eps_arc".to_string(),
            center: [0.0, 0.0],
            radius: 1.0,
            start_angle: 0.0,
            end_angle: ANGLE_TOLERANCE * 0.9,
        };
        let err = tessellate_sketch_element(&arc, 32).unwrap_err();
        assert!(matches!(
            err,
            KernelError::DegenerateSketchElement {
                element_id,
                reason: "|end_angle - start_angle| < ε_angle"
            } if element_id == "sub_eps_arc"
        ));
    }

    /// T_DETERMINISM_100_calls: Circle tessellation is deterministic across 100 calls.
    #[test]
    fn t_determinism_circle_100_calls() {
        let circle = SketchElement::Circle {
            id: "det_circle".to_string(),
            center: [3.5, 4.25],
            radius: 2.75,
        };
        let first = tessellate_sketch_element(&circle, 32).unwrap();
        for _ in 0..100 {
            let pts = tessellate_sketch_element(&circle, 32).unwrap();
            assert_eq!(pts, first, "determinism failed on repeated call");
        }
    }

    /// T_DETERMINISM_100_calls_arc: Arc tessellation is deterministic across 100 calls.
    #[test]
    fn t_determinism_arc_100_calls() {
        let arc = SketchElement::Arc {
            id: "det_arc".to_string(),
            center: [1.0, 2.0],
            radius: 1.5,
            start_angle: 0.25,
            end_angle: 2.5,
        };
        let first = tessellate_sketch_element(&arc, 32).unwrap();
        for _ in 0..100 {
            let pts = tessellate_sketch_element(&arc, 32).unwrap();
            assert_eq!(pts, first, "determinism failed on repeated call");
        }
    }

    /// T_EDGE_negative_sweep_arc: Arc with end_angle < start_angle (negative sweep) works.
    #[test]
    fn t_edge_negative_sweep_arc_succeeds() {
        let arc = SketchElement::Arc {
            id: "neg_sweep_arc".to_string(),
            center: [0.0, 0.0],
            radius: 1.0,
            start_angle: std::f64::consts::PI,
            end_angle: 0.0,
        };
        // 負の掃引角も |sweep| >= ε で成功
        let pts = tessellate_sketch_element(&arc, 32).unwrap();
        // 半周分 = 16 点
        assert_eq!(pts.len(), 16);
    }

    /// T_EDGE_multi_turn_arc: Arc with sweep > 2π (multi-turn) produces proportionally more points.
    #[test]
    fn t_edge_multi_turn_arc() {
        let arc = SketchElement::Arc {
            id: "multi_turn_arc".to_string(),
            center: [0.0, 0.0],
            radius: 1.0,
            start_angle: 0.0,
            end_angle: 4.0 * std::f64::consts::PI, // 2 turns
        };
        let pts = tessellate_sketch_element(&arc, 32).unwrap();
        // 2 周分 = 64 点
        assert_eq!(pts.len(), 64);
    }
}
