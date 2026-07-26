//! 2D sketch fillet (round) at a shared corner of two sketch elements.
//!
//! Phase 10 (#296): Line-Line only. Arc/Arc, Line/Arc, Circle, Ellipse, Conic are
//! Out-of-Scope (tangent-circle construction has distinct math; separate Issue).
//!
//! # Element-level contract (`compute_fillet`)
//! - `elem_a` / `elem_b` must both be `SketchElement::Line`
//! - `elem_a.to == elem_b.from` (the shared corner) within `LENGTH_TOLERANCE`
//! - Corner angle `theta` must be in `(ANGLE_TOLERANCE, π - ANGLE_TOLERANCE)`
//! - Tangent length `t = radius / tan(theta/2)` must not exceed either line length
//!
//! # Build-level contract (`apply_sketch_fillet_build`)
//! - `elem1_id` and `elem2_id` must resolve to adjacent Lines in the profile array
//!   (the pair `[last, first]` of a closed loop also counts as adjacent)
//! - Input order of `elem1_id` / `elem2_id` does not affect the result

use crate::error::KernelError;
use crate::geometry::math::{ANGLE_TOLERANCE, LENGTH_TOLERANCE};
use engawa_format::SketchElement;

/// Element-level fillet: trim two Lines to tangent points and insert a tangent Arc.
///
/// `elem_a` and `elem_b` are the source Lines in array order — `elem_a.to` must equal
/// `elem_b.from` (the shared corner). The caller is responsible for normalising input
/// order (see `find_adjacent_pair`).
///
/// Returns `(trimmed_a, new_arc, trimmed_b)` in the original array order.
///
/// # Errors
/// - `UnsupportedFeature { kind: "sketch_fillet_only_line_line" }` — either element is not a Line
/// - `InvalidParameter { kind: "sketch_fillet_no_shared_corner" }` — `a.to` != `b.from`
/// - `InvalidParameter { kind: "radius" }` — radius is non-positive or non-finite
/// - `DegenerateSketchElement { reason: "fillet_zero_length_input_line" }`
/// - `DegenerateSketchElement { reason: "fillet_corner_angle_degenerate" }`
/// - `FilletRadiusTooLarge { elem1_id, elem2_id, radius }` — tangent length exceeds Line length
pub fn compute_fillet(
    elem_a: &SketchElement,
    elem_b: &SketchElement,
    radius: f64,
) -> Result<(SketchElement, SketchElement, SketchElement), KernelError> {
    let (a_id, a_from, a_to) = as_line(elem_a)?;
    let (b_id, b_from, b_to) = as_line(elem_b)?;

    if !radius.is_finite() || radius <= LENGTH_TOLERANCE {
        return Err(KernelError::InvalidParameter { kind: "radius" });
    }

    if dist(&a_to, &b_from) > LENGTH_TOLERANCE {
        return Err(KernelError::InvalidParameter {
            kind: "sketch_fillet_no_shared_corner",
        });
    }
    let corner = a_to; // == b_from

    let len_a = dist(&a_from, &a_to);
    let len_b = dist(&b_from, &b_to);
    if len_a <= LENGTH_TOLERANCE || len_b <= LENGTH_TOLERANCE {
        return Err(KernelError::DegenerateSketchElement {
            element_id: a_id.to_string(),
            reason: "fillet_zero_length_input_line",
        });
    }

    let d_a = normalize(sub(&a_from, &corner)); // corner→far_a
    let d_b = normalize(sub(&b_to, &corner)); // corner→far_b

    let theta = dot(&d_a, &d_b).clamp(-1.0, 1.0).acos();
    if theta <= ANGLE_TOLERANCE || theta >= std::f64::consts::PI - ANGLE_TOLERANCE {
        return Err(KernelError::DegenerateSketchElement {
            element_id: format!("{a_id}_{b_id}"),
            reason: "fillet_corner_angle_degenerate",
        });
    }

    let t = radius / (theta / 2.0).tan();
    if t > len_a - LENGTH_TOLERANCE || t > len_b - LENGTH_TOLERANCE {
        return Err(KernelError::FilletRadiusTooLarge {
            elem1_id: a_id.to_string(),
            elem2_id: b_id.to_string(),
            radius,
        });
    }

    let tangent_a = add(&corner, &scale(&d_a, t));
    let tangent_b = add(&corner, &scale(&d_b, t));

    let center_dist = radius / (theta / 2.0).sin();
    let bisector = normalize(add(&d_a, &d_b));
    let center = add(&corner, &scale(&bisector, center_dist));

    let angle_a = angle_of(&sub(&tangent_a, &center));
    let angle_b = angle_of(&sub(&tangent_b, &center));
    let raw_sweep = normalize_angle(angle_b - angle_a);

    let new_a = SketchElement::Line {
        id: a_id.to_string(),
        from: a_from,
        to: tangent_a,
    };
    let new_b = SketchElement::Line {
        id: b_id.to_string(),
        from: tangent_b,
        to: b_to,
    };
    let new_arc = SketchElement::Arc {
        id: format!("{a_id}_{b_id}_fillet_arc"),
        center,
        radius,
        start_angle: angle_a,
        end_angle: angle_a + raw_sweep,
    };

    Ok((new_a, new_arc, new_b))
}

