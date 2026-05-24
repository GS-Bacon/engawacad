use super::math::orthonormal_basis;
use super::{Point, Vec3};
use serde::{Deserialize, Serialize};

/// A geometric curve in 3D space.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Curve {
    /// A line defined by an origin point and a direction vector.
    Line { origin: Point, direction: Vec3 },
    /// A circle defined by a center, normal, and radius.
    Circle {
        center: Point,
        normal: Vec3,
        radius: f64,
    },
}

impl Curve {
    /// Evaluate the curve at parameter t.
    /// For a line: origin + t * direction
    /// For a circle: center + radius * (cos(t) * u + sin(t) * v)
    pub fn evaluate(&self, t: f64) -> Point {
        match self {
            Curve::Line { origin, direction } => origin + t * direction,
            Curve::Circle {
                center,
                normal,
                radius,
            } => {
                // Build a local coordinate system from the normal
                let (u, v) = orthonormal_basis(normal);
                center + *radius * (t.cos() * u + t.sin() * v)
            }
        }
    }

    /// Sample points along the curve from `t_start` (inclusive) toward `t_end` (exclusive).
    ///
    /// Returns a non-empty point sequence starting at `evaluate(t_start)`.
    /// - `Line`: single point at `t_start` (linear interpolation between vertices handles the rest).
    /// - `Circle`: `segments` equally-spaced points from `t_start` toward `t_end`.
    pub fn sample_segment(&self, t_start: f64, t_end: f64, segments: usize) -> Vec<Point> {
        match self {
            Curve::Line { .. } => vec![self.evaluate(t_start)],
            Curve::Circle { .. } => {
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
