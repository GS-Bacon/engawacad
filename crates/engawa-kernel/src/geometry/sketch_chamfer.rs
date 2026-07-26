//! 2D sketch chamfer (bevel) at a shared corner of two sketch elements.
//!
//! Phase 10 (#297): Line-Line only, symmetric (single `length`). Arc/Arc, Line/Arc,
//! Circle, Ellipse, Conic and asymmetric (distance_a/distance_b) chamfer are
//! Out-of-Scope (separate Issue).
//!
//! # Element-level contract (`compute_chamfer`)
//! - `elem_a` / `elem_b` must both be `SketchElement::Line`
//! - `elem_a.to == elem_b.from` (the shared corner) within `LENGTH_TOLERANCE`
//! - Corner angle `theta` must be in `(ANGLE_TOLERANCE, π - ANGLE_TOLERANCE)`
//! - Cut length `length` (equal along both Lines) must not exceed either Line length
//!
//! # Build-level contract (`apply_sketch_chamfer_build`)
//! - `elem1_id` and `elem2_id` must resolve to adjacent Lines in the profile array
//!   (the pair `[last, first]` of a closed loop also counts as adjacent)
//! - Input order of `elem1_id` / `elem2_id` does not affect the result

use crate::error::KernelError;
use crate::geometry::math::{ANGLE_TOLERANCE, LENGTH_TOLERANCE};
use crate::geometry::sketch_fillet::find_adjacent_pair;
use engawa_format::SketchElement;

/// Element-level chamfer: trim two Lines to cut points and insert a connecting Line.
///
/// `elem_a` and `elem_b` are the source Lines in array order — `elem_a.to` must equal
/// `elem_b.from` (the shared corner). The caller is responsible for normalising input
/// order (see `find_adjacent_pair`).
///
/// Returns `(trimmed_a, new_line, trimmed_b)` in the original array order.
///
/// # Errors
/// - `UnsupportedFeature { kind: "sketch_fillet_only_line_line" }` — either element is not a Line
///   (kind string is fillet-derived because `as_line` is duplicated from `sketch_fillet.rs`;
///   variant is what matters, see ADR-018)
/// - `InvalidParameter { kind: "sketch_fillet_no_shared_corner" }` — `a.to` != `b.from`
/// - `InvalidParameter { kind: "length" }` — length is non-positive or non-finite
/// - `DegenerateSketchElement { reason: "chamfer_zero_length_input_line" }`
/// - `DegenerateSketchElement { reason: "chamfer_corner_angle_degenerate" }`
/// - `ChamferLengthTooLarge { elem1_id, elem2_id, length }` — cut length exceeds Line length
pub fn compute_chamfer(
    elem_a: &SketchElement,
    elem_b: &SketchElement,
    length: f64,
) -> Result<(SketchElement, SketchElement, SketchElement), KernelError> {
    let (a_id, a_from, a_to) = as_line(elem_a)?;
    let (b_id, b_from, b_to) = as_line(elem_b)?;

    if !length.is_finite() || length <= LENGTH_TOLERANCE {
        return Err(KernelError::InvalidParameter { kind: "length" });
    }

    if dist(&a_to, &b_from) > LENGTH_TOLERANCE {
        return Err(KernelError::InvalidParameter {
            kind: "sketch_fillet_no_shared_corner",
        });
    }
    let corner = a_to; // == b_from

    let len_a = dist(&a_from, &a_to);
    let len_b = dist(&b_from, &b_to);
    if len_a <= LENGTH_TOLERANCE {
        return Err(KernelError::DegenerateSketchElement {
            element_id: a_id.to_string(),
            reason: "chamfer_zero_length_input_line",
        });
    }
    if len_b <= LENGTH_TOLERANCE {
        return Err(KernelError::DegenerateSketchElement {
            element_id: b_id.to_string(),
            reason: "chamfer_zero_length_input_line",
        });
    }

    let d_a = normalize(sub(&a_from, &corner)); // corner→far_a
    let d_b = normalize(sub(&b_to, &corner)); // corner→far_b

    let theta = dot(&d_a, &d_b).clamp(-1.0, 1.0).acos();
    // The tangent length `t = length` does not depend on theta, but chamfer is only
    // geometrically meaningful at a real corner (direction change). θ≈0 (fold-back)
    // and θ≈π (collinear) are both "no corner to chamfer" cases — reject them with
    // the same threshold as fillet for consistency.
    if theta <= ANGLE_TOLERANCE || theta >= std::f64::consts::PI - ANGLE_TOLERANCE {
        return Err(KernelError::DegenerateSketchElement {
            element_id: format!("{a_id}_{b_id}"),
            reason: "chamfer_corner_angle_degenerate",
        });
    }

    let t = length;
    if t > len_a - LENGTH_TOLERANCE || t > len_b - LENGTH_TOLERANCE {
        return Err(KernelError::ChamferLengthTooLarge {
            elem1_id: a_id.to_string(),
            elem2_id: b_id.to_string(),
            length,
        });
    }

    let cut_a = add(&corner, &scale(&d_a, t));
    let cut_b = add(&corner, &scale(&d_b, t));

    let new_a = SketchElement::Line {
        id: a_id.to_string(),
        from: a_from,
        to: cut_a,
    };
    let new_b = SketchElement::Line {
        id: b_id.to_string(),
        from: cut_b,
        to: b_to,
    };
    let new_line = SketchElement::Line {
        id: format!("{a_id}_{b_id}_chamfer_line"),
        from: cut_a,
        to: cut_b,
    };

    Ok((new_a, new_line, new_b))
}