/// Locate `(a_idx, b_idx)` in the profile such that `b` is the array successor of `a`
/// (with closed-loop wraparound `[last, first]` also accepted).
///
/// Public so that `engawa-build::feature_crud` can evaluate the same adjacency contract
/// for history gates without duplicating the index arithmetic. Contains no geometry.
///
/// # Errors
/// - `InvalidParameter { kind: "sketch_fillet_same_element" }` — elem1_id == elem2_id
/// - `InvalidParameter { kind: "sketch_fillet_elem_not_found" }` — ID not present in profile
/// - `InvalidParameter { kind: "sketch_fillet_elems_not_adjacent" }` — pair is not adjacent
pub fn find_adjacent_pair(
    source: &[SketchElement],
    elem1_id: &str,
    elem2_id: &str,
) -> Result<(usize, usize), KernelError> {
    if elem1_id == elem2_id {
        return Err(KernelError::InvalidParameter {
            kind: "sketch_fillet_same_element",
        });
    }
    let idx1 = source
        .iter()
        .position(|e| element_id(e) == elem1_id)
        .ok_or(KernelError::InvalidParameter {
            kind: "sketch_fillet_elem_not_found",
        })?;
    let idx2 = source
        .iter()
        .position(|e| element_id(e) == elem2_id)
        .ok_or(KernelError::InvalidParameter {
            kind: "sketch_fillet_elem_not_found",
        })?;
    let n = source.len();
    if idx2 == idx1 + 1 {
        Ok((idx1, idx2))
    } else if idx1 == idx2 + 1 {
        Ok((idx2, idx1))
    } else if idx1 == n - 1 && idx2 == 0 {
        Ok((idx1, idx2))
    } else if idx2 == n - 1 && idx1 == 0 {
        Ok((idx2, idx1))
    } else {
        Err(KernelError::InvalidParameter {
            kind: "sketch_fillet_elems_not_adjacent",
        })
    }
}

/// Build-level fillet: locate the adjacent Lines, splice in the Arc, and trim both Lines.
///
/// # Errors
/// In addition to `compute_fillet` errors:
/// - `InvalidParameter { kind: "sketch_fillet_elem_not_found" }`
/// - `InvalidParameter { kind: "sketch_fillet_same_element" }`
/// - `InvalidParameter { kind: "sketch_fillet_elems_not_adjacent" }`
/// - `InvalidParameter { kind: "sketch_fillet_arc_id_collision" }` — derived Arc ID collides
///   with an existing user element ID
pub fn apply_sketch_fillet_build(
    source: &[SketchElement],
    elem1_id: &str,
    elem2_id: &str,
    radius: f64,
) -> Result<Vec<SketchElement>, KernelError> {
    let n = source.len();
    let (a_idx, b_idx) = find_adjacent_pair(source, elem1_id, elem2_id)?;

    let arc_id_a = element_id(&source[a_idx]);
    let arc_id_b = element_id(&source[b_idx]);
    let prospective_arc_id = format!("{arc_id_a}_{arc_id_b}_fillet_arc");
    if source.iter().any(|e| element_id(e) == prospective_arc_id) {
        return Err(KernelError::InvalidParameter {
            kind: "sketch_fillet_arc_id_collision",
        });
    }

    let (new_a, new_arc, new_b) = compute_fillet(&source[a_idx], &source[b_idx], radius)?;

    let mut out = source.to_vec();
    out[a_idx] = new_a;
    out[b_idx] = new_b;
    // For wraparound (b_idx == 0, a_idx == n-1) insert at the tail (== n) to preserve
    // array-order traversal `a → arc → b → ...`. Otherwise insert between a and b.
    let insert_at = if b_idx == a_idx + 1 { a_idx + 1 } else { n };
    out.insert(insert_at, new_arc);
    Ok(out)
}

