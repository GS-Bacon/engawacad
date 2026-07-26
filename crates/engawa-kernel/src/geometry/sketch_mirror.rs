//! 2D sketch mirror (reflection) across a 2-point axis line.
//!
//! Phase 10 (#298): Line/Circle/Arc only. Ellipse/Conic are Out-of-Scope (separate Issue,
//! matches SketchOffset's exact same exclusion — closed-form conic reflection would still
//! require iterative parameter re-fitting for trim/segmentation downstream).
//!
//! Mirrored duplicates are **appended** to the profile (originals are preserved, not modified),
//! and receive deterministic ids `"{elem_id}_mirror"`. Input order of `selection` does not
//! affect the result — elements are mirrored in source-array order.
//!
//! # Errors
//! - `InvalidParameter { kind: "sketch_mirror_axis_degenerate" }` — axis_p1 == axis_p2 within
//!   `LENGTH_TOLERANCE`, or either axis point contains NaN/Inf
//! - `InvalidParameter { kind: "sketch_mirror_unknown_element_id" }` — selection id not in profile
//! - `UnsupportedFeature { kind: "sketch_mirror_of_ellipse_or_conic" }` — Ellipse/Conic in selection
//! - `DegenerateSketchElement { reason: "mirror_axis_coincident" }` — element lies on the axis
//!   (its reflection coincides with itself)
//! - `DegenerateSketchElement { reason: "mirror_duplicate_id" }` — derived id `"{elem_id}_mirror"`
//!   already present in profile

use crate::error::KernelError;
use crate::geometry::math::{ANGLE_TOLERANCE, LENGTH_TOLERANCE};
use engawa_format::SketchElement;
use std::collections::BTreeSet;
use std::f64::consts::{PI, TAU};

/// Apply mirror (reflection) to selected elements in a sketch profile.
///
/// Kernel-level pure function: per-element reflection across the line through `axis_p1`
/// and `axis_p2`. Source elements are preserved unchanged; mirrored duplicates are
/// appended in source-array order with ids `"{elem_id}_mirror"`.
///
/// - `selection`: element IDs to mirror; empty = all elements
/// - Returns: new profile with mirrored duplicates appended
pub fn apply_sketch_mirror(
    source: &[SketchElement],
    axis_p1: [f64; 2],
    axis_p2: [f64; 2],
    selection: &[String],
) -> Result<Vec<SketchElement>, KernelError> {
    if !is_finite_point(&axis_p1) || !is_finite_point(&axis_p2) {
        return Err(KernelError::InvalidParameter {
            kind: "sketch_mirror_axis_degenerate",
        });
    }
    let axis_len = dist(&axis_p1, &axis_p2);
    if axis_len <= LENGTH_TOLERANCE {
        return Err(KernelError::InvalidParameter {
            kind: "sketch_mirror_axis_degenerate",
        });
    }
    let d = normalize(sub(&axis_p2, &axis_p1));

    // Resolve selection: empty = all; unknown id = error. BTreeSet makes the iteration
    // order deterministic w.r.t. selection contents (source-array order is used for
    // emitting mirrors, so selection order itself does not affect the result).
    let selected: BTreeSet<&str> = if selection.is_empty() {
        source.iter().map(element_id).collect()
    } else {
        let mut set: BTreeSet<&str> = BTreeSet::new();
        for id in selection {
            if !source.iter().any(|e| element_id(e) == id.as_str()) {
                return Err(KernelError::InvalidParameter {
                    kind: "sketch_mirror_unknown_element_id",
                });
            }
            set.insert(id.as_str());
        }
        set
    };

    let existing_ids: BTreeSet<&str> = source.iter().map(element_id).collect();

    let mut appended: Vec<SketchElement> = Vec::new();
    for elem in source {
        let eid = element_id(elem);
        if !selected.contains(eid) {
            continue;
        }
        let mirrored = mirror_element(elem, &axis_p1, &d)?;
        if is_axis_coincident(elem, &mirrored) {
            return Err(KernelError::DegenerateSketchElement {
                element_id: eid.to_string(),
                reason: "mirror_axis_coincident",
            });
        }
        let derived_id = format!("{eid}_mirror");
        if existing_ids.contains(derived_id.as_str()) {
            return Err(KernelError::DegenerateSketchElement {
                element_id: derived_id,
                reason: "mirror_duplicate_id",
            });
        }
        appended.push(rename_to(&mirrored, &derived_id));
    }

    let mut out = source.to_vec();
    out.append(&mut appended);
    Ok(out)
}

