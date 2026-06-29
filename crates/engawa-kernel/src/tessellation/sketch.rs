//! 2D sketch element tessellation (polyline generation).
//!
//! Converts `SketchElement` variants (Line, Circle, Arc) into 2D point sequences
//! for downstream extrusion/profile operations.

use crate::error::KernelError;
use crate::geometry::math::{
    arc_segment_count, ANGLE_TOLERANCE, EPS_AXIS_RATIO, EPS_DISCRIMINANT, LENGTH_TOLERANCE,
};
use engawa_format::SketchElement;

/// Tessellate a sketch element into a 2D polyline.
///
/// - `Line`: returns `vec![from]` (single point, compatible with existing dispatcher behavior)
/// - `Circle`: returns `n` points uniformly spaced on full circle (CCW from +X, no duplicate)
/// - `Arc`: returns `n` points from `start_angle` to `end_angle` (CCW, endpoint excluded for chaining)
///
/// # Errors
/// - `DegenerateSketchElement` if `radius <= LENGTH_TOLERANCE` or `|end_angle - start_angle| < ANGLE_TOLERANCE`
pub fn tessellate_sketch_element(
    elem: &SketchElement,
    base_segments: usize,
) -> Result<Vec<[f64; 2]>, KernelError> {
    match elem {
        SketchElement::Line { from, .. } => Ok(vec![*from]),
        SketchElement::Circle { id, center, radius } => {
            // F03: center NaN/Inf 検証
            if !center[0].is_finite() || !center[1].is_finite() {
                return Err(KernelError::DegenerateSketchElement {
                    element_id: id.clone(),
                    reason: "non-finite center",
                });
            }
            // F22: radius NaN/Inf 検証
            if !radius.is_finite() {
                return Err(KernelError::DegenerateSketchElement {
                    element_id: id.clone(),
                    reason: "non-finite radius",
                });
            }
            if *radius <= LENGTH_TOLERANCE {
                return Err(KernelError::DegenerateSketchElement {
                    element_id: id.clone(),
                    reason: "radius <= ε_radius",
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
            // F03: center NaN/Inf 検証
            if !center[0].is_finite() || !center[1].is_finite() {
                return Err(KernelError::DegenerateSketchElement {
                    element_id: id.clone(),
                    reason: "non-finite center",
                });
            }
            // F22: radius/angle NaN/Inf 検証
            if !radius.is_finite() || !start_angle.is_finite() || !end_angle.is_finite() {
                return Err(KernelError::DegenerateSketchElement {
                    element_id: id.clone(),
                    reason: "non-finite radius/angle",
                });
            }
            if *radius <= LENGTH_TOLERANCE {
                return Err(KernelError::DegenerateSketchElement {
                    element_id: id.clone(),
                    reason: "radius <= ε_radius",
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
        SketchElement::Ellipse {
            id,
            center,
            major,
            minor,
            rotation,
        } => {
            // 退化検査 - F03: center NaN/Inf 検証
            if !center[0].is_finite() || !center[1].is_finite() {
                return Err(KernelError::DegenerateSketchElement {
                    element_id: id.clone(),
                    reason: "non-finite center",
                });
            }
            if !major.is_finite() || !minor.is_finite() || !rotation.is_finite() {
                return Err(KernelError::DegenerateSketchElement {
                    element_id: id.clone(),
                    reason: "non-finite rotation",
                });
            }
            if *major <= LENGTH_TOLERANCE {
                return Err(KernelError::DegenerateSketchElement {
                    element_id: id.clone(),
                    reason: "major <= ε_radius",
                });
            }
            if *minor <= LENGTH_TOLERANCE {
                return Err(KernelError::DegenerateSketchElement {
                    element_id: id.clone(),
                    reason: "minor <= ε_radius",
                });
            }
            // F04: major >= minor invariant enforcement (ADR-017 §1)
            if *minor > *major {
                return Err(KernelError::DegenerateSketchElement {
                    element_id: id.clone(),
                    reason: "minor > major (axis order invariant)",
                });
            }
            if *minor / *major < EPS_AXIS_RATIO {
                return Err(KernelError::DegenerateSketchElement {
                    element_id: id.clone(),
                    reason: "minor/major < ε_axis_ratio",
                });
            }
            // パラメトリック楕円: (major·cos t, minor·sin t) を回転変換
            let cos_r = rotation.cos();
            let sin_r = rotation.sin();
            let n = base_segments.max(1); // F21: 下限保証
            Ok((0..n)
                .map(|i| {
                    let t = (i as f64) * std::f64::consts::TAU / (n as f64);
                    let local_x = major * t.cos();
                    let local_y = minor * t.sin();
                    // 回転変換: [cos_r -sin_r; sin_r cos_r] · [local_x; local_y]
                    let rot_x = cos_r * local_x - sin_r * local_y;
                    let rot_y = sin_r * local_x + cos_r * local_y;
                    [center[0] + rot_x, center[1] + rot_y]
                })
                .collect())
        }
        SketchElement::Conic { id, coeffs } => {
            // 係数の有限性チェック
            if !coeffs.iter().all(|&c| c.is_finite()) {
                return Err(KernelError::DegenerateSketchElement {
                    element_id: id.clone(),
                    reason: "non-finite coeff",
                });
            }
            // 判別式: B² - 4 A C (A=coeffs[0], B=coeffs[1], C=coeffs[2])
            let a = coeffs[0];
            let b = coeffs[1];
            let c = coeffs[2];
            let discriminant = b * b - 4.0 * a * c;
            if discriminant.abs() < EPS_DISCRIMINANT {
                return Err(KernelError::DegenerateSketchElement {
                    element_id: id.clone(),
                    reason: "|discriminant| < ε_discriminant",
                });
            }

            // === 主軸変換 (canonical form 化) ===
            // M = [[A, B/2], [B/2, C]] の固有値 λ1, λ2
            // 閉解: λ = (A+C)/2 ± sqrt(((A-C)/2)² + (B/2)²)
            let trace = a + c;
            let det = a * c - b * b / 4.0;
            let sqrt_term = ((a - c) / 2.0).powi(2) + (b / 2.0).powi(2);
            let sqrt_val = sqrt_term.sqrt();
            let lambda1 = trace / 2.0 + sqrt_val;
            let lambda2 = trace / 2.0 - sqrt_val;

            // 平行移動 t = -M⁻¹ · [D/2, E/2]ᵀ (M が可逆な場合のみ)
            // M⁻¹ = 1/det * [[C, -B/2], [-B/2, A]]
            let d = coeffs[3];
            let e = coeffs[4];
            let t_x = -(c * d / 2.0 - b * e / 4.0) / det;
            let t_y = -(-b * d / 4.0 + a * e / 2.0) / det;

            // 定数項 F' = F + (D/2, E/2) · t = -1 + (D/2, E/2) · t
            let fp = -1.0 + d / 2.0 * t_x + e / 2.0 * t_y;

            // F' が小さい（中心が conic 上に近い）→ 退化 1 点
            if fp.abs() < LENGTH_TOLERANCE {
                return Err(KernelError::DegenerateSketchElement {
                    element_id: id.clone(),
                    reason: "degenerate point conic",
                });
            }

            // === 種別ごとの tessellation ===
            let n = base_segments.max(1) * 2; // F21: 下限保証後 × 2
            if discriminant < 0.0 {
                // F02: lambda1, lambda2, fp の有限性検証
                if !lambda1.is_finite() || !lambda2.is_finite() || !fp.is_finite() {
                    return Err(KernelError::DegenerateSketchElement {
                        element_id: id.clone(),
                        reason: "non-finite canonical form",
                    });
                }
                // 楕円 (lambda1, lambda2 同符号、fp と逆符号)
                // a = sqrt(-fp / lambda1), b = sqrt(-fp / lambda2)
                // 実点チェック: -fp / lambda1, -fp / lambda2 が両方 > 0 である必要がある
                let ratio1 = -fp / lambda1;
                let ratio2 = -fp / lambda2;
                if ratio1 <= 0.0 || ratio2 <= 0.0 {
                    return Err(KernelError::DegenerateSketchElement {
                        element_id: id.clone(),
                        reason: "non-real conic (no real points)",
                    });
                }
                let semi_a = ratio1.sqrt();
                let semi_b = ratio2.sqrt();
                // パラメトリック: (a cos t, b sin t)
                // 回転: lambda1/lamda2 の固有ベクトルで
                //   e1 方向は (B/2, lambda1 - A) または (lambda1 - C, B/2)
                let (e1_x, e1_y) = if b.abs() > LENGTH_TOLERANCE {
                    let v_x = b / 2.0;
                    let v_y = lambda1 - a;
                    let norm = (v_x * v_x + v_y * v_y).sqrt();
                    (v_x / norm, v_y / norm)
                } else if (lambda1 - a).abs() < LENGTH_TOLERANCE {
                    // B ≈ 0 → 軸方向は (1,0)
                    (1.0, 0.0)
                } else {
                    (0.0, 1.0)
                };
                // e1 に直交する e2
                let e2_x = -e1_y;
                let e2_y = e1_x;

                Ok((0..n)
                    .map(|i| {
                        let t = (i as f64) * std::f64::consts::TAU / (n as f64);
                        let local_x = semi_a * t.cos();
                        let local_y = semi_b * t.sin();
                        // 主軸座標 → world 座標: (local_x * e1 + local_y * e2) + t
                        let wx = local_x * e1_x + local_y * e2_x + t_x;
                        let wy = local_x * e1_y + local_y * e2_y + t_y;
                        [wx, wy]
                    })
                    .collect())
            } else {
                // F02: lambda1, lambda2, fp の有限性検証
                if !lambda1.is_finite() || !lambda2.is_finite() || !fp.is_finite() {
                    return Err(KernelError::DegenerateSketchElement {
                        element_id: id.clone(),
                        reason: "non-finite canonical form",
                    });
                }
                // F12: 双曲線 (discriminant > 0): 実軸の方向は -fp/lambda_i の符号で決まる
                // canonical form: lambda1*X² + lambda2*Y² + fp = 0
                // → 実軸 = -fp/lambda_i > 0 となる i (cosh を載せる方向)
                let r1 = -fp / lambda1; // X² 係数
                let r2 = -fp / lambda2; // Y² 係数
                                        // hyperbola なら必ず r1, r2 異符号
                let (semi_a_sq, semi_b_sq, swap_axes) = if r1 > 0.0 && r2 < 0.0 {
                    (r1, -r2, false) // X 軸に cosh、Y 軸に sinh
                } else if r1 < 0.0 && r2 > 0.0 {
                    (r2, -r1, true) // Y 軸に cosh、X 軸に sinh
                } else {
                    return Err(KernelError::DegenerateSketchElement {
                        element_id: id.clone(),
                        reason: "non-real hyperbola (no real points)",
                    });
                };
                if !semi_a_sq.is_finite() || !semi_b_sq.is_finite() {
                    return Err(KernelError::DegenerateSketchElement {
                        element_id: id.clone(),
                        reason: "non-real hyperbola (no real points)",
                    });
                }
                let semi_a = semi_a_sq.sqrt();
                let semi_b = semi_b_sq.sqrt();
                // パラメトリック: (a cosh t, b sinh t), t ∈ [-2, 2] (AABB clip 相当)
                let t_min = -2.0;
                let t_max = 2.0;

                // 主軸方向 (楕円と同じロジック)
                let (e1_x, e1_y) = if b.abs() > LENGTH_TOLERANCE {
                    let v_x = b / 2.0;
                    let v_y = lambda1 - a;
                    let norm = (v_x * v_x + v_y * v_y).sqrt();
                    (v_x / norm, v_y / norm)
                } else if (lambda1 - a).abs() < LENGTH_TOLERANCE {
                    (1.0, 0.0)
                } else {
                    (0.0, 1.0)
                };
                let e2_x = -e1_y;
                let e2_y = e1_x;

                Ok((0..n)
                    .map(|i| {
                        let param = t_min + (i as f64) * (t_max - t_min) / ((n - 1) as f64);
                        let (local_x, local_y) = if !swap_axes {
                            (semi_a * param.cosh(), semi_b * param.sinh())
                        } else {
                            (semi_b * param.sinh(), semi_a * param.cosh())
                        };
                        let wx = local_x * e1_x + local_y * e2_x + t_x;
                        let wy = local_x * e1_y + local_y * e2_y + t_y;
                        [wx, wy]
                    })
                    .collect())
            }
        }
    }
}

/// Returns true if the sketch element is an open curve (cannot form a closed profile).
///
/// - Conic hyperbola (discriminant > 0) → true (open, single branch)
/// - All other elements → false (closed or can form closed profile)
///
/// Used by engawa-build to validate extrusion profiles.
pub fn sketch_element_is_open(elem: &SketchElement) -> bool {
    match elem {
        SketchElement::Conic { coeffs, .. } => {
            // 判別式: B² - 4 A C
            let a = coeffs[0];
            let b = coeffs[1];
            let c = coeffs[2];
            let discriminant = b * b - 4.0 * a * c;
            // hyperbola (discriminant > 0) は開曲線
            discriminant > 0.0
        }
        _ => false,
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
                reason: "radius <= ε_radius"
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

    /// T_EDGE_nan_radius: F22 — NaN radius は DegenerateSketchElement で reject。
    #[test]
    fn t_edge_nan_radius_no_panic() {
        let circle = SketchElement::Circle {
            id: "nan_circle".to_string(),
            center: [0.0, 0.0],
            radius: f64::NAN,
        };
        let err = tessellate_sketch_element(&circle, 32).unwrap_err();
        assert!(
            matches!(err, KernelError::DegenerateSketchElement { reason, .. } if reason.contains("non-finite"))
        );
    }

    /// T_EDGE_inf_radius: F22 — Inf radius は DegenerateSketchElement で reject。
    #[test]
    fn t_edge_inf_radius_succeeds() {
        let circle = SketchElement::Circle {
            id: "inf_circle".to_string(),
            center: [0.0, 0.0],
            radius: f64::INFINITY,
        };
        let err = tessellate_sketch_element(&circle, 32).unwrap_err();
        assert!(matches!(err, KernelError::DegenerateSketchElement { .. }));
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
                reason: "radius <= ε_radius"
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
                reason: "radius <= ε_radius"
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
                reason: "radius <= ε_radius"
            } if element_id == "tiny_circle"
        ));
    }

    /// T_EDGE_length_tolerance_boundary: radius = LENGTH_TOLERANCE rejected (`<=` convention, ADR-018).
    #[test]
    fn t_edge_length_tolerance_boundary_passes() {
        let circle = SketchElement::Circle {
            id: "eps_circle".to_string(),
            center: [0.0, 0.0],
            radius: LENGTH_TOLERANCE,
        };
        let mesh = tessellate_sketch_element(&circle, 32);
        assert!(matches!(
            mesh,
            Err(KernelError::DegenerateSketchElement { .. })
        ));
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

    // === Ellipse & Conic tests (Issue #274) ===

    /// T01_ellipse_determinism: Same ellipse produces identical polyline.
    #[test]
    fn t01_ellipse_determinism() {
        let ellipse = SketchElement::Ellipse {
            id: "e1".to_string(),
            center: [0.0, 0.0],
            major: 2.0,
            minor: 1.0,
            rotation: 0.0,
        };
        let pts1 = tessellate_sketch_element(&ellipse, 32).unwrap();
        let pts2 = tessellate_sketch_element(&ellipse, 32).unwrap();
        assert_eq!(pts1, pts2);
        assert_eq!(pts1.len(), 32);
    }

    /// T01b_conic_determinism: Same conic produces identical polyline.
    #[test]
    fn t01b_conic_determinism() {
        // x² + 4y² = 1 (楕円)
        let conic = SketchElement::Conic {
            id: "c1".to_string(),
            coeffs: [1.0, 0.0, 4.0, 0.0, 0.0],
        };
        let pts1 = tessellate_sketch_element(&conic, 32).unwrap();
        let pts2 = tessellate_sketch_element(&conic, 32).unwrap();
        assert_eq!(pts1, pts2);
    }

    /// T02_normal_ellipse_axis_aligned: Axis-aligned ellipse tessellation.
    #[test]
    fn t02_normal_ellipse_axis_aligned() {
        let ellipse = SketchElement::Ellipse {
            id: "e1".to_string(),
            center: [0.0, 0.0],
            major: 2.0,
            minor: 1.0,
            rotation: 0.0,
        };
        let pts = tessellate_sketch_element(&ellipse, 32).unwrap();
        assert_eq!(pts.len(), 32);
        // 頂点チェック: (2,0), (0,1), (-2,0), (0,-1) 付近に点がある
        let has_right = pts
            .iter()
            .any(|&p| (p[0] - 2.0).abs() < LENGTH_TOLERANCE && p[1].abs() < LENGTH_TOLERANCE);
        let has_top = pts
            .iter()
            .any(|&p| p[0].abs() < LENGTH_TOLERANCE && (p[1] - 1.0).abs() < LENGTH_TOLERANCE);
        let has_left = pts
            .iter()
            .any(|&p| (p[0] + 2.0).abs() < LENGTH_TOLERANCE && p[1].abs() < LENGTH_TOLERANCE);
        let has_bottom = pts
            .iter()
            .any(|&p| p[0].abs() < LENGTH_TOLERANCE && (p[1] + 1.0).abs() < LENGTH_TOLERANCE);
        assert!(has_right, "missing right vertex (2,0)");
        assert!(has_top, "missing top vertex (0,1)");
        assert!(has_left, "missing left vertex (-2,0)");
        assert!(has_bottom, "missing bottom vertex (0,-1)");
    }

    /// T02b_normal_ellipse_rotated: Rotated ellipse tessellation.
    #[test]
    fn t02b_normal_ellipse_rotated() {
        let ellipse = SketchElement::Ellipse {
            id: "e1".to_string(),
            center: [0.0, 0.0],
            major: 2.0,
            minor: 1.0,
            rotation: std::f64::consts::PI / 4.0,
        };
        let pts = tessellate_sketch_element(&ellipse, 32).unwrap();
        assert_eq!(pts.len(), 32);
        // π/4 回転後の主軸方向: (√2, √2) 付近に頂点
        let sqrt2 = 2.0_f64.sqrt();
        let has_vertex = pts
            .iter()
            .any(|&p| (p[0] - sqrt2).abs() < 1e-6 && (p[1] - sqrt2).abs() < 1e-6);
        assert!(has_vertex, "missing rotated vertex");
    }

    /// T03_normal_conic_ellipse: Ellipse conic tessellation.
    #[test]
    fn t03_normal_conic_ellipse() {
        // x² + 4y² = 1 (F=-1 normalize 済)
        let conic = SketchElement::Conic {
            id: "c1".to_string(),
            coeffs: [1.0, 0.0, 4.0, 0.0, 0.0],
        };
        let pts = tessellate_sketch_element(&conic, 32).unwrap();
        // base_segments × 2 = 64 点
        assert_eq!(pts.len(), 64);
        // 判別式チェック: B² - 4AC = 0 - 16 = -16 < 0 (楕円)
        let a = 1.0;
        let b = 0.0;
        let c = 4.0;
        let discriminant = b * b - 4.0 * a * c;
        assert!(
            discriminant < 0.0,
            "expected ellipse (negative discriminant)"
        );
        // 全点で conic equation を満たす（誤差許容 100×LENGTH_TOLERANCE）
        for &p in &pts {
            let x = p[0];
            let y = p[1];
            let val = a * x * x + b * x * y + c * y * y - 1.0;
            assert!(
                val.abs() < LENGTH_TOLERANCE * 100.0,
                "point off conic: {p:?}, val={val}"
            );
        }
    }

    /// T03b_normal_conic_hyperbola: Hyperbola conic tessellation.
    #[test]
    fn t03b_normal_conic_hyperbola() {
        // x² - y² = 1 (双曲線)
        let conic = SketchElement::Conic {
            id: "c1".to_string(),
            coeffs: [1.0, 0.0, -1.0, 0.0, 0.0],
        };
        let pts = tessellate_sketch_element(&conic, 32).unwrap();
        assert_eq!(pts.len(), 64);
        // 判別式: B² - 4AC = 0 - 4×1×(-1) = 4 > 0 (双曲線)
        let a = 1.0;
        let b = 0.0;
        let c = -1.0;
        let discriminant = b * b - 4.0 * a * c;
        assert!(
            discriminant > 0.0,
            "expected hyperbola (positive discriminant)"
        );
        // 全点で conic equation を満たす
        for &p in &pts {
            let x = p[0];
            let y = p[1];
            let val = a * x * x + b * x * y + c * y * y - 1.0;
            assert!(
                val.abs() < LENGTH_TOLERANCE * 100.0,
                "point off conic: {p:?}, val={val}"
            );
        }
        // 1 分枝のみ (全点で x > 0)
        assert!(
            pts.iter().all(|p| p[0] > 0.0),
            "expected single branch (x > 0)"
        );
    }

    /// T_DEG_zero_axis_major: Zero major axis fails.
    #[test]
    fn t_degenerate_zero_axis_major() {
        let ellipse = SketchElement::Ellipse {
            id: "e0".to_string(),
            center: [0.0, 0.0],
            major: 0.0,
            minor: 1.0,
            rotation: 0.0,
        };
        let err = tessellate_sketch_element(&ellipse, 32).unwrap_err();
        assert!(matches!(
            err,
            KernelError::DegenerateSketchElement {
                element_id,
                reason: "major <= ε_radius"
            } if element_id == "e0"
        ));
    }

    /// T_DEG_zero_axis_minor: Zero minor axis fails.
    #[test]
    fn t_degenerate_zero_axis_minor() {
        let ellipse = SketchElement::Ellipse {
            id: "e0".to_string(),
            center: [0.0, 0.0],
            major: 1.0,
            minor: 0.0,
            rotation: 0.0,
        };
        let err = tessellate_sketch_element(&ellipse, 32).unwrap_err();
        assert!(matches!(
            err,
            KernelError::DegenerateSketchElement {
                element_id,
                reason: "minor <= ε_radius"
            } if element_id == "e0"
        ));
    }

    /// T_DEG_circle_degeneracy: major == minor succeeds (circle case).
    #[test]
    fn t_boundary_circle_degeneracy() {
        let ellipse = SketchElement::Ellipse {
            id: "e_circle".to_string(),
            center: [0.0, 0.0],
            major: 1.0,
            minor: 1.0,
            rotation: 0.0,
        };
        let pts = tessellate_sketch_element(&ellipse, 32).unwrap();
        assert_eq!(pts.len(), 32);
        // 円と一致する頂点を確認
        let has_right = pts
            .iter()
            .any(|&p| (p[0] - 1.0).abs() < LENGTH_TOLERANCE && p[1].abs() < LENGTH_TOLERANCE);
        let has_top = pts
            .iter()
            .any(|&p| p[0].abs() < LENGTH_TOLERANCE && (p[1] - 1.0).abs() < LENGTH_TOLERANCE);
        assert!(has_right, "missing right vertex");
        assert!(has_top, "missing top vertex");
    }

    /// T_DEG_axis_ratio: minor/major < EPS_AXIS_RATIO fails.
    #[test]
    fn t_degenerate_axis_ratio() {
        let ellipse = SketchElement::Ellipse {
            id: "e_flat".to_string(),
            center: [0.0, 0.0],
            major: 1.0,
            minor: 1e-7, // < EPS_AXIS_RATIO * major
            rotation: 0.0,
        };
        let err = tessellate_sketch_element(&ellipse, 32).unwrap_err();
        assert!(matches!(
            err,
            KernelError::DegenerateSketchElement {
                element_id,
                reason: "minor/major < ε_axis_ratio"
            } if element_id == "e_flat"
        ));
    }

    /// T_DEG_conic_degenerate: Degenerate conic (line pair) fails.
    #[test]
    fn t_degenerate_conic_degenerate() {
        // x² = 1 → 直線対 x = ±1
        let conic = SketchElement::Conic {
            id: "c0".to_string(),
            coeffs: [1.0, 0.0, 0.0, 0.0, 0.0],
        };
        let err = tessellate_sketch_element(&conic, 32).unwrap_err();
        assert!(matches!(
            err,
            KernelError::DegenerateSketchElement {
                element_id,
                reason: "|discriminant| < ε_discriminant"
            } if element_id == "c0"
        ));
    }

    /// T_DEG_conic_nan: NaN coeff fails.
    #[test]
    fn t_degenerate_conic_nan() {
        let conic = SketchElement::Conic {
            id: "c_nan".to_string(),
            coeffs: [f64::NAN, 0.0, 1.0, 0.0, 0.0],
        };
        let err = tessellate_sketch_element(&conic, 32).unwrap_err();
        assert!(matches!(
            err,
            KernelError::DegenerateSketchElement {
                element_id,
                reason: "non-finite coeff"
            } if element_id == "c_nan"
        ));
    }

    /// T_DEG_ellipse_nan_rotation: NaN rotation fails.
    #[test]
    fn t_degenerate_ellipse_nan_rotation() {
        let ellipse = SketchElement::Ellipse {
            id: "e_nan".to_string(),
            center: [0.0, 0.0],
            major: 1.0,
            minor: 0.5,
            rotation: f64::NAN,
        };
        let err = tessellate_sketch_element(&ellipse, 32).unwrap_err();
        assert!(matches!(
            err,
            KernelError::DegenerateSketchElement {
                element_id,
                reason: "non-finite rotation"
            } if element_id == "e_nan"
        ));
    }

    /// F21: base_segments=0 でも ellipse は最低 1 点生成。
    #[test]
    fn t_boundary_ellipse_zero_segments() {
        let ellipse = SketchElement::Ellipse {
            id: "e0".to_string(),
            center: [0.0, 0.0],
            major: 2.0,
            minor: 1.0,
            rotation: 0.0,
        };
        let pts = tessellate_sketch_element(&ellipse, 0).unwrap();
        assert!(pts.len() >= 1);
    }

    /// F21: base_segments=0 でも conic は最低 2 点生成 (base_segments.max(1) * 2)。
    #[test]
    fn t_boundary_conic_zero_segments() {
        let conic = SketchElement::Conic {
            id: "c0".to_string(),
            coeffs: [1.0, 0.0, 4.0, 0.0, 0.0],
        };
        let pts = tessellate_sketch_element(&conic, 0).unwrap();
        assert!(pts.len() >= 2);
    }

    // === ADR-018 境界テスト: `<=` inclusive 規約 ===

    /// T_BOUNDARY_above_tolerance_circle: radius slightly above LENGTH_TOLERANCE passes.
    #[test]
    fn t_boundary_above_tolerance_circle() {
        let circle = SketchElement::Circle {
            id: "c1".to_string(),
            center: [0.0, 0.0],
            radius: LENGTH_TOLERANCE * 1.000_001,
        };
        let pts = tessellate_sketch_element(&circle, 32).unwrap();
        assert_eq!(pts.len(), 32);
    }

    /// T_BOUNDARY_above_tolerance_arc: radius slightly above LENGTH_TOLERANCE passes.
    #[test]
    fn t_boundary_above_tolerance_arc() {
        let arc = SketchElement::Arc {
            id: "a1".to_string(),
            center: [0.0, 0.0],
            radius: LENGTH_TOLERANCE * 1.000_001,
            start_angle: 0.0,
            end_angle: std::f64::consts::PI / 2.0,
        };
        let pts = tessellate_sketch_element(&arc, 32).unwrap();
        assert_eq!(pts.len(), 8);
    }

    /// T_BOUNDARY_above_tolerance_ellipse_major: major = LENGTH_TOLERANCE * 1.000_001 passes.
    /// 偽陽性ガード: `<` strict に戻すとテストが fail する。
    #[test]
    fn t_boundary_above_tolerance_ellipse_major() {
        let major = LENGTH_TOLERANCE * 1.000_001;
        let ellipse = SketchElement::Ellipse {
            id: "e1".to_string(),
            center: [0.0, 0.0],
            major,
            minor: LENGTH_TOLERANCE * 1.000_001,
            rotation: 0.0,
        };
        let pts = tessellate_sketch_element(&ellipse, 32).unwrap();
        assert_eq!(pts.len(), 32);
    }

    /// T_BOUNDARY_above_tolerance_ellipse_minor: minor = LENGTH_TOLERANCE * 1.000_001 passes.
    /// 偽陽性ガード: `<` strict に戻すとテストが fail する。
    #[test]
    fn t_boundary_above_tolerance_ellipse_minor() {
        let major = LENGTH_TOLERANCE * 1.000_001;
        let ellipse = SketchElement::Ellipse {
            id: "e1".to_string(),
            center: [0.0, 0.0],
            major,
            minor: LENGTH_TOLERANCE * 1.000_001,
            rotation: 0.0,
        };
        let pts = tessellate_sketch_element(&ellipse, 32).unwrap();
        assert_eq!(pts.len(), 32);
    }

    /// T_BOUNDARY_exact_tolerance_circle: radius == LENGTH_TOLERANCE rejected (`<=` convention).
    #[test]
    fn t_boundary_exact_tolerance_circle() {
        let circle = SketchElement::Circle {
            id: "c0".to_string(),
            center: [0.0, 0.0],
            radius: LENGTH_TOLERANCE,
        };
        let err = tessellate_sketch_element(&circle, 32).unwrap_err();
        assert!(matches!(
            err,
            KernelError::DegenerateSketchElement {
                reason: "radius <= ε_radius",
                ..
            }
        ));
    }

    /// T_BOUNDARY_exact_tolerance_arc: radius == LENGTH_TOLERANCE rejected (`<=` convention).
    #[test]
    fn t_boundary_exact_tolerance_arc() {
        let arc = SketchElement::Arc {
            id: "a0".to_string(),
            center: [0.0, 0.0],
            radius: LENGTH_TOLERANCE,
            start_angle: 0.0,
            end_angle: std::f64::consts::PI / 2.0,
        };
        let err = tessellate_sketch_element(&arc, 32).unwrap_err();
        assert!(matches!(
            err,
            KernelError::DegenerateSketchElement {
                reason: "radius <= ε_radius",
                ..
            }
        ));
    }

    /// T_BOUNDARY_exact_tolerance_ellipse_major: major == LENGTH_TOLERANCE rejected (`<=` convention).
    /// 偽陽性ガード: `<` strict に戻すとテストが fail する（reason "major <= ε_radius" 一致）。
    #[test]
    fn t_boundary_exact_tolerance_ellipse_major() {
        let ellipse = SketchElement::Ellipse {
            id: "e0".to_string(),
            center: [0.0, 0.0],
            major: LENGTH_TOLERANCE,
            minor: LENGTH_TOLERANCE,
            rotation: 0.0,
        };
        let err = tessellate_sketch_element(&ellipse, 32).unwrap_err();
        assert!(matches!(
            err,
            KernelError::DegenerateSketchElement {
                reason: "major <= ε_radius",
                ..
            }
        ));
    }

    /// T_BOUNDARY_exact_tolerance_ellipse_minor: minor == LENGTH_TOLERANCE rejected (`<=` convention).
    /// 偽陽性ガード: `<` strict に戻すとテストが fail する（reason "minor <= ε_radius" 一致）。
    #[test]
    fn t_boundary_exact_tolerance_ellipse_minor() {
        let ellipse = SketchElement::Ellipse {
            id: "e0".to_string(),
            center: [0.0, 0.0],
            major: LENGTH_TOLERANCE * 1.000_001,
            minor: LENGTH_TOLERANCE,
            rotation: 0.0,
        };
        let err = tessellate_sketch_element(&ellipse, 32).unwrap_err();
        assert!(matches!(
            err,
            KernelError::DegenerateSketchElement {
                reason: "minor <= ε_radius",
                ..
            }
        ));
    }
}