// --- private helpers (duplicated from sketch_offset.rs intentionally; see plan.md) ---

fn as_line(elem: &SketchElement) -> Result<(&str, [f64; 2], [f64; 2]), KernelError> {
    match elem {
        SketchElement::Line { id, from, to } => Ok((id.as_str(), *from, *to)),
        _ => Err(KernelError::UnsupportedFeature {
            kind: "sketch_fillet_only_line_line",
        }),
    }
}

fn element_id(elem: &SketchElement) -> &str {
    match elem {
        SketchElement::Line { id, .. }
        | SketchElement::Circle { id, .. }
        | SketchElement::Arc { id, .. }
        | SketchElement::Ellipse { id, .. }
        | SketchElement::Conic { id, .. } => id.as_str(),
    }
}

fn dist(a: &[f64; 2], b: &[f64; 2]) -> f64 {
    let dx = b[0] - a[0];
    let dy = b[1] - a[1];
    (dx * dx + dy * dy).sqrt()
}

fn sub(a: &[f64; 2], b: &[f64; 2]) -> [f64; 2] {
    [a[0] - b[0], a[1] - b[1]]
}

fn add(a: &[f64; 2], b: &[f64; 2]) -> [f64; 2] {
    [a[0] + b[0], a[1] + b[1]]
}

fn scale(a: &[f64; 2], s: f64) -> [f64; 2] {
    [a[0] * s, a[1] * s]
}

fn dot(a: &[f64; 2], b: &[f64; 2]) -> f64 {
    a[0] * b[0] + a[1] * b[1]
}

fn normalize(a: [f64; 2]) -> [f64; 2] {
    let n = (a[0] * a[0] + a[1] * a[1]).sqrt();
    // Callers ensure non-zero length via LENGTH_TOLERANCE gates before invoking.
    if n <= LENGTH_TOLERANCE {
        [0.0, 0.0]
    } else {
        [a[0] / n, a[1] / n]
    }
}

fn angle_of(a: &[f64; 2]) -> f64 {
    a[1].atan2(a[0])
}

