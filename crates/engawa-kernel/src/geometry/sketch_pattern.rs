//! 2D sketch pattern: equally-spaced duplication of selected elements.
//!
//! Phase 10 (#299): Line/Circle/Arc only. Ellipse/Conic are Out-of-Scope (matches
//! SketchOffset/SketchMirror's exact same exclusion — closed-form duplication would still
//! require iterative parameter re-fitting for trim/segmentation downstream).
//!
//! Patterned duplicates are **appended** to the profile (originals are preserved, not
//! modified), and receive deterministic ids `"{elem_id}_pattern_{linear|circular}_{k}"`
//! where k ∈ 1..count is the 1-indexed copy number. `count` is the TOTAL instance count
//! INCLUDING the original (`count=0` is an error). At `count=1` no copies are produced and
//! `selection` is not consulted at all, but the parameter checks that precede the count
//! branch (finite/degenerate `direction`, `center`, `total_angle`) still apply. Input
//! order of `selection` does not affect the result — elements are processed in source-array
//! order (matches SketchMirror).
//!
//! # Linear
//! - `direction` is normalised internally to a unit vector; `distance` is the sole
//!   spacing parameter (offset of copy k = unit(direction) * distance * k).
//! - Translation preserves angles and radii.
//!
//! # Circular
//! - Angular step = `total_angle / count` ("equal spacing" convention; avoids the
//!   division-by-zero that `total_angle / (count-1)` would hit at count=1).
//! - Rotation by `angle_k = step * k` (CCW positive). Rotating an Arc about an external
//!   centre adds `angle_k` to both its `start_angle` and `end_angle` (rigid rotation
//!   preserves sweep sign and magnitude, so no atan2 recomputation needed).
//!
//! # Errors
//! - `InvalidParameter { kind: "sketch_pattern_linear_direction_degenerate" }` — direction
//!   contains NaN/Inf or `|direction| <= LENGTH_TOLERANCE`
//! - `InvalidParameter { kind: "sketch_pattern_linear_distance_invalid" }` — distance
//!   is NaN/Inf
//! - `InvalidParameter { kind: "sketch_pattern_linear_count_zero" }` — count == 0
//! - `InvalidParameter { kind: "sketch_pattern_linear_count_too_large" }` — count exceeds
//!   `MAX_PATTERN_COUNT` (resource guard against a mistyped count)
//! - `InvalidParameter { kind: "sketch_pattern_linear_distance_zero" }` — count >= 2
//!   and `|distance| <= LENGTH_TOLERANCE` (every copy would land on the original)
//! - `InvalidParameter { kind: "sketch_pattern_linear_unknown_element_id" }` — selection
//!   id not in profile
//! - `UnsupportedFeature { kind: "sketch_pattern_linear_of_ellipse_or_conic" }` — Ellipse
//!   or Conic in selection
//! - `DegenerateSketchElement { reason: "pattern_linear_duplicate_id" }` — derived id
//!   `"{elem_id}_pattern_linear_{k}"` already present in profile
//! - Mirror-image errors exist for Circular with `circular` in the kind string and
//!   `center_degenerate` / `angle_invalid` / `angle_degenerate` / `count_too_large` as
//!   the degeneracy modes.

use crate::error::KernelError;
use crate::geometry::math::{ANGLE_TOLERANCE, LENGTH_TOLERANCE};
use engawa_format::SketchElement;
use std::collections::BTreeSet;

/// Upper bound on the TOTAL instance count of a single pattern feature.
///
/// `count` comes straight from the `.engawa` document, so a single digit slip
/// (`count: 30000000`) would otherwise allocate tens of millions of `SketchElement`s and
/// OOM-kill the process. 10_000 instances of one element is already far beyond any
/// plausible 2D sketch, so the cap only ever fires on input errors. Note the total output
/// size is `selected_elements * count`, i.e. the cap bounds the multiplier, not the profile.
pub const MAX_PATTERN_COUNT: u32 = 10_000;