/// Reflect a single sketch element across the axis (point `p` + unit direction `d`).
fn mirror_element(
    elem: &SketchElement,
    p: &[f64; 2],
    d: &[f64; 2],
) -> Result<SketchElement, KernelError> {
    match elem {
        SketchElement::Line { id, from, to } => Ok(SketchElement::Line {
            id: id.clone(),
            from: reflect_point(from, p, d),
            to: reflect_point(to, p, d),
        }),
        SketchElement::Circle { id, center, radius } => Ok(SketchElement::Circle {
            id: id.clone(),
            center: reflect_point(center, p, d),
            radius: *radius,
        }),
        SketchElement::Arc {
            id,
            center,
            radius,
            start_angle,
            end_angle,
        } => {
            // Direct angle formula: reflection across a line through origin with direction
            // angle φ maps θ ↦ 2φ - θ. With axis offset by point `p`, the center translates
            // by reflect_point but the angular sweep formula stays the same (translation is
            // angle-preserving). The sweep sign is inverted because reflection reverses
            // orientation: new_end = new_start - (end_angle - start_angle).
            //
            // The earlier implementation derived new_start from atan2 of the reflected
            // start point, which suffers catastrophic cancellation when |center| >> radius
            // (atan2 of nearly-equal large numbers loses precision). Direct φ computation
            // is numerically stable.
            let new_center = reflect_point(center, p, d);
            let phi = d[1].atan2(d[0]);
            let new_start = 2.0 * phi - start_angle;
            let new_end = new_start - (end_angle - start_angle);
            Ok(SketchElement::Arc {
                id: id.clone(),
                center: new_center,
                radius: *radius,
                start_angle: new_start,
                end_angle: new_end,
            })
        }
        SketchElement::Ellipse { .. } | SketchElement::Conic { .. } => {
            Err(KernelError::UnsupportedFeature {
                kind: "sketch_mirror_of_ellipse_or_conic",
            })
        }
    }
}

/// Reflect point `q` across the line through `p` with unit direction `d`.
/// R(q) = p + (2*((q-p)·d)*d - (q-p))
fn reflect_point(q: &[f64; 2], p: &[f64; 2], d: &[f64; 2]) -> [f64; 2] {
    let v = sub(q, p);
    let v_dot_d = dot(&v, d);
    let scaled = scale(d, 2.0 * v_dot_d);
    let r = sub(&scaled, &v);
    add(p, &r)
}