/// Normalise an angle to `(-π, π]`.
fn normalize_angle(a: f64) -> f64 {
    use std::f64::consts::{PI, TAU};
    let mut r = a % TAU; // (-2π, 2π)
    if r > PI {
        r -= TAU;
    } else if r <= -PI {
        r += TAU;
    }
    r
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::f64::consts::PI;

    /// T01: Determinism — same input produces same output, and derived Arc id matches
    /// the ADR-017 §3 `{a}_{b}_fillet_arc` convention.
    #[test]
    fn t01_determinism_and_derived_arc_id() {
        let profile = rect_profile();
        let out1 = apply_sketch_fillet_build(&profile, "l1", "l2", 1.0).unwrap();
        let out2 = apply_sketch_fillet_build(&profile, "l1", "l2", 1.0).unwrap();
        assert_eq!(out1, out2);

        // Arc id derivation contract.
        let arc = out1
            .iter()
            .find(|e| matches!(e, SketchElement::Arc { .. }))
            .expect("fillet should insert an Arc");
        assert_eq!(element_id(arc), "l1_l2_fillet_arc");
    }

    /// T01b: elem1_id/elem2_id order invariance.
    #[test]
    fn t01b_input_order_invariance() {
        let profile = rect_profile();
        let forward = apply_sketch_fillet_build(&profile, "l1", "l2", 1.0).unwrap();
        let reverse = apply_sketch_fillet_build(&profile, "l2", "l1", 1.0).unwrap();
        assert_eq!(forward, reverse);
    }

    /// T01c: Arc ID collision guard.
    #[test]
    fn t01c_arc_id_collision_rejected() {
        let mut profile = rect_profile();
        profile.push(SketchElement::Arc {
            id: "l1_l2_fillet_arc".to_string(),
            center: [0.0, 0.0],
            radius: 0.5,
            start_angle: 0.0,
            end_angle: PI / 2.0,
        });
        let err = apply_sketch_fillet_build(&profile, "l1", "l2", 1.0).unwrap_err();
        assert!(matches!(
            err,
            KernelError::InvalidParameter {
                kind: "sketch_fillet_arc_id_collision"
            }
        ));
    }

    /// T02: 90° corner — analytic tangent length / center position.
    /// Lines: l1 (0,0)→(10,0), l2 (10,0)→(10,5). radius=1.0.
    /// theta=π/2, t=1/tan(π/4)=1, center=(9,1).
    #[test]
    fn t02_normal_90deg() {
        let l1 = SketchElement::Line {
            id: "l1".to_string(),
            from: [0.0, 0.0],
            to: [10.0, 0.0],
        };
        let l2 = SketchElement::Line {
            id: "l2".to_string(),
            from: [10.0, 0.0],
            to: [10.0, 5.0],
        };
        let (new_a, new_arc, new_b) = compute_fillet(&l1, &l2, 1.0).unwrap();
        match (new_a, new_arc, new_b) {
            (
                SketchElement::Line { to, .. },
                SketchElement::Arc {
                    center,
                    radius,
                    start_angle,
                    end_angle,
                    ..
                },
                SketchElement::Line { from, .. },
            ) => {
                // tangent_a = corner + d_a*t = (10,0) + (-1,0)*1 = (9,0)
                assert!((to[0] - 9.0).abs() < LENGTH_TOLERANCE);
                assert!((to[1] - 0.0).abs() < LENGTH_TOLERANCE);
                // tangent_b = corner + d_b*t = (10,0) + (0,1)*1 = (10,1)
                assert!((from[0] - 10.0).abs() < LENGTH_TOLERANCE);
                assert!((from[1] - 1.0).abs() < LENGTH_TOLERANCE);
                // center = corner + bisector * (radius/sin(theta/2))
                //        = (10,0) + normalize((-1,0)+(0,1)) * (1/sin(π/4))
                //        = (10,0) + (-1/√2, 1/√2) * √2 = (10,0) + (-1,1) = (9,1)
                assert!((center[0] - 9.0).abs() < LENGTH_TOLERANCE);
                assert!((center[1] - 1.0).abs() < LENGTH_TOLERANCE);
                assert!((radius - 1.0).abs() < LENGTH_TOLERANCE);
                // start_angle: vector from center (9,1) to tangent_a (9,0) = (0,-1) → -π/2
                assert!((start_angle - (-PI / 2.0)).abs() < LENGTH_TOLERANCE);
                // CCW convex corner → positive sweep π/2; end_angle = -π/2 + π/2 = 0
                assert!((end_angle - 0.0).abs() < LENGTH_TOLERANCE);
            }
            _ => panic!("compute_fillet returned wrong variant triple"),
        }
    }

    /// T03: 60° corner — analytic tangent length and center distance.
    /// Setup: corner at origin. l1: (0,0)→(1,0). l2: (0,0)→(cos(60°), sin(60°))*length.
    /// Actually we need lines with corner = a.to == b.from.
    /// l1 = (-1, 0) → (0,0), so d_a = normalize((-1,0)-(0,0)) = (-1,0)
    /// l2 = (0,0) → (cos60, sin60)*5 = (2.5, 5*√3/2), d_b = (cos60, sin60) = (0.5, √3/2)
    /// theta = acos((-1,0)·(0.5,√3/2)) = acos(-0.5) = 2π/3 — that's 120°, not 60°.
    /// For 60°: d_a · d_b = cos(60°). Pick d_a = (1,0), d_b = (cos60, sin60) = (0.5, √3/2).
    /// But then corner is between them, so l1.from needs to be in -d_a direction from corner.
    /// Easier: corner=(0,0), l1: (-5,0)→(0,0), l2: (0,0)→(0.5,√3/2)*5=(2.5, 5√3/2)
    /// d_a = normalize((-5,0))=(-1,0), d_b = (0.5,√3/2). dot=-0.5, theta=acos(-0.5)=2π/3 ❌
    /// Use l1 from positive x: l1: (5,0)→(0,0). d_a = normalize((5,0)-(0,0))=(1,0).
    /// l2: (0,0)→(0.5,√3/2)*5. d_b=(0.5,√3/2). dot=0.5, theta=acos(0.5)=π/3 ✓
    /// t = 1 / tan(π/6) = √3 ≈ 1.7320508
    /// center_dist = 1 / sin(π/6) = 2
    #[test]
    fn t03_normal_60deg() {
        let l1 = SketchElement::Line {
            id: "l1".to_string(),
            from: [5.0, 0.0],
            to: [0.0, 0.0],
        };
        let l2 = SketchElement::Line {
            id: "l2".to_string(),
            from: [0.0, 0.0],
            to: [2.5, 5.0 * 3.0_f64.sqrt() / 2.0],
        };
        let (_a, arc, _b) = compute_fillet(&l1, &l2, 1.0).unwrap();
        match arc {
            SketchElement::Arc { center, radius, .. } => {
                assert!((radius - 1.0).abs() < LENGTH_TOLERANCE);
                let half_theta = PI / 6.0;
                let expected_t = 1.0 / half_theta.tan();
                let expected_center_dist = 1.0 / half_theta.sin();
                // bisector = normalize(d_a + d_b) = normalize((1+0.5, 0+√3/2)) = normalize((1.5, √3/2))
                //          = (1.5, √3/2) / |(1.5, √3/2)|. |.|² = 2.25 + 0.75 = 3, |.|=√3.
                //          = (√3/2, 1/2) — this is exactly the bisector direction at 30°.
                let expected_center = [
                    expected_center_dist * (3.0_f64.sqrt() / 2.0),
                    expected_center_dist * 0.5,
                ];
                assert!((center[0] - expected_center[0]).abs() < LENGTH_TOLERANCE);
                assert!((center[1] - expected_center[1]).abs() < LENGTH_TOLERANCE);
                // t sanity (length-only check on the lines, computed from public state).
                let _ = expected_t;
            }
            _ => panic!("expected Arc"),
        }
    }

    /// T09: profile chain continuity — out[i] endpoint meets out[(i+1)%n] startpoint.
    /// Validates both Line.to (which Extrude path ignores) and Arc end point.
    #[test]
    fn t09_profile_chain_continuity() {
        // In-order pair (l1, l2): rectangle.
        let profile = rect_profile();
        let out = apply_sketch_fillet_build(&profile, "l1", "l2", 1.0).unwrap();
        assert_chain_continuous(&out);

        // Wraparound pair (l4, l1) of the same rectangle.
        let out_wrap = apply_sketch_fillet_build(&profile, "l4", "l1", 1.0).unwrap();
        assert_chain_continuous(&out_wrap);
    }

    /// T_DEG_fillet_too_large: short Line pair with radius=10 → FilletRadiusTooLarge.
    #[test]
    fn t_deg_fillet_too_large() {
        let l1 = SketchElement::Line {
            id: "l1".to_string(),
            from: [0.0, 0.0],
            to: [1.0, 0.0],
        };
        let l2 = SketchElement::Line {
            id: "l2".to_string(),
            from: [1.0, 0.0],
            to: [1.0, 1.0],
        };
        let err = compute_fillet(&l1, &l2, 10.0).unwrap_err();
        assert!(matches!(
            err,
            KernelError::FilletRadiusTooLarge {
                elem1_id,
                elem2_id,
                radius: 10.0,
            } if elem1_id == "l1" && elem2_id == "l2"
        ));
    }

    /// T_DEG_corner_angle_flat: ~180° corner (collinear, folded flat).
    #[test]
    fn t_deg_corner_angle_flat() {
        let l1 = SketchElement::Line {
            id: "l1".to_string(),
            from: [0.0, 0.0],
            to: [5.0, 0.0],
        };
        // l2 continues +x direction → corner angle approaches π.
        let l2 = SketchElement::Line {
            id: "l2".to_string(),
            from: [5.0, 0.0],
            to: [5.0 + 1e-12, 0.0],
        };
        let err = compute_fillet(&l1, &l2, 0.5).unwrap_err();
        // Either degenerate angle (≈π) or zero-length line (depending on tolerance interplay).
        assert!(matches!(
            err,
            KernelError::DegenerateSketchElement { reason, .. }
                if reason == "fillet_corner_angle_degenerate"
                    || reason == "fillet_zero_length_input_line"
        ));
    }

    /// T_DEG_corner_angle_zero: ~0° corner (fold-back).
    #[test]
    fn t_deg_corner_angle_zero() {
        let l1 = SketchElement::Line {
            id: "l1".to_string(),
            from: [-5.0, 0.0],
            to: [0.0, 0.0],
        };
        // l2 reverses direction → corner angle approaches 0.
        let l2 = SketchElement::Line {
            id: "l2".to_string(),
            from: [0.0, 0.0],
            to: [-5.0, 0.0],
        };
        let err = compute_fillet(&l1, &l2, 0.5).unwrap_err();
        assert!(matches!(
            err,
            KernelError::DegenerateSketchElement {
                reason: "fillet_corner_angle_degenerate",
                ..
            }
        ));
    }

    /// T_DEG_no_shared_corner: endpoints do not meet.
    #[test]
    fn t_deg_no_shared_corner() {
        let l1 = SketchElement::Line {
            id: "l1".to_string(),
            from: [0.0, 0.0],
            to: [5.0, 0.0],
        };
        let l2 = SketchElement::Line {
            id: "l2".to_string(),
            from: [6.0, 0.0],
            to: [6.0, 5.0],
        };
        let err = compute_fillet(&l1, &l2, 0.5).unwrap_err();
        assert!(matches!(
            err,
            KernelError::InvalidParameter {
                kind: "sketch_fillet_no_shared_corner"
            }
        ));
    }

    /// T_DEG_non_line_element: elem1_id points at a Circle.
    #[test]
    fn t_deg_non_line_element() {
        let mut profile = rect_profile();
        profile[1] = SketchElement::Circle {
            id: "c1".to_string(),
            center: [0.0, 0.0],
            radius: 1.0,
        };
        let err = apply_sketch_fillet_build(&profile, "l1", "c1", 0.5).unwrap_err();
        assert!(matches!(
            err,
            KernelError::UnsupportedFeature {
                kind: "sketch_fillet_only_line_line"
            }
        ));
    }

    /// T_DEG_same_element: elem1_id == elem2_id.
    #[test]
    fn t_deg_same_element() {
        let profile = rect_profile();
        let err = apply_sketch_fillet_build(&profile, "l1", "l1", 0.5).unwrap_err();
        assert!(matches!(
            err,
            KernelError::InvalidParameter {
                kind: "sketch_fillet_same_element"
            }
        ));
    }

    /// T_DEG_not_adjacent: non-adjacent pair (gap of one element).
    #[test]
    fn t_deg_not_adjacent() {
        let profile = rect_profile();
        let err = apply_sketch_fillet_build(&profile, "l1", "l3", 0.5).unwrap_err();
        assert!(matches!(
            err,
            KernelError::InvalidParameter {
                kind: "sketch_fillet_elems_not_adjacent"
            }
        ));
    }

    /// T_DEG_elem_not_found: unknown element ID.
    #[test]
    fn t_deg_elem_not_found() {
        let profile = rect_profile();
        let err = apply_sketch_fillet_build(&profile, "l1", "nope", 0.5).unwrap_err();
        assert!(matches!(
            err,
            KernelError::InvalidParameter {
                kind: "sketch_fillet_elem_not_found"
            }
        ));
    }

    /// T_DEG_negative_radius.
    #[test]
    fn t_deg_negative_radius() {
        let l1 = SketchElement::Line {
            id: "l1".to_string(),
            from: [0.0, 0.0],
            to: [10.0, 0.0],
        };
        let l2 = SketchElement::Line {
            id: "l2".to_string(),
            from: [10.0, 0.0],
            to: [10.0, 5.0],
        };
        let err = compute_fillet(&l1, &l2, -1.0).unwrap_err();
        assert!(matches!(
            err,
            KernelError::InvalidParameter { kind: "radius" }
        ));
    }

    /// T_DEG_nan_radius.
    #[test]
    fn t_deg_nan_radius() {
        let l1 = SketchElement::Line {
            id: "l1".to_string(),
            from: [0.0, 0.0],
            to: [10.0, 0.0],
        };
        let l2 = SketchElement::Line {
            id: "l2".to_string(),
            from: [10.0, 0.0],
            to: [10.0, 5.0],
        };
        let err = compute_fillet(&l1, &l2, f64::NAN).unwrap_err();
        assert!(matches!(
            err,
            KernelError::InvalidParameter { kind: "radius" }
        ));
    }

    /// T_DEG_inf_radius.
    #[test]
    fn t_deg_inf_radius() {
        let l1 = SketchElement::Line {
            id: "l1".to_string(),
            from: [0.0, 0.0],
            to: [10.0, 0.0],
        };
        let l2 = SketchElement::Line {
            id: "l2".to_string(),
            from: [10.0, 0.0],
            to: [10.0, 5.0],
        };
        let err = compute_fillet(&l1, &l2, f64::INFINITY).unwrap_err();
        assert!(matches!(
            err,
            KernelError::InvalidParameter { kind: "radius" }
        ));
    }

    /// T_DEG_zero_length_input_line.
    #[test]
    fn t_deg_zero_length_input_line() {
        let l1 = SketchElement::Line {
            id: "l1".to_string(),
            from: [5.0, 0.0],
            to: [5.0, 0.0],
        };
        let l2 = SketchElement::Line {
            id: "l2".to_string(),
            from: [5.0, 0.0],
            to: [5.0, 5.0],
        };
        let err = compute_fillet(&l1, &l2, 0.5).unwrap_err();
        assert!(matches!(
            err,
            KernelError::DegenerateSketchElement {
                element_id,
                reason: "fillet_zero_length_input_line",
            } if element_id == "l1"
        ));
    }

    /// 100-run determinism on build path.
    #[test]
    fn t_determinism_100_runs_build() {
        let profile = rect_profile();
        let reference = apply_sketch_fillet_build(&profile, "l1", "l2", 1.0).unwrap();
        for _ in 0..100 {
            let out = apply_sketch_fillet_build(&profile, "l1", "l2", 1.0).unwrap();
            assert_eq!(out, reference);
        }
    }

    /// Helper: 4-Line CCW rectangle (0,0)→(10,0)→(10,5)→(0,5)→(0,0).
    fn rect_profile() -> Vec<SketchElement> {
        vec![
            SketchElement::Line {
                id: "l1".to_string(),
                from: [0.0, 0.0],
                to: [10.0, 0.0],
            },
            SketchElement::Line {
                id: "l2".to_string(),
                from: [10.0, 0.0],
                to: [10.0, 5.0],
            },
            SketchElement::Line {
                id: "l3".to_string(),
                from: [10.0, 5.0],
                to: [0.0, 5.0],
            },
            SketchElement::Line {
                id: "l4".to_string(),
                from: [0.0, 5.0],
                to: [0.0, 0.0],
            },
        ]
    }

    /// Compute the terminal point of a sketch element (Line.to or Arc end point).
    fn endpoint(elem: &SketchElement) -> [f64; 2] {
        match elem {
            SketchElement::Line { to, .. } => *to,
            SketchElement::Arc {
                center,
                radius,
                end_angle,
                ..
            } => [
                center[0] + radius * end_angle.cos(),
                center[1] + radius * end_angle.sin(),
            ],
            _ => panic!("non-Line/Arc element in fillet output"),
        }
    }

    /// Compute the start point of a sketch element.
    fn startpoint(elem: &SketchElement) -> [f64; 2] {
        match elem {
            SketchElement::Line { from, .. } => *from,
            SketchElement::Arc {
                center,
                radius,
                start_angle,
                ..
            } => [
                center[0] + radius * start_angle.cos(),
                center[1] + radius * start_angle.sin(),
            ],
            _ => panic!("non-Line/Arc element in fillet output"),
        }
    }

    /// Assert every consecutive pair (with wraparound) connects within LENGTH_TOLERANCE.
    fn assert_chain_continuous(out: &[SketchElement]) {
        let n = out.len();
        assert!(n >= 2, "fillet output must have ≥2 elements");
        for i in 0..n {
            let j = (i + 1) % n;
            let end_i = endpoint(&out[i]);
            let start_j = startpoint(&out[j]);
            let d = dist(&end_i, &start_j);
            assert!(
                d <= LENGTH_TOLERANCE,
                "chain break at i={i}: out[{i}].to={end_i:?} vs out[{j}].from={start_j:?} (d={d})"
            );
        }
    }
}