/// Apply a linear pattern (equally-spaced translation duplication) to selected elements.
///
/// Kernel-level pure function. `count` is the TOTAL instance count including the original
/// (count=1 is a true no-op, count=0 is an error). Source elements are preserved unchanged;
/// translated duplicates are appended in source-array order with ids
/// `"{elem_id}_pattern_linear_{k}"` for k = 1..count.
///
/// - `selection`: element IDs to pattern; empty = all elements
/// - `direction`: pattern direction (internally normalised to a unit vector)
/// - `distance`: spacing between consecutive instances along `direction`
/// - Returns: new profile with translated duplicates appended
pub fn apply_sketch_pattern_linear(
    source: &[SketchElement],
    selection: &[String],
    count: u32,
    direction: [f64; 2],
    distance: f64,
) -> Result<Vec<SketchElement>, KernelError> {
    if !is_finite_point(&direction) {
        return Err(KernelError::InvalidParameter {
            kind: "sketch_pattern_linear_direction_degenerate",
        });
    }
    let dir_len = direction[0].hypot(direction[1]);
    if dir_len <= LENGTH_TOLERANCE {
        return Err(KernelError::InvalidParameter {
            kind: "sketch_pattern_linear_direction_degenerate",
        });
    }
    if !distance.is_finite() {
        return Err(KernelError::InvalidParameter {
            kind: "sketch_pattern_linear_distance_invalid",
        });
    }
    if count == 0 {
        return Err(KernelError::InvalidParameter {
            kind: "sketch_pattern_linear_count_zero",
        });
    }
    if count > MAX_PATTERN_COUNT {
        return Err(KernelError::InvalidParameter {
            kind: "sketch_pattern_linear_count_too_large",
        });
    }
    if count == 1 {
        // True no-op — selection contents are not consulted.
        return Ok(source.to_vec());
    }
    if distance.abs() <= LENGTH_TOLERANCE {
        // count >= 2: every copy would coincide with the original.
        return Err(KernelError::InvalidParameter {
            kind: "sketch_pattern_linear_distance_zero",
        });
    }
    let unit_dir = [direction[0] / dir_len, direction[1] / dir_len];

    let selected: BTreeSet<&str> = resolve_selection(source, selection, "linear")?;
    let existing_ids: BTreeSet<&str> = source.iter().map(element_id).collect();

    let mut appended: Vec<SketchElement> = Vec::new();
    for elem in source {
        let eid = element_id(elem);
        if !selected.contains(eid) {
            continue;
        }
        for k in 1..count {
            let offset = scale(&unit_dir, distance * k as f64);
            let translated = translate_element(elem, &offset)?;
            let derived_id = format!("{eid}_pattern_linear_{k}");
            if existing_ids.contains(derived_id.as_str()) {
                return Err(KernelError::DegenerateSketchElement {
                    element_id: derived_id,
                    reason: "pattern_linear_duplicate_id",
                });
            }
            appended.push(rename_to(&translated, &derived_id));
        }
    }

    let mut out = source.to_vec();
    out.append(&mut appended);
    Ok(out)
}

/// Apply a circular pattern (equally-spaced rotation duplication) to selected elements.
///
/// Kernel-level pure function. `count` is the TOTAL instance count including the original
/// (count=1 is a true no-op, count=0 is an error). Angular step = `total_angle / count`
/// (avoids division-by-zero at count=1). Source elements are preserved unchanged; rotated
/// duplicates are appended in source-array order with ids `"{elem_id}_pattern_circular_{k}"`
/// for k = 1..count.
///
/// - `selection`: element IDs to pattern; empty = all elements
/// - `center`: rotation centre
/// - `total_angle`: total angular span in radians; step = total_angle / count
/// - Returns: new profile with rotated duplicates appended
pub fn apply_sketch_pattern_circular(
    source: &[SketchElement],
    selection: &[String],
    center: [f64; 2],
    count: u32,
    total_angle: f64,
) -> Result<Vec<SketchElement>, KernelError> {
    if !is_finite_point(&center) {
        return Err(KernelError::InvalidParameter {
            kind: "sketch_pattern_circular_center_degenerate",
        });
    }
    if !total_angle.is_finite() {
        return Err(KernelError::InvalidParameter {
            kind: "sketch_pattern_circular_angle_invalid",
        });
    }
    if count == 0 {
        return Err(KernelError::InvalidParameter {
            kind: "sketch_pattern_circular_count_zero",
        });
    }
    if count > MAX_PATTERN_COUNT {
        return Err(KernelError::InvalidParameter {
            kind: "sketch_pattern_circular_count_too_large",
        });
    }
    if count == 1 {
        return Ok(source.to_vec());
    }
    let step = total_angle / count as f64;
    if step.abs() <= ANGLE_TOLERANCE {
        // count >= 2: every copy would coincide with the original.
        return Err(KernelError::InvalidParameter {
            kind: "sketch_pattern_circular_angle_degenerate",
        });
    }

    let selected: BTreeSet<&str> = resolve_selection(source, selection, "circular")?;
    let existing_ids: BTreeSet<&str> = source.iter().map(element_id).collect();

    let mut appended: Vec<SketchElement> = Vec::new();
    for elem in source {
        let eid = element_id(elem);
        if !selected.contains(eid) {
            continue;
        }
        for k in 1..count {
            let angle = step * k as f64;
            let rotated = rotate_element(elem, &center, angle)?;
            let derived_id = format!("{eid}_pattern_circular_{k}");
            if existing_ids.contains(derived_id.as_str()) {
                return Err(KernelError::DegenerateSketchElement {
                    element_id: derived_id,
                    reason: "pattern_circular_duplicate_id",
                });
            }
            appended.push(rename_to(&rotated, &derived_id));
        }
    }

    let mut out = source.to_vec();
    out.append(&mut appended);
    Ok(out)
}

