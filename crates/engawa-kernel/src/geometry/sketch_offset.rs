//! 2D sketch element offset (parallel offset, signed distance).
//!
//! Converts `SketchElement` variants (Line, Circle, Arc) into offset counterparts.
//! Ellipse and Conic are unsupported (numerical iterative methods required).
//!
//! # Build-level contract (apply_sketch_offset_build)
//! - Circle single-element only (Line/Arc require corner join/trim, deferred to Phase 11+)
//! - Use `apply_sketch_offset` for kernel-level pure function testing (per-element offset)
//!
//! # Kernel-level contract (apply_sketch_offset)
//! - Line/Arc offset works on isolated elements (unit test coverage)
//! - Negative sweep Arc is rejected (符号規約が正 sweep 前提)

use crate::error::KernelError;
use crate::geometry::math::LENGTH_TOLERANCE;
use engawa_format::SketchElement;
use std::collections::BTreeSet;

/// Build-level SketchOffset: Circle single-element only.
///
/// Enforces Phase 10 scope defense: Line/Arc-based closed loop offset requires
/// corner join/trim (Trim/Extend #276), deferred to Phase 11+.
///
/// # Errors
/// - `UnsupportedFeature { kind: "sketch_offset_only_circle" }` if profile is not single Circle
pub fn apply_sketch_offset_build(
    source: &[SketchElement],
    selection: &[String],
    distance: f64,
) -> Result<Vec<SketchElement>, KernelError> {
    if source.len() != 1 || !matches!(source[0], SketchElement::Circle { .. }) {
        return Err(KernelError::UnsupportedFeature {
            kind: "sketch_offset_only_circle",
        });
    }
    apply_sketch_offset(source, selection, distance)
}

/// Apply offset to selected elements in a sketch profile.
///
/// Kernel-level pure function: per-element offset (Line, Circle, Arc).
/// Use for unit testing; build-level dispatch uses `apply_sketch_offset_build`.
///
/// - `distance`: signed offset distance (positive = left/outer, negative = right/inner)
/// - `selection`: element IDs to offset; empty = all elements
/// - Returns: new profile with offset applied to selected elements (others unchanged)
///
/// # Errors
/// - `InvalidParameter { kind: "distance" }` if distance is NaN or Inf
/// - `DegenerateSketchElement` if offset result collapses (zero-length line, zero-radius circle/arc)
/// - `UnsupportedFeature { kind: "sketch_offset_of_ellipse_or_conic" }` if Ellipse/Conic in selection
/// - `InvalidParameter { kind: "arc_negative_sweep" }` if Arc has negative sweep (end_angle < start_angle)
pub fn apply_sketch_offset(
    source: &[SketchElement],
    selection: &[String],
    distance: f64,
) -> Result<Vec<SketchElement>, KernelError> {
    if !distance.is_finite() {
        return Err(KernelError::InvalidParameter { kind: "distance" });
    }
    if distance.abs() <= LENGTH_TOLERANCE {
        return Ok(source.to_vec());
    }

    let selected: BTreeSet<&str> = if selection.is_empty() {
        source.iter().map(element_id).collect()
    } else {
        selection.iter().map(String::as_str).collect()
    };

    source
        .iter()
        .map(|elem| {
            let eid = element_id(elem);
            if selected.contains(eid) {
                offset_element(elem, distance)
            } else {
                Ok(elem.clone())
            }
        })
        .collect()
}

/// Extract element ID from SketchElement (helper for deterministic selection).
fn element_id(elem: &SketchElement) -> &str {
    match elem {
        SketchElement::Line { id, .. }
        | SketchElement::Circle { id, .. }
        | SketchElement::Arc { id, .. }
        | SketchElement::Ellipse { id, .. }
        | SketchElement::Conic { id, .. } => id.as_str(),
    }
}

/// Offset a single sketch element.
fn offset_element(elem: &SketchElement, distance: f64) -> Result<SketchElement, KernelError> {
    match elem {
        SketchElement::Line { id, from, to } => offset_line(id, from, to, distance),
        SketchElement::Circle { id, center, radius } => {
            offset_circle(id, center, *radius, distance)
        }
        SketchElement::Arc {
            id,
            center,
            radius,
            start_angle,
            end_angle,
        } => offset_arc(id, center, *radius, *start_angle, *end_angle, distance),
        SketchElement::Ellipse { id: _, .. } | SketchElement::Conic { id: _, .. } => {
            Err(KernelError::UnsupportedFeature {
                kind: "sketch_offset_of_ellipse_or_conic",
            })
        }
    }
}