/// Build-level chamfer: locate the adjacent Lines, splice in the connecting Line,
/// and trim both Lines. Reuses `find_adjacent_pair` from `sketch_fillet` (no geometry
/// in that helper — see plan.md).
///
/// # Errors
/// In addition to `compute_chamfer` errors:
/// - `InvalidParameter { kind: "sketch_fillet_elem_not_found" }`
/// - `InvalidParameter { kind: "sketch_fillet_same_element" }`
/// - `InvalidParameter { kind: "sketch_fillet_elems_not_adjacent" }`
/// - `InvalidParameter { kind: "sketch_chamfer_line_id_collision" }` — derived Line ID collides
///   with an existing user element ID
pub fn apply_sketch_chamfer_build(
    source: &[SketchElement],
    elem1_id: &str,
    elem2_id: &str,
    length: f64,
) -> Result<Vec<SketchElement>, KernelError> {
    let n = source.len();
    let (a_idx, b_idx) = find_adjacent_pair(source, elem1_id, elem2_id)?;

    let line_id_a = element_id(&source[a_idx]);
    let line_id_b = element_id(&source[b_idx]);
    let prospective_line_id = format!("{line_id_a}_{line_id_b}_chamfer_line");
    if source.iter().any(|e| element_id(e) == prospective_line_id) {
        return Err(KernelError::InvalidParameter {
            kind: "sketch_chamfer_line_id_collision",
        });
    }

    let (new_a, new_line, new_b) = compute_chamfer(&source[a_idx], &source[b_idx], length)?;

    let mut out = source.to_vec();
    out[a_idx] = new_a;
    out[b_idx] = new_b;
    // For wraparound (b_idx == 0, a_idx == n-1) insert at the tail (== n) to preserve
    // array-order traversal `a → line → b → ...`. Otherwise insert between a and b.
    let insert_at = if b_idx == a_idx + 1 { a_idx + 1 } else { n };
    out.insert(insert_at, new_line);
    Ok(out)
}

// --- private helpers (duplicated from sketch_fillet.rs intentionally; see plan.md) ---