/// Resolve `selection` against `source`. Empty selection = all element ids. Unknown id
/// returns an error with the appropriate per-mode kind. Iteration order of the returned
/// set is independent of selection input order (BTreeSet is sorted).
fn resolve_selection<'a>(
    source: &'a [SketchElement],
    selection: &'a [String],
    mode: &'static str,
) -> Result<BTreeSet<&'a str>, KernelError> {
    if selection.is_empty() {
        Ok(source.iter().map(element_id).collect())
    } else {
        let mut set: BTreeSet<&str> = BTreeSet::new();
        for id in selection {
            if !source.iter().any(|e| element_id(e) == id.as_str()) {
                return Err(KernelError::InvalidParameter {
                    kind: match mode {
                        "circular" => "sketch_pattern_circular_unknown_element_id",
                        _ => "sketch_pattern_linear_unknown_element_id",
                    },
                });
            }
            set.insert(id.as_str());
        }
        Ok(set)
    }
}

/// Translate a single sketch element by `offset`.
fn translate_element(
    elem: &SketchElement,
    offset: &[f64; 2],
) -> Result<SketchElement, KernelError> {
    match elem {
        SketchElement::Line { id, from, to } => Ok(SketchElement::Line {
            id: id.clone(),
            from: add(from, offset),
            to: add(to, offset),
        }),
        SketchElement::Circle { id, center, radius } => Ok(SketchElement::Circle {
            id: id.clone(),
            center: add(center, offset),
            radius: *radius,
        }),
        SketchElement::Arc {
            id,
            center,
            radius,
            start_angle,
            end_angle,
        } => Ok(SketchElement::Arc {
            id: id.clone(),
            center: add(center, offset),
            radius: *radius,
            start_angle: *start_angle,
            end_angle: *end_angle,
        }),
        SketchElement::Ellipse { .. } | SketchElement::Conic { .. } => {
            Err(KernelError::UnsupportedFeature {
                kind: "sketch_pattern_linear_of_ellipse_or_conic",
            })
        }
    }
}