/// Offset a line segment (parallel translation perpendicular to direction).
///
/// Offset direction: rotate direction 90° CCW (left-hand side from +z view).
/// - new_from = from + distance * R90CCW(direction)
/// - new_to = to + distance * R90CCW(direction)
fn offset_line(
    id: &str,
    from: &[f64; 2],
    to: &[f64; 2],
    distance: f64,
) -> Result<SketchElement, KernelError> {
    let dx = to[0] - from[0];
    let dy = to[1] - from[1];
    let length = (dx * dx + dy * dy).sqrt();

    if length <= LENGTH_TOLERANCE {
        return Err(KernelError::DegenerateSketchElement {
            element_id: id.to_string(),
            reason: "collapsed_offset_zero_length",
        });
    }

    // Unit direction
    let ux = dx / length;
    let uy = dy / length;

    // Perpendicular (90° CCW): (-uy, ux)
    let px = -uy * distance;
    let py = ux * distance;

    let new_from = [from[0] + px, from[1] + py];
    let new_to = [to[0] + px, to[1] + py];

    Ok(SketchElement::Line {
        id: id.to_string(),
        from: new_from,
        to: new_to,
    })
}

/// Offset a circle (signed radius change).
///
/// new_radius = radius + distance
/// - Positive distance = outer (radius increases)
/// - Negative distance = inner (radius decreases)
///
/// Center unchanged.
fn offset_circle(
    id: &str,
    center: &[f64; 2],
    radius: f64,
    distance: f64,
) -> Result<SketchElement, KernelError> {
    let new_radius = radius + distance;
    if new_radius <= LENGTH_TOLERANCE {
        return Err(KernelError::DegenerateSketchElement {
            element_id: id.to_string(),
            reason: "collapsed_offset_negative_radius",
        });
    }
    Ok(SketchElement::Circle {
        id: id.to_string(),
        center: *center,
        radius: new_radius,
    })
}