/// Detect axis-coincident (self-symmetric) elements — mirroring them yields a duplicate
/// that overlaps the original. fail-fast per plan §3 (matches SketchFillet/Chamfer policy).
fn is_axis_coincident(original: &SketchElement, mirrored: &SketchElement) -> bool {
    match (original, mirrored) {
        (
            SketchElement::Line {
                from: o_from,
                to: o_to,
                ..
            },
            SketchElement::Line {
                from: m_from,
                to: m_to,
                ..
            },
        ) => {
            (points_near(o_from, m_from) && points_near(o_to, m_to))
                || (points_near(o_from, m_to) && points_near(o_to, m_from))
        }
        (SketchElement::Circle { center: o_c, .. }, SketchElement::Circle { center: m_c, .. }) => {
            points_near(o_c, m_c)
        }
        (
            SketchElement::Arc {
                center: o_c,
                start_angle: o_s,
                end_angle: o_e,
                radius: o_r,
                ..
            },
            SketchElement::Arc {
                center: m_c,
                start_angle: m_s,
                end_angle: m_e,
                ..
            },
        ) => {
            // Multi-turn arc (|sweep| ≈ 2π or more) covers the full circle, so any
            // reflection that keeps the center on the axis produces a self-overlap.
            // Otherwise: center must coincide (reflection preserves radius) AND start/end
            // must be swapped under mod-2π (because reflection inverts the sweep sign).
            let sweep = (*o_e - *o_s).abs();
            if sweep >= TAU - ANGLE_TOLERANCE {
                points_near(o_c, m_c)
            } else {
                let _ = o_r; // radius equality implied by `mirrored` being a copy
                points_near(o_c, m_c)
                    && angles_near_mod_2pi(*m_s, *o_e)
                    && angles_near_mod_2pi(*m_e, *o_s)
            }
        }
        _ => false,
    }
}