/// Rotate a single sketch element around `center` by `angle` (CCW positive, radians).
fn rotate_element(
    elem: &SketchElement,
    center: &[f64; 2],
    angle: f64,
) -> Result<SketchElement, KernelError> {
    let (cos_a, sin_a) = (angle.cos(), angle.sin());
    match elem {
        SketchElement::Line { id, from, to } => Ok(SketchElement::Line {
            id: id.clone(),
            from: rotate_point(from, center, cos_a, sin_a),
            to: rotate_point(to, center, cos_a, sin_a),
        }),
        SketchElement::Circle {
            id,
            center: c,
            radius,
        } => Ok(SketchElement::Circle {
            id: id.clone(),
            center: rotate_point(c, center, cos_a, sin_a),
            radius: *radius,
        }),
        SketchElement::Arc {
            id,
            center: c,
            radius,
            start_angle,
            end_angle,
        } => {
            // Rotation is a rigid orientation-preserving transform: sweep sign and magnitude
            // are invariant. Both start/end shift by the same `angle`. (Contrast with Mirror,
            // where reflection inverts the sweep and requires sign-reversal.)
            Ok(SketchElement::Arc {
                id: id.clone(),
                center: rotate_point(c, center, cos_a, sin_a),
                radius: *radius,
                start_angle: start_angle + angle,
                end_angle: end_angle + angle,
            })
        }
        SketchElement::Ellipse { .. } | SketchElement::Conic { .. } => {
            Err(KernelError::UnsupportedFeature {
                kind: "sketch_pattern_circular_of_ellipse_or_conic",
            })
        }
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
            unreachable!("translate/rotate reject Ellipse/Conic before rename")
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

fn add(a: &[f64; 2], b: &[f64; 2]) -> [f64; 2] {
    [a[0] + b[0], a[1] + b[1]]
}

fn scale(a: &[f64; 2], s: f64) -> [f64; 2] {
    [a[0] * s, a[1] * s]
}

/// Rotate point `q` around `center` given precomputed (cos, sin) of the rotation angle.
fn rotate_point(q: &[f64; 2], center: &[f64; 2], cos_a: f64, sin_a: f64) -> [f64; 2] {
    let vx = q[0] - center[0];
    let vy = q[1] - center[1];
    [
        center[0] + vx * cos_a - vy * sin_a,
        center[1] + vx * sin_a + vy * cos_a,
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::f64::consts::{FRAC_PI_2, PI, TAU};

    /// T01: Determinism — same input produces same output.
    #[test]
    fn t01_determinism() {
        let profile = vec![
            SketchElement::Line {
                id: "l1".to_string(),
                from: [0.0, 0.0],
                to: [2.0, 0.0],
            },
            SketchElement::Circle {
                id: "c1".to_string(),
                center: [3.0, 0.0],
                radius: 1.0,
            },
            SketchElement::Arc {
                id: "a1".to_string(),
                center: [0.0, 0.0],
                radius: 5.0,
                start_angle: 0.0,
                end_angle: FRAC_PI_2,
            },
        ];
        let lin1 = apply_sketch_pattern_linear(&profile, &[], 3, [1.0, 0.0], 3.0).unwrap();
        let lin2 = apply_sketch_pattern_linear(&profile, &[], 3, [1.0, 0.0], 3.0).unwrap();
        assert_eq!(lin1, lin2);

        let cir1 = apply_sketch_pattern_circular(&profile, &[], [0.0, 0.0], 4, TAU).unwrap();
        let cir2 = apply_sketch_pattern_circular(&profile, &[], [0.0, 0.0], 4, TAU).unwrap();
        assert_eq!(cir1, cir2);
    }

    /// T02: Linear pattern on Line. `direction=(1,0), distance=3, count=3` produces two
    /// copies at offsets +3 and +6 along x.
    #[test]
    fn t02_linear_line_normal() {
        let profile = vec![SketchElement::Line {
            id: "l1".to_string(),
            from: [0.0, 0.0],
            to: [2.0, 0.0],
        }];
        let out = apply_sketch_pattern_linear(&profile, &[], 3, [1.0, 0.0], 3.0).unwrap();
        assert_eq!(out.len(), 3);
        // Original unchanged.
        match &out[0] {
            SketchElement::Line { id, from, to } => {
                assert_eq!(id, "l1");
                assert_eq!(*from, [0.0, 0.0]);
                assert_eq!(*to, [2.0, 0.0]);
            }
            _ => panic!("expected original Line"),
        }
        // Copy 1: (3,0)-(5,0), id "l1_pattern_linear_1".
        match &out[1] {
            SketchElement::Line { id, from, to } => {
                assert_eq!(id, "l1_pattern_linear_1");
                assert!((from[0] - 3.0).abs() < LENGTH_TOLERANCE);
                assert!((from[1] - 0.0).abs() < LENGTH_TOLERANCE);
                assert!((to[0] - 5.0).abs() < LENGTH_TOLERANCE);
                assert!((to[1] - 0.0).abs() < LENGTH_TOLERANCE);
            }
            _ => panic!("expected copy-1 Line"),
        }
        // Copy 2: (6,0)-(8,0), id "l1_pattern_linear_2".
        match &out[2] {
            SketchElement::Line { id, from, to } => {
                assert_eq!(id, "l1_pattern_linear_2");
                assert!((from[0] - 6.0).abs() < LENGTH_TOLERANCE);
                assert!((from[1] - 0.0).abs() < LENGTH_TOLERANCE);
                assert!((to[0] - 8.0).abs() < LENGTH_TOLERANCE);
                assert!((to[1] - 0.0).abs() < LENGTH_TOLERANCE);
            }
            _ => panic!("expected copy-2 Line"),
        }
    }

    /// T03: Circular pattern on Circle. `center=(0,0), total_angle=2π, count=4` step=π/2,
    /// so copies k=1,2,3 land at 90°, 180°, 270° around origin. Radius unchanged.
    #[test]
    fn t03_circular_circle_normal() {
        let profile = vec![SketchElement::Circle {
            id: "c1".to_string(),
            center: [3.0, 0.0],
            radius: 1.0,
        }];
        let out = apply_sketch_pattern_circular(&profile, &[], [0.0, 0.0], 4, TAU).unwrap();
        assert_eq!(out.len(), 4);
        // k=1: 90° rotation of (3,0) → (0,3)
        match &out[1] {
            SketchElement::Circle { id, center, radius } => {
                assert_eq!(id, "c1_pattern_circular_1");
                assert!((center[0] - 0.0).abs() < LENGTH_TOLERANCE);
                assert!((center[1] - 3.0).abs() < LENGTH_TOLERANCE);
                assert!((radius - 1.0).abs() < LENGTH_TOLERANCE);
            }
            _ => panic!("expected copy-1 Circle"),
        }
        // k=2: 180° → (-3,0)
        match &out[2] {
            SketchElement::Circle { id, center, .. } => {
                assert_eq!(id, "c1_pattern_circular_2");
                assert!((center[0] - (-3.0)).abs() < LENGTH_TOLERANCE);
                assert!((center[1] - 0.0).abs() < LENGTH_TOLERANCE);
            }
            _ => panic!("expected copy-2 Circle"),
        }
        // k=3: 270° → (0,-3)
        match &out[3] {
            SketchElement::Circle { id, center, .. } => {
                assert_eq!(id, "c1_pattern_circular_3");
                assert!((center[0] - 0.0).abs() < LENGTH_TOLERANCE);
                assert!((center[1] - (-3.0)).abs() < LENGTH_TOLERANCE);
            }
            _ => panic!("expected copy-3 Circle"),
        }
    }

    /// T04: Circular pattern on Arc — verifies the numeric model (sweep preservation via
    /// equal shift of start/end). `step = total_angle / count = π/2`, so only k=1 copy.
    #[test]
    fn t04_circular_arc_numeric_model() {
        let profile = vec![SketchElement::Arc {
            id: "a1".to_string(),
            center: [2.0, 0.0],
            radius: 1.0,
            start_angle: 0.0,
            end_angle: FRAC_PI_2,
        }];
        let out = apply_sketch_pattern_circular(&profile, &[], [0.0, 0.0], 2, PI).unwrap();
        assert_eq!(out.len(), 2);
        match &out[1] {
            SketchElement::Arc {
                id,
                center,
                radius,
                start_angle,
                end_angle,
            } => {
                assert_eq!(id, "a1_pattern_circular_1");
                // 90° rotation of (2,0) around origin → (0,2)
                assert!((center[0] - 0.0).abs() < LENGTH_TOLERANCE);
                assert!((center[1] - 2.0).abs() < LENGTH_TOLERANCE);
                assert!((radius - 1.0).abs() < LENGTH_TOLERANCE);
                // start = 0 + π/2; end = π/2 + π/2 = π
                assert!((start_angle - FRAC_PI_2).abs() < LENGTH_TOLERANCE);
                assert!((end_angle - PI).abs() < LENGTH_TOLERANCE);
            }
            _ => panic!("expected copy-1 Arc"),
        }
    }

    /// T05: Empty selection = all elements patterned (both Linear and Circular).
    #[test]
    fn t05_empty_selection_all() {
        let profile = vec![
            SketchElement::Line {
                id: "l1".to_string(),
                from: [0.0, 0.0],
                to: [2.0, 0.0],
            },
            SketchElement::Circle {
                id: "c1".to_string(),
                center: [3.0, 0.0],
                radius: 1.0,
            },
            SketchElement::Arc {
                id: "a1".to_string(),
                center: [0.0, 0.0],
                radius: 5.0,
                start_angle: 0.0,
                end_angle: FRAC_PI_2,
            },
        ];
        let lin = apply_sketch_pattern_linear(&profile, &[], 3, [1.0, 0.0], 3.0).unwrap();
        assert_eq!(lin.len(), profile.len() * 3);
        let cir = apply_sketch_pattern_circular(&profile, &[], [0.0, 0.0], 3, TAU).unwrap();
        assert_eq!(cir.len(), profile.len() * 3);
    }

    /// 100-run determinism for both modes.
    #[test]
    fn t_determinism_100_runs() {
        let profile = vec![
            SketchElement::Line {
                id: "l1".to_string(),
                from: [0.0, 0.0],
                to: [2.0, 0.0],
            },
            SketchElement::Circle {
                id: "c1".to_string(),
                center: [3.0, 0.0],
                radius: 1.0,
            },
        ];
        let lin_ref = apply_sketch_pattern_linear(&profile, &[], 3, [1.0, 0.0], 3.0).unwrap();
        let cir_ref = apply_sketch_pattern_circular(&profile, &[], [0.0, 0.0], 4, TAU).unwrap();
        for _ in 0..100 {
            let lin = apply_sketch_pattern_linear(&profile, &[], 3, [1.0, 0.0], 3.0).unwrap();
            assert_eq!(lin, lin_ref);
            let cir = apply_sketch_pattern_circular(&profile, &[], [0.0, 0.0], 4, TAU).unwrap();
            assert_eq!(cir, cir_ref);
        }
    }
}