/// Offset an arc (signed radius change, angles preserved).
///
/// new_radius = radius + distance
/// start_angle and end_angle unchanged.
///
/// # Errors
/// - `InvalidParameter { kind: "arc_negative_sweep" }` if end_angle < start_angle (負 sweep 未対応)
fn offset_arc(
    id: &str,
    center: &[f64; 2],
    radius: f64,
    start_angle: f64,
    end_angle: f64,
    distance: f64,
) -> Result<SketchElement, KernelError> {
    if end_angle < start_angle {
        return Err(KernelError::InvalidParameter {
            kind: "arc_negative_sweep",
        });
    }
    let new_radius = radius + distance;
    if new_radius <= LENGTH_TOLERANCE {
        return Err(KernelError::DegenerateSketchElement {
            element_id: id.to_string(),
            reason: "collapsed_offset_negative_radius",
        });
    }
    Ok(SketchElement::Arc {
        id: id.to_string(),
        center: *center,
        radius: new_radius,
        start_angle,
        end_angle,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::f64::consts::PI;

    /// T01: Determinism — same input produces same output on repeated calls.
    #[test]
    fn t01_offset_deterministic() {
        let profile = vec![
            SketchElement::Line {
                id: "l1".to_string(),
                from: [0.0, 0.0],
                to: [10.0, 0.0],
            },
            SketchElement::Circle {
                id: "c1".to_string(),
                center: [5.0, 5.0],
                radius: 2.0,
            },
        ];
        let out1 = apply_sketch_offset(&profile, &[], 1.0).unwrap();
        let out2 = apply_sketch_offset(&profile, &[], 1.0).unwrap();
        assert_eq!(out1, out2);
    }

    /// T01a: Selection order invariance — different selection orders produce same output.
    #[test]
    fn t01a_selection_order_invariant() {
        let profile = vec![
            SketchElement::Line {
                id: "l_a".to_string(),
                from: [0.0, 0.0],
                to: [10.0, 0.0],
            },
            SketchElement::Line {
                id: "l_b".to_string(),
                from: [10.0, 0.0],
                to: [10.0, 5.0],
            },
            SketchElement::Circle {
                id: "c_c".to_string(),
                center: [5.0, 2.5],
                radius: 1.0,
            },
        ];
        let out_forward = apply_sketch_offset(
            &profile,
            &["l_a".to_string(), "l_b".to_string(), "c_c".to_string()],
            1.0,
        )
        .unwrap();
        let out_reverse = apply_sketch_offset(
            &profile,
            &["c_c".to_string(), "l_b".to_string(), "l_a".to_string()],
            1.0,
        )
        .unwrap();
        assert_eq!(out_forward, out_reverse);
    }

    /// T02: Line offset (normal case).
    #[test]
    fn t02_line_offset() {
        let line = SketchElement::Line {
            id: "l1".to_string(),
            from: [0.0, 0.0],
            to: [10.0, 0.0],
        };
        let out = apply_sketch_offset(&[line.clone()], &["l1".to_string()], 1.0).unwrap();
        match &out[0] {
            SketchElement::Line { from, to, .. } => {
                assert!((from[0] - 0.0).abs() < LENGTH_TOLERANCE);
                assert!((from[1] - 1.0).abs() < LENGTH_TOLERANCE);
                assert!((to[0] - 10.0).abs() < LENGTH_TOLERANCE);
                assert!((to[1] - 1.0).abs() < LENGTH_TOLERANCE);
            }
            _ => panic!("expected Line"),
        }
    }

    /// T03: Circle offset (normal case).
    #[test]
    fn t03_circle_offset() {
        let circle = SketchElement::Circle {
            id: "c1".to_string(),
            center: [0.0, 0.0],
            radius: 5.0,
        };
        let out = apply_sketch_offset(&[circle.clone()], &["c1".to_string()], 2.0).unwrap();
        match &out[0] {
            SketchElement::Circle { radius, .. } => {
                assert!((*radius - 7.0).abs() < LENGTH_TOLERANCE);
            }
            _ => panic!("expected Circle"),
        }
    }

    /// T04: Arc offset (normal case).
    #[test]
    fn t04_arc_offset() {
        let arc = SketchElement::Arc {
            id: "a1".to_string(),
            center: [0.0, 0.0],
            radius: 5.0,
            start_angle: 0.0,
            end_angle: PI / 2.0,
        };
        let out = apply_sketch_offset(&[arc.clone()], &["a1".to_string()], -1.0).unwrap();
        match &out[0] {
            SketchElement::Arc {
                radius,
                start_angle,
                end_angle,
                ..
            } => {
                assert!((*radius - 4.0).abs() < LENGTH_TOLERANCE);
                assert!((start_angle - 0.0).abs() < LENGTH_TOLERANCE);
                assert!((*end_angle - PI / 2.0).abs() < LENGTH_TOLERANCE);
            }
            _ => panic!("expected Arc"),
        }
    }

    /// T_DEG_zero_distance: Near-zero distance is no-op.
    #[test]
    fn t_deg_zero_distance() {
        let line = SketchElement::Line {
            id: "l1".to_string(),
            from: [0.0, 0.0],
            to: [10.0, 0.0],
        };
        let out = apply_sketch_offset(&[line.clone()], &["l1".to_string()], 1e-10).unwrap();
        match &out[0] {
            SketchElement::Line { from, to, .. } => {
                assert_eq!(from, &[0.0, 0.0]);
                assert_eq!(to, &[10.0, 0.0]);
            }
            _ => panic!("expected Line"),
        }
    }

    /// T_DEG_zero_distance_boundary: EPS_LENGTH exactly is no-op.
    #[test]
    fn t_deg_zero_distance_boundary() {
        let line = SketchElement::Line {
            id: "l1".to_string(),
            from: [0.0, 0.0],
            to: [10.0, 0.0],
        };
        let out =
            apply_sketch_offset(&[line.clone()], &["l1".to_string()], LENGTH_TOLERANCE).unwrap();
        match &out[0] {
            SketchElement::Line { from, to, .. } => {
                assert_eq!(from, &[0.0, 0.0]);
                assert_eq!(to, &[10.0, 0.0]);
            }
            _ => panic!("expected Line"),
        }
    }

    /// T_DEG_offset_collapse_circle: Negative distance that collapses radius.
    #[test]
    fn t_deg_offset_collapse_circle() {
        let circle = SketchElement::Circle {
            id: "c1".to_string(),
            center: [0.0, 0.0],
            radius: 3.0,
        };
        let err = apply_sketch_offset(
            &[circle],
            &["c1".to_string()],
            -3.0 - LENGTH_TOLERANCE * 0.1,
        )
        .unwrap_err();
        assert!(matches!(
            err,
            KernelError::DegenerateSketchElement {
                element_id,
                reason: "collapsed_offset_negative_radius"
            } if element_id == "c1"
        ));
    }

    /// T_DEG_offset_collapse_line: Zero-length line fails.
    #[test]
    fn t_deg_offset_collapse_line() {
        let line = SketchElement::Line {
            id: "l1".to_string(),
            from: [0.0, 0.0],
            to: [LENGTH_TOLERANCE * 0.5, 0.0],
        };
        let err = apply_sketch_offset(&[line], &["l1".to_string()], 1.0).unwrap_err();
        assert!(matches!(
            err,
            KernelError::DegenerateSketchElement {
                element_id,
                reason: "collapsed_offset_zero_length"
            } if element_id == "l1"
        ));
    }

    /// T_DEG_ellipse_reject: Ellipse in selection is rejected.
    #[test]
    fn t_deg_ellipse_reject() {
        let ellipse = SketchElement::Ellipse {
            id: "e1".to_string(),
            center: [0.0, 0.0],
            major: 2.0,
            minor: 1.0,
            rotation: 0.0,
        };
        let err = apply_sketch_offset(&[ellipse], &["e1".to_string()], 1.0).unwrap_err();
        assert!(matches!(
            err,
            KernelError::UnsupportedFeature {
                kind: "sketch_offset_of_ellipse_or_conic"
            }
        ));
    }

    /// T_DEG_nan_distance: NaN distance is rejected.
    #[test]
    fn t_deg_nan_distance() {
        let line = SketchElement::Line {
            id: "l1".to_string(),
            from: [0.0, 0.0],
            to: [10.0, 0.0],
        };
        let err = apply_sketch_offset(&[line], &["l1".to_string()], f64::NAN).unwrap_err();
        assert!(matches!(
            err,
            KernelError::InvalidParameter { kind: "distance" }
        ));
    }

    /// T07: Selection partial application.
    /// Circle + Line mix (not all-Lines, so C-F01 does not apply).
    #[test]
    fn t07_selection_partial() {
        let profile = vec![
            SketchElement::Line {
                id: "l1".to_string(),
                from: [0.0, 0.0],
                to: [10.0, 0.0],
            },
            SketchElement::Circle {
                id: "c1".to_string(),
                center: [5.0, 5.0],
                radius: 2.0,
            },
        ];
        let out = apply_sketch_offset(&profile, &["l1".to_string()], 1.0).unwrap();
        match &out[0] {
            SketchElement::Line { id, from, .. } => {
                assert_eq!(id, "l1");
                assert!((from[1] - 1.0).abs() < LENGTH_TOLERANCE);
            }
            _ => panic!("expected offset l1"),
        }
        match &out[1] {
            SketchElement::Circle { id, radius, .. } => {
                assert_eq!(id, "c1");
                assert_eq!(*radius, 2.0); // Unchanged
            }
            _ => panic!("expected unchanged c1"),
        }
    }

    /// T08: Empty selection = all elements offset.
    #[test]
    fn t08_empty_selection_all() {
        let profile = vec![
            SketchElement::Line {
                id: "l1".to_string(),
                from: [0.0, 0.0],
                to: [10.0, 0.0],
            },
            SketchElement::Circle {
                id: "c1".to_string(),
                center: [5.0, 5.0],
                radius: 2.0,
            },
            SketchElement::Arc {
                id: "a1".to_string(),
                center: [0.0, 0.0],
                radius: 1.0,
                start_angle: 0.0,
                end_angle: PI / 2.0,
            },
        ];
        let out = apply_sketch_offset(&profile, &[], 1.0).unwrap();
        assert_eq!(out.len(), 3);
        // Verify all were offset
        match &out[0] {
            SketchElement::Line { from, .. } => {
                assert!((from[1] - 1.0).abs() < LENGTH_TOLERANCE);
            }
            _ => panic!("expected offset Line"),
        }
        match &out[1] {
            SketchElement::Circle { radius, .. } => {
                assert!((*radius - 3.0).abs() < LENGTH_TOLERANCE);
            }
            _ => panic!("expected offset Circle"),
        }
        match &out[2] {
            SketchElement::Arc { radius, .. } => {
                assert!((*radius - 2.0).abs() < LENGTH_TOLERANCE);
            }
            _ => panic!("expected offset Arc"),
        }
    }

    /// Conic in selection is rejected.
    #[test]
    fn t_deg_conic_reject() {
        let conic = SketchElement::Conic {
            id: "cn1".to_string(),
            coeffs: [1.0, 0.0, 1.0, 0.0, 0.0],
        };
        let err = apply_sketch_offset(&[conic], &["cn1".to_string()], 1.0).unwrap_err();
        assert!(matches!(
            err,
            KernelError::UnsupportedFeature {
                kind: "sketch_offset_of_ellipse_or_conic"
            }
        ));
    }

    /// Negative line offset (right-hand side).
    #[test]
    fn t_negative_line_offset() {
        let line = SketchElement::Line {
            id: "l1".to_string(),
            from: [0.0, 0.0],
            to: [10.0, 0.0],
        };
        let out = apply_sketch_offset(&[line.clone()], &["l1".to_string()], -1.0).unwrap();
        match &out[0] {
            SketchElement::Line { from, to, .. } => {
                assert!((from[0] - 0.0).abs() < LENGTH_TOLERANCE);
                assert!((from[1] - (-1.0)).abs() < LENGTH_TOLERANCE);
                assert!((to[0] - 10.0).abs() < LENGTH_TOLERANCE);
                assert!((to[1] - (-1.0)).abs() < LENGTH_TOLERANCE);
            }
            _ => panic!("expected Line"),
        }
    }

    /// Diagonal line offset.
    #[test]
    fn t_diagonal_line_offset() {
        let line = SketchElement::Line {
            id: "l1".to_string(),
            from: [0.0, 0.0],
            to: [10.0, 10.0],
        };
        let out = apply_sketch_offset(&[line.clone()], &["l1".to_string()], 1.0).unwrap();
        match &out[0] {
            SketchElement::Line { from, to: _, .. } => {
                // Direction (1,1), perpendicular (-1,1) normalized ≈ (-0.707, 0.707)
                // Offset = (-0.707, 0.707)
                let expected_offset = 1.0 / 2.0_f64.sqrt();
                assert!((from[0] - (-expected_offset)).abs() < 1e-9);
                assert!((from[1] - expected_offset).abs() < 1e-9);
            }
            _ => panic!("expected Line"),
        }
    }

    /// 100-run determinism for offset operations.
    #[test]
    fn t_determinism_100_runs() {
        let profile = vec![
            SketchElement::Line {
                id: "l1".to_string(),
                from: [0.0, 0.0],
                to: [10.0, 0.0],
            },
            SketchElement::Circle {
                id: "c1".to_string(),
                center: [5.0, 5.0],
                radius: 2.0,
            },
        ];
        let reference = apply_sketch_offset(&profile, &[], 1.0).unwrap();
        for _ in 0..100 {
            let out = apply_sketch_offset(&profile, &[], 1.0).unwrap();
            assert_eq!(out, reference);
        }
    }

    /// T_DEG_inf_distance: Inf distance is rejected.
    #[test]
    fn t_deg_inf_distance() {
        let line = SketchElement::Line {
            id: "l1".to_string(),
            from: [0.0, 0.0],
            to: [10.0, 0.0],
        };
        let err = apply_sketch_offset(&[line], &["l1".to_string()], f64::INFINITY).unwrap_err();
        assert!(matches!(
            err,
            KernelError::InvalidParameter { kind: "distance" }
        ));
    }

    /// T_DEG_neg_inf_distance: -Inf distance is rejected.
    #[test]
    fn t_deg_neg_inf_distance() {
        let line = SketchElement::Line {
            id: "l1".to_string(),
            from: [0.0, 0.0],
            to: [10.0, 0.0],
        };
        let err = apply_sketch_offset(&[line], &["l1".to_string()], f64::NEG_INFINITY).unwrap_err();
        assert!(matches!(
            err,
            KernelError::InvalidParameter { kind: "distance" }
        ));
    }

    /// T_DEG_offset_arc_negative_sweep_rejected: Negative sweep Arc is rejected.
    /// A-F02: offset_arc does not support end_angle < start_angle.
    #[test]
    fn t_deg_offset_arc_negative_sweep_rejected() {
        let arc = SketchElement::Arc {
            id: "a1".to_string(),
            center: [0.0, 0.0],
            radius: 5.0,
            start_angle: PI / 2.0,
            end_angle: 0.0, // Negative sweep
        };
        let err = apply_sketch_offset(&[arc], &["a1".to_string()], 1.0).unwrap_err();
        assert!(matches!(
            err,
            KernelError::InvalidParameter {
                kind: "arc_negative_sweep"
            }
        ));
    }
}