fn as_line(elem: &SketchElement) -> Result<(&str, [f64; 2], [f64; 2]), KernelError> {
    match elem {
        SketchElement::Line { id, from, to } => Ok((id.as_str(), *from, *to)),
        _ => Err(KernelError::UnsupportedFeature {
            // kind string is fillet-derived (helper duplicated from sketch_fillet.rs);
            // variant is what matters per ADR-018.
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
    if n <= LENGTH_TOLERANCE {
        [0.0, 0.0]
    } else {
        [a[0] / n, a[1] / n]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// T01: Determinism — same input produces same output, and derived Line id matches
    /// the `{a}_{b}_chamfer_line` convention.
    #[test]
    fn t01_determinism_and_derived_line_id() {
        let profile = rect_profile();
        let out1 = apply_sketch_chamfer_build(&profile, "l1", "l2", 1.0).unwrap();
        let out2 = apply_sketch_chamfer_build(&profile, "l1", "l2", 1.0).unwrap();
        assert_eq!(out1, out2);

        let line = out1
            .iter()
            .find(|e| matches!(e, SketchElement::Line { id, .. } if id.ends_with("_chamfer_line")))
            .expect("chamfer should insert a derived Line");
        let id = match line {
            SketchElement::Line { id, .. } => id.as_str(),
            _ => unreachable!(),
        };
        assert_eq!(id, "l1_l2_chamfer_line");
    }

    /// T01b: elem1_id/elem2_id order invariance (basic case + wraparound).
    #[test]
    fn t01b_input_order_invariance() {
        let profile = rect_profile();
        let forward = apply_sketch_chamfer_build(&profile, "l1", "l2", 1.0).unwrap();
        let reverse = apply_sketch_chamfer_build(&profile, "l2", "l1", 1.0).unwrap();
        assert_eq!(forward, reverse);

        let fwd_wrap = apply_sketch_chamfer_build(&profile, "l4", "l1", 1.0).unwrap();
        let rev_wrap = apply_sketch_chamfer_build(&profile, "l1", "l4", 1.0).unwrap();
        assert_eq!(fwd_wrap, rev_wrap);
    }

    /// T01c: Line ID collision guard.
    #[test]
    fn t01c_line_id_collision_rejected() {
        let mut profile = rect_profile();
        profile.push(SketchElement::Line {
            id: "l1_l2_chamfer_line".to_string(),
            from: [0.0, 0.0],
            to: [1.0, 1.0],
        });
        let err = apply_sketch_chamfer_build(&profile, "l1", "l2", 0.5).unwrap_err();
        assert!(matches!(
            err,
            KernelError::InvalidParameter {
                kind: "sketch_chamfer_line_id_collision"
            }
        ));
    }

    /// T02: 90° corner — analytic cut points.
    /// Lines: l1 (0,0)→(10,0), l2 (10,0)→(10,5). length=1.0.
    /// corner=(10,0). d_a = normalize((0,0)-(10,0))=(-1,0). d_b = normalize((10,5)-(10,0))=(0,1).
    /// cut_a = corner + d_a * 1 = (9, 0). cut_b = corner + d_b * 1 = (10, 1).
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
        let (new_a, new_line, new_b) = compute_chamfer(&l1, &l2, 1.0).unwrap();
        match (new_a, new_line, new_b) {
            (
                SketchElement::Line { to: a_to, .. },
                SketchElement::Line {
                    from: l_from,
                    to: l_to,
                    ..
                },
                SketchElement::Line { from: b_from, .. },
            ) => {
                assert!((a_to[0] - 9.0).abs() < LENGTH_TOLERANCE);
                assert!((a_to[1] - 0.0).abs() < LENGTH_TOLERANCE);
                assert!((b_from[0] - 10.0).abs() < LENGTH_TOLERANCE);
                assert!((b_from[1] - 1.0).abs() < LENGTH_TOLERANCE);
                assert_eq!(l_from, a_to);
                assert_eq!(l_to, b_from);
            }
            _ => panic!("compute_chamfer returned wrong variant triple"),
        }
    }

    /// T_DEG_chamfer_too_long: short Line pair with length=10 → ChamferLengthTooLarge.
    #[test]
    fn t_deg_chamfer_too_long() {
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
        let err = compute_chamfer(&l1, &l2, 10.0).unwrap_err();
        assert!(matches!(
            err,
            KernelError::ChamferLengthTooLarge {
                elem1_id,
                elem2_id,
                length: 10.0,
            } if elem1_id == "l1" && elem2_id == "l2"
        ));
    }

    /// 100-run determinism on build path.
    #[test]
    fn t_determinism_100_runs_build() {
        let profile = rect_profile();
        let reference = apply_sketch_chamfer_build(&profile, "l1", "l2", 1.0).unwrap();
        for _ in 0..100 {
            let out = apply_sketch_chamfer_build(&profile, "l1", "l2", 1.0).unwrap();
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
}
