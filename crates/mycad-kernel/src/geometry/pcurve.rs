use crate::error::KernelError;
use serde::{Deserialize, Serialize};

/// A 2D curve in the UV parameter space of a surface.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "curve2d_type", rename_all = "snake_case")]
pub enum Curve2D {
    Line2D {
        origin: (f64, f64),
        direction: (f64, f64),
    },
    Circle2D {
        center: (f64, f64),
        radius: f64,
    },
}

impl Curve2D {
    /// Create a line in parameter space. Rejects non-finite components or zero direction.
    pub fn try_line(origin: (f64, f64), direction: (f64, f64)) -> Result<Self, KernelError> {
        if !origin.0.is_finite()
            || !origin.1.is_finite()
            || !direction.0.is_finite()
            || !direction.1.is_finite()
        {
            return Err(KernelError::DegeneratePcurve {
                reason: "non-finite component",
            });
        }
        if direction == (0.0, 0.0) {
            return Err(KernelError::DegeneratePcurve {
                reason: "zero direction",
            });
        }
        Ok(Curve2D::Line2D { origin, direction })
    }

    /// Create a circle in parameter space. Rejects non-finite values or non-positive radius.
    pub fn try_circle(center: (f64, f64), radius: f64) -> Result<Self, KernelError> {
        if !center.0.is_finite() || !center.1.is_finite() || !radius.is_finite() {
            return Err(KernelError::DegeneratePcurve {
                reason: "non-finite component",
            });
        }
        if radius <= 0.0 {
            return Err(KernelError::DegeneratePcurve {
                reason: "non-positive radius",
            });
        }
        Ok(Curve2D::Circle2D { center, radius })
    }

    /// Evaluate the curve at parameter t.
    pub fn evaluate(&self, t: f64) -> (f64, f64) {
        match self {
            Curve2D::Line2D { origin, direction } => {
                (origin.0 + t * direction.0, origin.1 + t * direction.1)
            }
            Curve2D::Circle2D { center, radius } => {
                (center.0 + radius * t.cos(), center.1 + radius * t.sin())
            }
        }
    }

    /// Sample N points from `t_start` toward `t_end` (endpoint exclusive).
    /// Descending order (`t_start > t_end`) is allowed.
    pub fn sample_segment(&self, t_start: f64, t_end: f64, segments: usize) -> Vec<(f64, f64)> {
        match self {
            Curve2D::Line2D { .. } => {
                vec![self.evaluate(t_start)]
            }
            Curve2D::Circle2D { .. } => {
                if segments == 0 {
                    return vec![self.evaluate(t_start)];
                }
                let step = (t_end - t_start) / segments as f64;
                (0..segments)
                    .map(|i| self.evaluate(t_start + step * i as f64))
                    .collect()
            }
        }
    }
}

/// A curve in the UV parameter space of a surface, bounded by a parameter range.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Pcurve {
    curve_2d: Curve2D,
    t_range: [f64; 2],
}

impl Pcurve {
    /// Create a pcurve. Rejects non-finite t_range or zero-length trim.
    /// Descending t_range (`t_range[0] > t_range[1]`) is allowed for CW arcs.
    pub fn try_new(curve_2d: Curve2D, t_range: [f64; 2]) -> Result<Self, KernelError> {
        if !t_range[0].is_finite() || !t_range[1].is_finite() {
            return Err(KernelError::InvalidPcurveTrange {
                t_start: t_range[0],
                t_end: t_range[1],
            });
        }
        if t_range[0] == t_range[1] {
            return Err(KernelError::InvalidPcurveTrange {
                t_start: t_range[0],
                t_end: t_range[1],
            });
        }
        Ok(Pcurve { curve_2d, t_range })
    }

    pub fn curve_2d(&self) -> &Curve2D {
        &self.curve_2d
    }

    pub fn t_range(&self) -> [f64; 2] {
        self.t_range
    }

    /// Evaluate at parameter t, clamped to the t_range bounds.
    pub fn evaluate(&self, t: f64) -> (f64, f64) {
        let lo = f64::min(self.t_range[0], self.t_range[1]);
        let hi = f64::max(self.t_range[0], self.t_range[1]);
        let clamped = f64::clamp(t, lo, hi);
        self.curve_2d.evaluate(clamped)
    }

    /// Sample N points from `t_range[0]` toward `t_range[1]` (HE traversal direction).
    pub fn sample(&self, segments: usize) -> Vec<(f64, f64)> {
        self.curve_2d
            .sample_segment(self.t_range[0], self.t_range[1], segments)
    }
}

// --- Custom Deserialize: validate through try_* constructors ---

