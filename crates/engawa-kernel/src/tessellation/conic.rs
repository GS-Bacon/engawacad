//! Canonical axes and domain checks for the five-coefficient Conic of ADR-017.

use crate::{KernelError, EPS_DISCRIMINANT, RELATIVE_TOLERANCE};

pub(super) struct Conic {
    center: [f64; 2],
    axis: [f64; 2],
    linear_scale: f64,
    quadratic_scale: f64,
    radii: [f64; 2],
    // Negative principal coefficient ratio places sinh along that axis.
    hyperbolic_axis: Option<usize>,
}

impl Conic {
    pub(super) fn from_coeffs(id: &str, coeffs: [f64; 5]) -> Result<Self, KernelError> {
        let invalid = |reason| KernelError::DegenerateSketchElement {
            element_id: id.into(),
            reason,
        };
        if !coeffs.iter().all(|c| c.is_finite()) {
            return Err(invalid("non-finite coeff"));
        }

        // Scale the quadratic part first. Delta now compares
        // axis conditioning independently of the overall size of the sketch.
        let [a, b, c, d, e] = coeffs;
        let scale = a.abs().max(b.abs()).max(c.abs());
        if scale == 0.0 {
            return Err(invalid("no quadratic term"));
        }
        let (a, b, c) = (a / scale, b / scale, c / scale);
        let delta = b * b - 4.0 * a * c;
        let roundoff = 8.0 * f64::EPSILON * (b * b + (4.0 * a * c).abs());
        if delta.abs() <= roundoff {
            // A row of the quadratic matrix is normal to its null direction.
            // Choose the longer row, then take its cross product with (D, E).
            let (nx, ny) = if a.abs() >= c.abs() {
                (a, b * 0.5)
            } else {
                (b * 0.5, c)
            };
            let linear_scale = d.abs().max(e.abs());
            if linear_scale > 0.0 {
                let p = nx * (e / linear_scale);
                let q = ny * (d / linear_scale);
                if (p - q).abs() > 8.0 * f64::EPSILON * (p.abs() + q.abs()) {
                    return Err(KernelError::UnsupportedFeature {
                        kind: "conic_parabola",
                    });
                }
            }
            return Err(invalid("degenerate rank-one conic"));
        }
        if delta.abs() < EPS_DISCRIMINANT {
            return Err(KernelError::UnsupportedFeature {
                kind: "conic_near_parabolic",
            });
        }

        // In coordinates (u,v) = (x,y) * scale / linear_scale both the
        // quadratic and linear coefficients are bounded by one. In particular,
        // translating x²+y²+1e200*x−1 no longer squares a 1e200 coefficient.
        let linear_scale = d.abs().max(e.abs()).max(scale.sqrt());
        let (d, e) = (d / linear_scale, e / linear_scale);
        let det = -delta * 0.25;
        let u = (b * 0.25 * e - c * 0.5 * d) / det;
        let v = (b * 0.25 * d - a * 0.5 * e) / det;
        let du = d * 0.5 * u;
        let ev = e * 0.5 * v;
        // The constant is -(sqrt(scale)/linear_scale)². Compare its square
        // root with the translation terms before squaring: in xy+1e200*x−1
        // both translation terms vanish but the original −1 still matters.
        let constant_root = scale.sqrt() / linear_scale;
        let magnitude = constant_root.max(du.abs().sqrt()).max(ev.abs().sqrt());
        if magnitude == 0.0 {
            return Err(invalid("non-finite canonical form"));
        }
        let fp = -(constant_root / magnitude).powi(2)
            + (du / magnitude) / magnitude
            + (ev / magnitude) / magnitude;
        if fp.abs() <= RELATIVE_TOLERANCE {
            return Err(invalid(if delta < 0.0 {
                "degenerate point conic"
            } else {
                "degenerate intersecting lines"
            }));
        }

        // Use the determinant for the smaller-magnitude eigenvalue to avoid
        // losing it to cancellation near the conditioning threshold.
        let mid = (a + c) * 0.5;
        let radius = (a - c).hypot(b) * 0.5;
        let (lambda1, lambda2) = if mid >= 0.0 {
            (mid + radius, det / (mid + radius))
        } else {
            (det / (mid - radius), mid - radius)
        };
        let hyperbolic_axis = match (
            fp.signum() == lambda1.signum(),
            fp.signum() == lambda2.signum(),
        ) {
            (false, false) => None,
            (true, false) => Some(0),
            (false, true) => Some(1),
            (true, true) => return Err(invalid("non-real conic (no real points)")),
        };
        let radii = [lambda1, lambda2].map(|l| magnitude * fp.abs().sqrt() / l.abs().sqrt());
        // Preserve the direction (and chosen hyperbola branch) of the
        // existing eigenvector (B/2, lambda1 - A), including axis-aligned cases.
        let axis = if b == 0.0 {
            if a >= c {
                [1.0, 0.0]
            } else {
                [0.0, 1.0]
            }
        } else {
            let (sin, cos) = (0.5 * b.atan2(a - c)).sin_cos();
            [cos * b.signum(), sin * b.signum()]
        };
        let conic = Self {
            center: [u, v],
            linear_scale,
            quadratic_scale: scale,
            axis,
            radii,
            hyperbolic_axis,
        };
        if conic
            .center
            .iter()
            .chain(&conic.radii)
            .any(|&x| !conic.world_length(x).is_finite())
        {
            return Err(invalid("non-finite canonical form"));
        }
        Ok(conic)
    }

    fn world_length(&self, x: f64) -> f64 {
        let factor = self.linear_scale / self.quadratic_scale;
        if factor.is_finite() {
            x * factor
        } else {
            // The scale factor alone may overflow while a small x still fits.
            (x * self.linear_scale) / self.quadratic_scale
        }
    }

    pub(super) fn is_open(&self) -> bool {
        self.hyperbolic_axis.is_some()
    }

    pub(super) fn tessellate(
        &self,
        id: &str,
        segments: usize,
    ) -> Result<Vec<[f64; 2]>, KernelError> {
        let n = segments.max(1) * 2;
        (0..n)
            .map(|i| {
                let (x, y) = if let Some(axis) = self.hyperbolic_axis {
                    let t = -2.0 + i as f64 * 4.0 / (n - 1) as f64;
                    if axis == 0 {
                        (t.sinh(), t.cosh())
                    } else {
                        (t.cosh(), t.sinh())
                    }
                } else {
                    let t = i as f64 * std::f64::consts::TAU / n as f64;
                    (t.cos(), t.sin())
                };
                let (x, y) = (x * self.radii[0], y * self.radii[1]);
                let [ex, ey] = self.axis;
                let point = [
                    self.world_length(x * ex - y * ey + self.center[0]),
                    self.world_length(x * ey + y * ex + self.center[1]),
                ];
                if point.iter().all(|p| p.is_finite()) {
                    Ok(point)
                } else {
                    Err(KernelError::DegenerateSketchElement {
                        element_id: id.into(),
                        reason: "non-finite conic sample",
                    })
                }
            })
            .collect()
    }
}