fn rename_to(elem: &SketchElement, new_id: &str) -> SketchElement {
    match elem {
        SketchElement::Line { from, to, .. } => SketchElement::Line {
            id: new_id.to_string(),
            from: *from,
            to: *to,
        },
        SketchElement::Circle { center, radius, .. } => SketchElement::Circle {
            id: new_id.to_string(),
            center: *center,
            radius: *radius,
        },
        SketchElement::Arc {
            center,
            radius,
            start_angle,
            end_angle,
            ..
        } => SketchElement::Arc {
            id: new_id.to_string(),
            center: *center,
            radius: *radius,
            start_angle: *start_angle,
            end_angle: *end_angle,
        },
        SketchElement::Ellipse { .. } | SketchElement::Conic { .. } => {
            unreachable!("mirror_element rejects Ellipse/Conic before rename")
        }
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

fn is_finite_point(p: &[f64; 2]) -> bool {
    p[0].is_finite() && p[1].is_finite()
}

fn dist(a: &[f64; 2], b: &[f64; 2]) -> f64 {
    // hypot avoids intermediate overflow for very large coordinates (Item 5).
    (b[0] - a[0]).hypot(b[1] - a[1])
}

fn points_near(a: &[f64; 2], b: &[f64; 2]) -> bool {
    dist(a, b) <= LENGTH_TOLERANCE
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
    // hypot avoids intermediate overflow for very large coordinates (Item 5). Without
    // this, axis=(0,0)-(1e300,0) overflows (dx*dx = inf) and normalize returns the zero
    // vector, silently degrading reflection to a point-symmetry around axis_p1.
    let n = a[0].hypot(a[1]);
    if n <= LENGTH_TOLERANCE {
        [0.0, 0.0]
    } else {
        [a[0] / n, a[1] / n]
    }
}

/// Compare two angles modulo 2π within `ANGLE_TOLERANCE`.
fn angles_near_mod_2pi(a: f64, b: f64) -> bool {
    let diff = (((a - b) % TAU) + TAU) % TAU; // [0, 2π)
    let d = if diff > PI { TAU - diff } else { diff }; // [0, π]
    d <= ANGLE_TOLERANCE
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::f64::consts::{FRAC_PI_2, FRAC_PI_4, PI};

    /// T01: Determinism — same input produces same output.
    #[test]
    fn t01_determinism() {
        let profile = vec![
            SketchElement::Line {
                id: "l1".to_string(),
                from: [2.0, 0.0],
                to: [2.0, 3.0],
            },
            SketchElement::Circle {
                id: "c1".to_string(),
                center: [3.0, 4.0],
                radius: 2.0,
            },
            SketchElement::Arc {
                id: "a1".to_string(),
                center: [0.0, 0.0],
                radius: 5.0,
                start_angle: 0.0,
                end_angle: FRAC_PI_2,
            },
        ];
        let out1 = apply_sketch_mirror(&profile, [0.0, 0.0], [0.0, 1.0], &[]).unwrap();
        let out2 = apply_sketch_mirror(&profile, [0.0, 0.0], [0.0, 1.0], &[]).unwrap();
        assert_eq!(out1, out2);
    }

    /// T02: Line mirror across y-axis (axis_p1=(0,0), axis_p2=(0,1)).
    #[test]
    fn t02_normal_line_mirror_y_axis() {
        let profile = vec![SketchElement::Line {
            id: "l1".to_string(),
            from: [2.0, 0.0],
            to: [2.0, 3.0],
        }];
        let out = apply_sketch_mirror(&profile, [0.0, 0.0], [0.0, 1.0], &[]).unwrap();
        assert_eq!(out.len(), 2);
        // Original unchanged.
        match &out[0] {
            SketchElement::Line { id, from, to } => {
                assert_eq!(id, "l1");
                assert_eq!(*from, [2.0, 0.0]);
                assert_eq!(*to, [2.0, 3.0]);
            }
            _ => panic!("expected original Line"),
        }
        // Mirror: (-2, 0) - (-2, 3), id "l1_mirror".
        match &out[1] {
            SketchElement::Line { id, from, to } => {
                assert_eq!(id, "l1_mirror");
                assert!((from[0] - (-2.0)).abs() < LENGTH_TOLERANCE);
                assert!((from[1] - 0.0).abs() < LENGTH_TOLERANCE);
                assert!((to[0] - (-2.0)).abs() < LENGTH_TOLERANCE);
                assert!((to[1] - 3.0).abs() < LENGTH_TOLERANCE);
            }
            _ => panic!("expected mirrored Line"),
        }
    }

    /// T03: Circle mirror across x-axis.
    #[test]
    fn t03_normal_circle_mirror() {
        let profile = vec![SketchElement::Circle {
            id: "c1".to_string(),
            center: [3.0, 4.0],
            radius: 2.0,
        }];
        let out = apply_sketch_mirror(&profile, [0.0, 0.0], [1.0, 0.0], &[]).unwrap();
        assert_eq!(out.len(), 2);
        match &out[1] {
            SketchElement::Circle { id, center, radius } => {
                assert_eq!(id, "c1_mirror");
                assert!((center[0] - 3.0).abs() < LENGTH_TOLERANCE);
                assert!((center[1] - (-4.0)).abs() < LENGTH_TOLERANCE);
                assert!((radius - 2.0).abs() < LENGTH_TOLERANCE);
            }
            _ => panic!("expected mirrored Circle"),
        }
    }

    /// T04: Arc mirror across x-axis. STEP 3.5 Codex R01修正: new_end = new_start - sweep.
    #[test]
    fn t04_normal_arc_mirror() {
        let profile = vec![SketchElement::Arc {
            id: "a1".to_string(),
            center: [0.0, 0.0],
            radius: 5.0,
            start_angle: 0.0,
            end_angle: FRAC_PI_2,
        }];
        let out = apply_sketch_mirror(&profile, [0.0, 0.0], [1.0, 0.0], &[]).unwrap();
        assert_eq!(out.len(), 2);
        match &out[1] {
            SketchElement::Arc {
                id,
                center,
                radius,
                start_angle,
                end_angle,
            } => {
                assert_eq!(id, "a1_mirror");
                assert!((center[0] - 0.0).abs() < LENGTH_TOLERANCE);
                assert!((center[1] - 0.0).abs() < LENGTH_TOLERANCE);
                assert!((radius - 5.0).abs() < LENGTH_TOLERANCE);
                assert!((start_angle - 0.0).abs() < LENGTH_TOLERANCE);
                assert!((end_angle - (-FRAC_PI_2)).abs() < LENGTH_TOLERANCE);
            }
            _ => panic!("expected mirrored Arc"),
        }
    }

    /// T05: Empty selection = all elements mirrored.
    #[test]
    fn t05_empty_selection_mirrors_all() {
        let profile = vec![
            SketchElement::Line {
                id: "l1".to_string(),
                from: [2.0, 0.0],
                to: [2.0, 3.0],
            },
            SketchElement::Circle {
                id: "c1".to_string(),
                center: [3.0, 4.0],
                radius: 2.0,
            },
            SketchElement::Arc {
                id: "a1".to_string(),
                center: [0.0, 0.0],
                radius: 5.0,
                start_angle: 0.0,
                end_angle: FRAC_PI_2,
            },
        ];
        let out = apply_sketch_mirror(&profile, [0.0, 0.0], [0.0, 1.0], &[]).unwrap();
        assert_eq!(out.len(), profile.len() * 2);
    }

    /// 100-run determinism.
    #[test]
    fn t_determinism_100_runs() {
        let profile = vec![
            SketchElement::Line {
                id: "l1".to_string(),
                from: [2.0, 0.0],
                to: [2.0, 3.0],
            },
            SketchElement::Circle {
                id: "c1".to_string(),
                center: [3.0, 4.0],
                radius: 2.0,
            },
        ];
        let reference = apply_sketch_mirror(&profile, [0.0, 0.0], [0.0, 1.0], &[]).unwrap();
        for _ in 0..100 {
            let out = apply_sketch_mirror(&profile, [0.0, 0.0], [0.0, 1.0], &[]).unwrap();
            assert_eq!(out, reference);
        }
    }

    /// T_DEG_mirror_on_axis (Line): line lies on the x-axis → reflection coincides with itself.
    #[test]
    fn t_deg_mirror_on_axis_line() {
        let profile = vec![SketchElement::Line {
            id: "l1".to_string(),
            from: [0.0, 0.0],
            to: [5.0, 0.0],
        }];
        let err = apply_sketch_mirror(&profile, [0.0, 0.0], [1.0, 0.0], &[]).unwrap_err();
        assert!(matches!(
            err,
            KernelError::DegenerateSketchElement {
                element_id,
                reason: "mirror_axis_coincident",
            } if element_id == "l1"
        ));
    }

    /// T_DEG_mirror_on_axis_arc: symmetric Arc across x-axis → reflection coincides with itself.
    #[test]
    fn t_deg_mirror_on_axis_arc() {
        let profile = vec![SketchElement::Arc {
            id: "a1".to_string(),
            center: [0.0, 0.0],
            radius: 1.0,
            start_angle: -FRAC_PI_4,
            end_angle: FRAC_PI_4,
        }];
        let err = apply_sketch_mirror(&profile, [0.0, 0.0], [1.0, 0.0], &[]).unwrap_err();
        assert!(matches!(
            err,
            KernelError::DegenerateSketchElement {
                reason: "mirror_axis_coincident",
                ..
            }
        ));
    }

    /// T_boundary_arc_endpoints_on_axis: half-circle endpoints on x-axis. Reflection does NOT
    /// coincide with itself (sweep inverts but endpoints land on the same axis points via a
    /// different arc center path) — must NOT be rejected.
    #[test]
    fn t_boundary_arc_endpoints_on_axis() {
        let profile = vec![SketchElement::Arc {
            id: "a1".to_string(),
            center: [0.0, 0.0],
            radius: 1.0,
            start_angle: 0.0,
            end_angle: PI,
        }];
        let out = apply_sketch_mirror(&profile, [0.0, 0.0], [1.0, 0.0], &[]).unwrap();
        assert_eq!(out.len(), 2);
        match &out[1] {
            SketchElement::Arc {
                start_angle,
                end_angle,
                ..
            } => {
                assert!((start_angle - 0.0).abs() < LENGTH_TOLERANCE);
                assert!((end_angle - (-PI)).abs() < LENGTH_TOLERANCE);
            }
            _ => panic!("expected mirrored Arc"),
        }
    }

    /// T_DEG_axis_degenerate (same point).
    #[test]
    fn t_deg_axis_degenerate_same_point() {
        let profile = vec![SketchElement::Line {
            id: "l1".to_string(),
            from: [2.0, 0.0],
            to: [2.0, 3.0],
        }];
        let err = apply_sketch_mirror(&profile, [0.0, 0.0], [0.0, 0.0], &[]).unwrap_err();
        assert!(matches!(
            err,
            KernelError::InvalidParameter {
                kind: "sketch_mirror_axis_degenerate"
            }
        ));
    }

    /// T_DEG_axis_degenerate (NaN).
    #[test]
    fn t_deg_axis_degenerate_nan() {
        let profile = vec![SketchElement::Line {
            id: "l1".to_string(),
            from: [2.0, 0.0],
            to: [2.0, 3.0],
        }];
        let err = apply_sketch_mirror(&profile, [f64::NAN, 0.0], [0.0, 1.0], &[]).unwrap_err();
        assert!(matches!(
            err,
            KernelError::InvalidParameter {
                kind: "sketch_mirror_axis_degenerate"
            }
        ));
    }

    /// T_DEG_axis_degenerate (Inf).
    #[test]
    fn t_deg_axis_degenerate_inf() {
        let profile = vec![SketchElement::Line {
            id: "l1".to_string(),
            from: [2.0, 0.0],
            to: [2.0, 3.0],
        }];
        let err = apply_sketch_mirror(&profile, [0.0, 0.0], [f64::INFINITY, 0.0], &[]).unwrap_err();
        assert!(matches!(
            err,
            KernelError::InvalidParameter {
                kind: "sketch_mirror_axis_degenerate"
            }
        ));
    }

    /// T_DEG_unknown_element: selection id not in profile.
    #[test]
    fn t_deg_unknown_element() {
        let profile = vec![SketchElement::Line {
            id: "l1".to_string(),
            from: [2.0, 0.0],
            to: [2.0, 3.0],
        }];
        let err = apply_sketch_mirror(&profile, [0.0, 0.0], [0.0, 1.0], &["missing".to_string()])
            .unwrap_err();
        assert!(matches!(
            err,
            KernelError::InvalidParameter {
                kind: "sketch_mirror_unknown_element_id"
            }
        ));
    }

    /// T_DEG_unsupported_type (Ellipse).
    #[test]
    fn t_deg_unsupported_type_ellipse() {
        let profile = vec![SketchElement::Ellipse {
            id: "e1".to_string(),
            center: [0.0, 0.0],
            major: 2.0,
            minor: 1.0,
            rotation: 0.0,
        }];
        let err =
            apply_sketch_mirror(&profile, [0.0, 0.0], [0.0, 1.0], &["e1".to_string()]).unwrap_err();
        assert!(matches!(
            err,
            KernelError::UnsupportedFeature {
                kind: "sketch_mirror_of_ellipse_or_conic"
            }
        ));
    }

    /// T_DEG_unsupported_type (Conic).
    #[test]
    fn t_deg_unsupported_type_conic() {
        let profile = vec![SketchElement::Conic {
            id: "cn1".to_string(),
            coeffs: [1.0, 0.0, 1.0, 0.0, 0.0],
        }];
        let err = apply_sketch_mirror(&profile, [0.0, 0.0], [0.0, 1.0], &["cn1".to_string()])
            .unwrap_err();
        assert!(matches!(
            err,
            KernelError::UnsupportedFeature {
                kind: "sketch_mirror_of_ellipse_or_conic"
            }
        ));
    }

    /// T_DEG_duplicate_id: derived "{elem_id}_mirror" already present in profile.
    #[test]
    fn t_deg_duplicate_id() {
        let profile = vec![
            SketchElement::Line {
                id: "l1".to_string(),
                from: [2.0, 0.0],
                to: [2.0, 3.0],
            },
            SketchElement::Line {
                id: "l1_mirror".to_string(),
                from: [-2.0, 0.0],
                to: [-2.0, 3.0],
            },
        ];
        let err =
            apply_sketch_mirror(&profile, [0.0, 0.0], [0.0, 1.0], &["l1".to_string()]).unwrap_err();
        assert!(matches!(
            err,
            KernelError::DegenerateSketchElement {
                element_id,
                reason: "mirror_duplicate_id",
            } if element_id == "l1_mirror"
        ));
    }

    /// Item 3 regression: large-coordinate axis-coincident Arc must be detected.
    /// The atan2-based formula lost precision when |center| >> radius; direct 2φ - θ
    /// computation does not.
    #[test]
    fn t_item3_large_coord_axis_coincident_arc() {
        // Axis 45° through origin: direction (1,1)/√2, φ = π/4.
        // Arc center=(1e6, 1e6), radius=1e-3, start/end chosen symmetric about the axis
        // (axis bisects the sweep). φ - start = end - φ ⇒ start = φ - δ, end = φ + δ.
        let phi = std::f64::consts::FRAC_PI_4;
        let delta = 0.1_f64; // small symmetric arc
        let profile = vec![SketchElement::Arc {
            id: "a1".to_string(),
            center: [1.0e6, 1.0e6],
            radius: 1.0e-3,
            start_angle: phi - delta,
            end_angle: phi + delta,
        }];
        let err = apply_sketch_mirror(&profile, [0.0, 0.0], [1.0, 1.0], &[]).unwrap_err();
        assert!(matches!(
            err,
            KernelError::DegenerateSketchElement {
                element_id,
                reason: "mirror_axis_coincident",
            } if element_id == "a1"
        ));
    }

    /// Item 4 regression: multi-turn Arc (|sweep| >= 2π) with center on axis must be
    /// detected as self-coincident (point set covers the full circle).
    #[test]
    fn t_item4_multiturn_arc_on_axis_coincident() {
        // 2.5π sweep centered at origin; x-axis mirror → center stays at origin → self.
        let profile = vec![SketchElement::Arc {
            id: "a1".to_string(),
            center: [0.0, 0.0],
            radius: 1.0,
            start_angle: 0.0,
            end_angle: 2.5 * PI,
        }];
        let err = apply_sketch_mirror(&profile, [0.0, 0.0], [1.0, 0.0], &[]).unwrap_err();
        assert!(matches!(
            err,
            KernelError::DegenerateSketchElement {
                reason: "mirror_axis_coincident",
                ..
            }
        ));
    }

    /// Item 5 regression: very large axis coordinates (1e300) must not overflow.
    /// axis=(0,0)-(1e300,0) is mathematically the x-axis. Mirror of Line (2,0)-(2,3)
    /// must produce (2,0)-(2,-3), NOT the point-symmetry result (-2,0)-(-2,-3).
    #[test]
    fn t_item5_large_axis_no_overflow() {
        let profile = vec![SketchElement::Line {
            id: "l1".to_string(),
            from: [2.0, 0.0],
            to: [2.0, 3.0],
        }];
        let out = apply_sketch_mirror(&profile, [0.0, 0.0], [1.0e300, 0.0], &[]).unwrap();
        assert_eq!(out.len(), 2);
        match &out[1] {
            SketchElement::Line { id, from, to } => {
                assert_eq!(id, "l1_mirror");
                // The x-axis preserves x and flips y sign. If overflow occurred, the
                // reflection would degenerate to point-symmetry around axis_p1=(0,0),
                // producing from=(-2,0), to=(-2,-3).
                assert!(
                    (from[0] - 2.0).abs() < LENGTH_TOLERANCE,
                    "from[0]={}",
                    from[0]
                );
                assert!(
                    (from[1] - 0.0).abs() < LENGTH_TOLERANCE,
                    "from[1]={}",
                    from[1]
                );
                assert!((to[0] - 2.0).abs() < LENGTH_TOLERANCE, "to[0]={}", to[0]);
                assert!((to[1] - (-3.0)).abs() < LENGTH_TOLERANCE, "to[1]={}", to[1]);
            }
            _ => panic!("expected mirrored Line"),
        }
    }
}