impl<'de> Deserialize<'de> for Curve2D {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)]
        #[serde(tag = "curve2d_type", rename_all = "snake_case")]
        enum Shadow {
            Line2D {
                origin: (f64, f64),
                direction: (f64, f64),
            },
            Circle2D {
                center: (f64, f64),
                radius: f64,
            },
        }

        let raw = Shadow::deserialize(deserializer)?;
        match raw {
            Shadow::Line2D { origin, direction } => {
                Curve2D::try_line(origin, direction).map_err(serde::de::Error::custom)
            }
            Shadow::Circle2D { center, radius } => {
                Curve2D::try_circle(center, radius).map_err(serde::de::Error::custom)
            }
        }
    }
}

impl<'de> Deserialize<'de> for Pcurve {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)]
        struct Shadow {
            curve_2d: Curve2D,
            t_range: [f64; 2],
        }

        let raw = Shadow::deserialize(deserializer)?;
        Pcurve::try_new(raw.curve_2d, raw.t_range).map_err(serde::de::Error::custom)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // T05: Curve2D::try_line evaluate
    #[test]
    fn t05_line_evaluate() {
        let line = Curve2D::try_line((0.0, 0.0), (1.0, 0.0)).unwrap();
        let (u, v) = line.evaluate(0.5);
        assert!((u - 0.5).abs() < 1e-12);
        assert!(v.abs() < 1e-12);
    }

    // T06: Curve2D::try_circle sample_segment
    #[test]
    fn t06_circle_sample_segment() {
        let circle = Curve2D::try_circle((0.0, 0.0), 1.0).unwrap();
        let pts = circle.sample_segment(0.0, std::f64::consts::PI, 4);
        assert_eq!(pts.len(), 4);
        for (u, v) in &pts {
            let r = (u * u + v * v).sqrt();
            assert!((r - 1.0).abs() < 1e-12, "radius={}", r);
        }
    }

    // T07: degenerate Curve2D
    #[test]
    fn t07_line_zero_direction() {
        assert!(matches!(
            Curve2D::try_line((0.0, 0.0), (0.0, 0.0)),
            Err(KernelError::DegeneratePcurve { .. })
        ));
    }

    #[test]
    fn t07_circle_zero_radius() {
        assert!(matches!(
            Curve2D::try_circle((0.0, 0.0), 0.0),
            Err(KernelError::DegeneratePcurve { .. })
        ));
    }

    #[test]
    fn t07_circle_negative_radius() {
        assert!(matches!(
            Curve2D::try_circle((0.0, 0.0), -1.0),
            Err(KernelError::DegeneratePcurve { .. })
        ));
    }

    #[test]
    fn t07_line_non_finite() {
        assert!(matches!(
            Curve2D::try_line((f64::NAN, 0.0), (1.0, 0.0)),
            Err(KernelError::DegeneratePcurve { .. })
        ));
    }

    #[test]
    fn t07_circle_non_finite() {
        assert!(matches!(
            Curve2D::try_circle((0.0, f64::INFINITY), 1.0),
            Err(KernelError::DegeneratePcurve { .. })
        ));
    }

    // T07b: very small but non-zero direction is valid
    #[test]
    fn t07b_line_tiny_direction_valid() {
        assert!(Curve2D::try_line((0.0, 0.0), (1e-15, 0.0)).is_ok());
    }

    // T07c: invalid Curve2D YAML rejected by Deserialize
    #[test]
    fn t07c_yaml_reject_zero_direction() {
        let yaml = "curve2d_type: line2d\norigin:\n- 0\n- 0\ndirection:\n- 0\n- 0\n";
        let result = serde_yaml::from_str::<Curve2D>(yaml);
        assert!(result.is_err());
    }

    // T08: Pcurve degenerate t_range
    #[test]
    fn t08_zero_length_trange() {
        let line = Curve2D::try_line((0.0, 0.0), (1.0, 0.0)).unwrap();
        assert!(matches!(
            Pcurve::try_new(line, [1.0, 1.0]),
            Err(KernelError::InvalidPcurveTrange { .. })
        ));
    }

    #[test]
    fn t08_nan_trange() {
        let line = Curve2D::try_line((0.0, 0.0), (1.0, 0.0)).unwrap();
        assert!(matches!(
            Pcurve::try_new(line, [f64::NAN, 1.0]),
            Err(KernelError::InvalidPcurveTrange { .. })
        ));
    }

    // T08b: descending t_range (CW arc) is allowed
    #[test]
    fn t08b_descending_trange_cw_arc() {
        let circle = Curve2D::try_circle((0.0, 0.0), 1.0).unwrap();
        let pc = Pcurve::try_new(circle, [std::f64::consts::PI, 0.0]).unwrap();
        let pts = pc.sample(4);
        assert_eq!(pts.len(), 4);
        // Angles should be decreasing (CW)
        for i in 1..pts.len() {
            let angle_prev = pts[i - 1].1.atan2(pts[i - 1].0);
            let angle_cur = pts[i].1.atan2(pts[i].0);
            assert!(
                angle_cur < angle_prev + 1e-9,
                "angle should decrease: {} -> {}",
                angle_prev,
                angle_cur
            );
        }
    }

    // T08c: descending sample_segment for Line2D
    #[test]
    fn t08c_line_descending_sample() {
        let line = Curve2D::try_line((0.0, 0.0), (1.0, 0.0)).unwrap();
        let pts = line.sample_segment(1.0, 0.0, 3);
        // Line2D always returns 1 point (at t_start)
        assert_eq!(pts.len(), 1);
        let (u, _v) = pts[0];
        assert!((u - 1.0).abs() < 1e-12);
    }

    // T08d: invalid Pcurve YAML rejected
    #[test]
    fn t08d_yaml_reject_zero_trange() {
        let yaml = "curve_2d:\n  curve2d_type: line2d\n  origin:\n  - 0\n  - 0\n  direction:\n  - 1\n  - 0\nt_range:\n- 1.0\n- 1.0\n";
        let result = serde_yaml::from_str::<Pcurve>(yaml);
        assert!(result.is_err());
    }

    // YAML roundtrip for Curve2D
    #[test]
    fn test_curve2d_yaml_roundtrip() {
        let line = Curve2D::try_line((1.0, 2.0), (3.0, 4.0)).unwrap();
        let yaml = serde_yaml::to_string(&line).unwrap();
        let parsed: Curve2D = serde_yaml::from_str(&yaml).unwrap();
        assert_eq!(line, parsed);
    }

    #[test]
    fn test_circle2d_yaml_roundtrip() {
        let circle = Curve2D::try_circle((1.0, 2.0), 3.0).unwrap();
        let yaml = serde_yaml::to_string(&circle).unwrap();
        let parsed: Curve2D = serde_yaml::from_str(&yaml).unwrap();
        assert_eq!(circle, parsed);
    }

    // YAML roundtrip for Pcurve
    #[test]
    fn test_pcurve_yaml_roundtrip() {
        let line = Curve2D::try_line((0.0, 0.0), (1.0, 0.0)).unwrap();
        let pc = Pcurve::try_new(line, [0.0, 1.0]).unwrap();
        let yaml = serde_yaml::to_string(&pc).unwrap();
        let parsed: Pcurve = serde_yaml::from_str(&yaml).unwrap();
        assert_eq!(pc, parsed);
    }

    // Adversarial: determinism
    #[test]
    fn test_determinism_100_runs() {
        for _ in 0..100 {
            let circle = Curve2D::try_circle((0.0, 0.0), 1.0).unwrap();
            let pc = Pcurve::try_new(circle, [0.0, std::f64::consts::FRAC_PI_2]).unwrap();
            let pts = pc.sample(8);
            assert_eq!(pts.len(), 8);
        }
    }

    // Adversarial: line evaluate consistency
    #[test]
    fn test_line_evaluate_consistency() {
        let line = Curve2D::try_line((0.0, 0.0), (1.0, 0.0)).unwrap();
        for _ in 0..100 {
            let (u, v) = line.evaluate(0.5);
            assert!((u - 0.5).abs() < 1e-15);
            assert!(v.abs() < 1e-15);
        }
    }

    // Adversarial: Pcurve clamping
    #[test]
    fn test_pcurve_clamp_out_of_range() {
        let line = Curve2D::try_line((0.0, 0.0), (1.0, 0.0)).unwrap();
        let pc = Pcurve::try_new(line, [0.0, 1.0]).unwrap();
        // Beyond upper bound
        let (u, _) = pc.evaluate(2.0);
        assert!((u - 1.0).abs() < 1e-12);
        // Below lower bound
        let (u, _) = pc.evaluate(-1.0);
        assert!(u.abs() < 1e-12);
    }

    #[test]
    fn test_pcurve_clamp_descending() {
        let circle = Curve2D::try_circle((0.0, 0.0), 1.0).unwrap();
        let pc = Pcurve::try_new(circle, [std::f64::consts::PI, 0.0]).unwrap();
        // Beyond lower (0.0) should clamp to 0.0
        let (u, v) = pc.evaluate(-1.0);
        assert!((u - 1.0).abs() < 1e-12);
        assert!(v.abs() < 1e-12);
    }
}
