use super::curve::Curve;
use super::math::ANGLE_TOLERANCE;
use super::pcurve::Curve2D;
use super::surface::Surface;
use super::Point;
use crate::error::KernelError;
use serde::{Deserialize, Serialize};
use std::f64::consts::PI;

/// One 3D intersection curve between two surfaces, with pcurve representations
/// on each surface's UV parameter space.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct IntersectionLoop {
    pub curve_3d: Curve,
    pub t_range: [f64; 2],
    pub pcurve_on_a: Curve2D,
    pub pcurve_on_a_t_range: [f64; 2],
    pub pcurve_on_b: Curve2D,
    pub pcurve_on_b_t_range: [f64; 2],
}

/// Compute intersection curves between two surfaces.
/// MVP supports: Plane×Plane, Plane×Cylinder (axis ⊥ plane),
/// Plane×Sphere (normal ±Z), Cylinder×Sphere (coaxial).
pub fn intersect_surfaces(a: &Surface, b: &Surface) -> Result<Vec<IntersectionLoop>, KernelError> {
    match (a, b) {
        (Surface::Plane { .. }, Surface::Plane { .. }) => intersect_plane_plane(a, b),
        (Surface::Plane { .. }, Surface::Cylinder { .. }) => intersect_plane_cylinder(a, b),
        (Surface::Cylinder { .. }, Surface::Plane { .. }) => intersect_plane_cylinder(b, a),
        (Surface::Plane { .. }, Surface::Sphere { .. }) => intersect_plane_sphere(a, b),
        (Surface::Sphere { .. }, Surface::Plane { .. }) => intersect_plane_sphere(b, a),
        (Surface::Cylinder { .. }, Surface::Sphere { .. }) => intersect_cylinder_sphere(a, b),
        (Surface::Sphere { .. }, Surface::Cylinder { .. }) => intersect_cylinder_sphere(b, a),
        (Surface::Cylinder { .. }, Surface::Cylinder { .. }) => {
            Err(KernelError::UnsupportedSurfaceIntersection {
                reason: "cylinder × cylinder not supported",
            })
        }
        (Surface::Sphere { .. }, Surface::Sphere { .. }) => {
            Err(KernelError::UnsupportedSurfaceIntersection {
                reason: "sphere × sphere not supported",
            })
        }
        (_, Surface::Cone { .. }) | (Surface::Cone { .. }, _) => {
            Err(KernelError::UnsupportedSurfaceIntersection {
                reason: "cone surfaces not supported",
            })
        }
    }
}

fn intersect_plane_plane(a: &Surface, b: &Surface) -> Result<Vec<IntersectionLoop>, KernelError> {
    let (a_origin, a_normal, a_u_axis, a_v_axis) = match a {
        Surface::Plane {
            origin,
            normal,
            u_axis,
            v_axis,
        } => (*origin, *normal, *u_axis, *v_axis),
        _ => unreachable!(),
    };
    let (b_origin, b_normal, b_u_axis, b_v_axis) = match b {
        Surface::Plane {
            origin,
            normal,
            u_axis,
            v_axis,
        } => (*origin, *normal, *u_axis, *v_axis),
        _ => unreachable!(),
    };

    let n_dot = a_normal.dot(&b_normal);
    if n_dot.abs() > 1.0 - ANGLE_TOLERANCE {
        return Ok(vec![]);
    }

    let direction = a_normal.cross(&b_normal).normalize();

    // Solve both plane equations simultaneously:
    // P = α * a_normal + β * b_normal where a_normal·P = c_a and b_normal·P = c_b
    let c_a = a_origin.coords.dot(&a_normal);
    let c_b = b_origin.coords.dot(&b_normal);
    let denom2 = 1.0 - n_dot * n_dot;
    if denom2.abs() < ANGLE_TOLERANCE * ANGLE_TOLERANCE {
        return Ok(vec![]);
    }
    let origin_3d = Point::from(
        ((c_a - n_dot * c_b) / denom2) * a_normal + ((c_b - n_dot * c_a) / denom2) * b_normal,
    );

    let curve_3d = Curve::Line {
        origin: origin_3d,
        direction,
    };

    let a_dir_2d = (direction.dot(&a_u_axis), direction.dot(&a_v_axis));
    let a_orig_2d = (
        (origin_3d - a_origin).dot(&a_u_axis),
        (origin_3d - a_origin).dot(&a_v_axis),
    );

    let b_dir_2d = (direction.dot(&b_u_axis), direction.dot(&b_v_axis));
    let b_orig_2d = (
        (origin_3d - b_origin).dot(&b_u_axis),
        (origin_3d - b_origin).dot(&b_v_axis),
    );

    let pcurve_on_a = Curve2D::try_line(a_orig_2d, a_dir_2d)
        .map_err(|e| KernelError::BooleanInternal(format!("pcurve_on_a: {e}")))?;
    let pcurve_on_b = Curve2D::try_line(b_orig_2d, b_dir_2d)
        .map_err(|e| KernelError::BooleanInternal(format!("pcurve_on_b: {e}")))?;

    Ok(vec![IntersectionLoop {
        curve_3d,
        t_range: [0.0, 1.0],
        pcurve_on_a,
        pcurve_on_a_t_range: [0.0, 1.0],
        pcurve_on_b,
        pcurve_on_b_t_range: [0.0, 1.0],
    }])
}

fn intersect_plane_cylinder(
    plane: &Surface,
    cylinder: &Surface,
) -> Result<Vec<IntersectionLoop>, KernelError> {
    let (plane_origin, plane_normal, plane_u_axis, plane_v_axis) = match plane {
        Surface::Plane {
            origin,
            normal,
            u_axis,
            v_axis,
        } => (*origin, *normal, *u_axis, *v_axis),
        _ => unreachable!(),
    };
    let (cyl_origin, cyl_axis_raw, cyl_radius) = match cylinder {
        Surface::Cylinder {
            origin,
            axis,
            radius,
        } => (*origin, *axis, *radius),
        _ => unreachable!(),
    };

    let cyl_axis = cyl_axis_raw.normalize();

    // MVP: axis must be perpendicular to plane normal (|axis·normal| ≈ 1)
    let alignment = cyl_axis.dot(&plane_normal).abs();
    if alignment < 1.0 - ANGLE_TOLERANCE {
        return Err(KernelError::UnsupportedSurfaceIntersection {
            reason: "non-perpendicular plane × cylinder",
        });
    }

    // Center of intersection circle: cylinder axis intersects the plane
    let diff = plane_origin - cyl_origin;
    let t = diff.dot(&plane_normal) / cyl_axis.dot(&plane_normal);
    let center = cyl_origin + t * cyl_axis;

    let curve_3d = Curve::Circle {
        center,
        normal: plane_normal,
        radius: cyl_radius,
    };

    // Pcurve on plane: Circle2D
    let center_2d = (
        (center - plane_origin).dot(&plane_u_axis),
        (center - plane_origin).dot(&plane_v_axis),
    );
    let pcurve_on_plane = Curve2D::try_circle(center_2d, cyl_radius)
        .map_err(|e| KernelError::BooleanInternal(format!("pcurve_on_plane: {e}")))?;

    // Pcurve on cylinder: Line2D (v=const on cylinder axis, u sweeps 0→2π)
    let v_const = (center - cyl_origin).dot(&cyl_axis);
    let pcurve_on_cyl = Curve2D::try_line((0.0, v_const), (1.0, 0.0))
        .map_err(|e| KernelError::BooleanInternal(format!("pcurve_on_cyl: {e}")))?;

    Ok(vec![IntersectionLoop {
        curve_3d,
        t_range: [0.0, 2.0 * PI],
        pcurve_on_a: pcurve_on_plane,
        pcurve_on_a_t_range: [0.0, 2.0 * PI],
        pcurve_on_b: pcurve_on_cyl,
        pcurve_on_b_t_range: [0.0, 2.0 * PI],
    }])
}

fn intersect_plane_sphere(
    plane: &Surface,
    sphere: &Surface,
) -> Result<Vec<IntersectionLoop>, KernelError> {
    let (plane_origin, plane_normal, plane_u_axis, plane_v_axis) = match plane {
        Surface::Plane {
            origin,
            normal,
            u_axis,
            v_axis,
        } => (*origin, *normal, *u_axis, *v_axis),
        _ => unreachable!(),
    };
    let (sph_center, sph_radius) = match sphere {
        Surface::Sphere { center, radius } => (*center, *radius),
        _ => unreachable!(),
    };

    // MVP: plane normal must be ±Z
    if plane_normal.z.abs() < 1.0 - ANGLE_TOLERANCE {
        return Err(KernelError::UnsupportedSurfaceIntersection {
            reason: "non-Z-aligned plane × sphere",
        });
    }

    let d = (sph_center - plane_origin).dot(&plane_normal);
    if d.abs() > sph_radius - super::math::LENGTH_TOLERANCE {
        return Ok(vec![]);
    }

    let r = (sph_radius * sph_radius - d * d).sqrt();
    let center = sph_center - d * plane_normal;

    let curve_3d = Curve::Circle {
        center,
        normal: plane_normal,
        radius: r,
    };

    // Pcurve on plane: Circle2D
    let center_2d = (
        (center - plane_origin).dot(&plane_u_axis),
        (center - plane_origin).dot(&plane_v_axis),
    );
    let pcurve_on_plane = Curve2D::try_circle(center_2d, r)
        .map_err(|e| KernelError::BooleanInternal(format!("pcurve_on_plane: {e}")))?;

    // Pcurve on sphere: Line2D at v=const (latitude)
    let v_lat = (d / sph_radius).clamp(-1.0, 1.0).asin();
    let pcurve_on_sph = Curve2D::try_line((0.0, v_lat), (1.0, 0.0))
        .map_err(|e| KernelError::BooleanInternal(format!("pcurve_on_sph: {e}")))?;

    Ok(vec![IntersectionLoop {
        curve_3d,
        t_range: [0.0, 2.0 * PI],
        pcurve_on_a: pcurve_on_plane,
        pcurve_on_a_t_range: [0.0, 2.0 * PI],
        pcurve_on_b: pcurve_on_sph,
        pcurve_on_b_t_range: [0.0, 2.0 * PI],
    }])
}

fn intersect_cylinder_sphere(
    cylinder: &Surface,
    sphere: &Surface,
) -> Result<Vec<IntersectionLoop>, KernelError> {
    let (cyl_origin, cyl_axis_raw, cyl_radius) = match cylinder {
        Surface::Cylinder {
            origin,
            axis,
            radius,
        } => (*origin, *axis, *radius),
        _ => unreachable!(),
    };
    let (sph_center, sph_radius) = match sphere {
        Surface::Sphere { center, radius } => (*center, *radius),
        _ => unreachable!(),
    };

    let cyl_axis = cyl_axis_raw.normalize();

    // MVP: axis must be ±Z
    if cyl_axis.z.abs() < 1.0 - ANGLE_TOLERANCE {
        return Err(KernelError::UnsupportedSurfaceIntersection {
            reason: "non-coaxial cylinder × sphere",
        });
    }

    // Sphere center must be on cylinder axis
    let to_sph = sph_center - cyl_origin;
    let perp = to_sph - to_sph.dot(&cyl_axis) * cyl_axis;
    if perp.norm() > super::math::LENGTH_TOLERANCE {
        return Err(KernelError::UnsupportedSurfaceIntersection {
            reason: "non-coaxial cylinder × sphere",
        });
    }

    let h_center = to_sph.dot(&cyl_axis);
    let r_sq = sph_radius * sph_radius - cyl_radius * cyl_radius;
    if r_sq < -super::math::LENGTH_TOLERANCE {
        return Ok(vec![]);
    }
    if r_sq < super::math::LENGTH_TOLERANCE {
        return Ok(vec![]);
    }

    let h_offset = r_sq.sqrt();
    let mut loops = Vec::new();

    for h in [h_center - h_offset, h_center + h_offset] {
        let center = cyl_origin + h * cyl_axis;
        let curve_3d = Curve::Circle {
            center,
            normal: cyl_axis,
            radius: cyl_radius,
        };

        let pcurve_on_cyl = Curve2D::try_line((0.0, h), (1.0, 0.0))
            .map_err(|e| KernelError::BooleanInternal(format!("pcurve_on_cyl: {e}")))?;

        let v_lat = (h / sph_radius).clamp(-1.0, 1.0).asin();
        let pcurve_on_sph = Curve2D::try_line((0.0, v_lat), (1.0, 0.0))
            .map_err(|e| KernelError::BooleanInternal(format!("pcurve_on_sph: {e}")))?;

        loops.push(IntersectionLoop {
            curve_3d,
            t_range: [0.0, 2.0 * PI],
            pcurve_on_a: pcurve_on_cyl,
            pcurve_on_a_t_range: [0.0, 2.0 * PI],
            pcurve_on_b: pcurve_on_sph,
            pcurve_on_b_t_range: [0.0, 2.0 * PI],
        });
    }

    // R04: sort by h (projection of center onto cylinder axis) ascending
    loops.sort_by(|a_loop, b_loop| {
        let ha = match &a_loop.curve_3d {
            Curve::Circle { center, .. } => (*center - cyl_origin).dot(&cyl_axis),
            _ => 0.0,
        };
        let hb = match &b_loop.curve_3d {
            Curve::Circle { center, .. } => (*center - cyl_origin).dot(&cyl_axis),
            _ => 0.0,
        };
        ha.partial_cmp(&hb).unwrap_or(std::cmp::Ordering::Equal)
    });

    Ok(loops)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::geometry::math::orthonormal_basis;
    use crate::geometry::math::LENGTH_TOLERANCE;
    use crate::geometry::{Point, Vec3};

    fn plane_z(z: f64) -> Surface {
        Surface::Plane {
            origin: Point::new(0.0, 0.0, z),
            normal: Vec3::z(),
            u_axis: Vec3::x(),
            v_axis: Vec3::y(),
        }
    }

    fn plane_x(x: f64) -> Surface {
        Surface::Plane {
            origin: Point::new(x, 0.0, 0.0),
            normal: Vec3::x(),
            u_axis: Vec3::y(),
            v_axis: Vec3::z(),
        }
    }

    fn plane_normal(normal: Vec3, origin: Point) -> Surface {
        let n = normal.normalize();
        let (u, v) = orthonormal_basis(&n);
        Surface::Plane {
            origin,
            normal: n,
            u_axis: u,
            v_axis: v,
        }
    }

    fn cylinder_z(radius: f64, origin: Point) -> Surface {
        Surface::Cylinder {
            origin,
            axis: Vec3::z(),
            radius,
        }
    }

    fn sphere(center: Point, radius: f64) -> Surface {
        Surface::Sphere { center, radius }
    }

    // T01: Plane×Cylinder (axis ⊥ plane) → 1 Circle
    #[test]
    fn t01_plane_cylinder() {
        let plane = plane_z(0.0);
        let cyl = cylinder_z(2.0, Point::origin());
        let loops = intersect_surfaces(&plane, &cyl).unwrap();
        assert_eq!(loops.len(), 1);
        let il = &loops[0];
        if let Curve::Circle {
            center,
            normal,
            radius,
        } = &il.curve_3d
        {
            let dist: Vec3 = center - Point::origin();
            assert!(dist.norm() < LENGTH_TOLERANCE);
            assert!((normal - Vec3::z()).norm() < LENGTH_TOLERANCE);
            assert!((radius - 2.0).abs() < LENGTH_TOLERANCE);
        } else {
            panic!("expected Circle");
        }
        assert!(matches!(il.pcurve_on_a, Curve2D::Circle2D { .. }));
        assert!(matches!(il.pcurve_on_b, Curve2D::Line2D { .. }));
    }

    // T02: Plane×Sphere (normal ±Z) → 1 Circle
    #[test]
    fn t02_plane_sphere() {
        let plane = plane_z(3.0);
        let sph = sphere(Point::origin(), 5.0);
        let loops = intersect_surfaces(&plane, &sph).unwrap();
        assert_eq!(loops.len(), 1);
        let il = &loops[0];
        if let Curve::Circle { radius, .. } = &il.curve_3d {
            let expected_r = (25.0_f64 - 9.0).sqrt();
            assert!((radius - expected_r).abs() < LENGTH_TOLERANCE);
        } else {
            panic!("expected Circle");
        }
    }

    // T03: Cylinder×Sphere (coaxial) → 2 Circles
    #[test]
    fn t03_cylinder_sphere() {
        let cyl = cylinder_z(3.0, Point::new(0.0, 0.0, -10.0));
        let sph = sphere(Point::origin(), 5.0);
        let loops = intersect_surfaces(&cyl, &sph).unwrap();
        assert_eq!(loops.len(), 2);
        for il in &loops {
            if let Curve::Circle { radius, .. } = &il.curve_3d {
                assert!((radius - 3.0).abs() < LENGTH_TOLERANCE);
            }
        }
        // R04: sorted by h ascending
        if let (Curve::Circle { center: c0, .. }, Curve::Circle { center: c1, .. }) =
            (&loops[0].curve_3d, &loops[1].curve_3d)
        {
            assert!(c0.z < c1.z);
        }
    }

    // T04: Plane×Cylinder axis ∥ plane → UnsupportedSurfaceIntersection
    #[test]
    fn t04_plane_cylinder_axis_parallel() {
        let plane = plane_x(0.0);
        let cyl = cylinder_z(2.0, Point::origin());
        let result = intersect_surfaces(&plane, &cyl);
        assert!(matches!(
            result,
            Err(KernelError::UnsupportedSurfaceIntersection { .. })
        ));
    }

    // T04b: Plane×Cylinder intermediate angle → UnsupportedSurfaceIntersection
    #[test]
    fn t04b_plane_cylinder_intermediate_angle() {
        let normal = Vec3::new(1.0, 1.0, 0.0).normalize();
        let plane = plane_normal(normal, Point::origin());
        let cyl = cylinder_z(2.0, Point::origin());
        let result = intersect_surfaces(&plane, &cyl);
        assert!(matches!(
            result,
            Err(KernelError::UnsupportedSurfaceIntersection { .. })
        ));
    }

    // T05: Plane×Sphere normal not ±Z → UnsupportedSurfaceIntersection
    #[test]
    fn t05_plane_sphere_non_z() {
        let plane = plane_x(0.0);
        let sph = sphere(Point::origin(), 5.0);
        let result = intersect_surfaces(&plane, &sph);
        assert!(matches!(
            result,
            Err(KernelError::UnsupportedSurfaceIntersection { .. })
        ));
    }

    // T06: Cylinder×Sphere non-coaxial → UnsupportedSurfaceIntersection
    #[test]
    fn t06_cylinder_sphere_non_coaxial() {
        let cyl = cylinder_z(2.0, Point::origin());
        let sph = sphere(Point::new(1.0, 0.0, 0.0), 2.0);
        let result = intersect_surfaces(&cyl, &sph);
        assert!(matches!(
            result,
            Err(KernelError::UnsupportedSurfaceIntersection { .. })
        ));
    }

    // T07: Plane×Sphere distance > R → empty Vec
    #[test]
    fn t07_plane_sphere_no_intersection() {
        let plane = plane_z(10.0);
        let sph = sphere(Point::origin(), 5.0);
        let loops = intersect_surfaces(&plane, &sph).unwrap();
        assert!(loops.is_empty());
    }

    // T08: Plane×Sphere tangent → empty Vec
    #[test]
    fn t08_plane_sphere_tangent() {
        let plane = plane_z(5.0);
        let sph = sphere(Point::origin(), 5.0);
        let loops = intersect_surfaces(&plane, &sph).unwrap();
        assert!(loops.is_empty());
    }

    // T04_reason: verify reason string
    #[test]
    fn t04_reason_string() {
        let plane = plane_x(0.0);
        let cyl = cylinder_z(2.0, Point::origin());
        let result = intersect_surfaces(&plane, &cyl);
        if let Err(KernelError::UnsupportedSurfaceIntersection { reason }) = result {
            assert_eq!(reason, "non-perpendicular plane × cylinder");
        } else {
            panic!("expected UnsupportedSurfaceIntersection");
        }
    }

    // Cylinder×Cylinder → UnsupportedSurfaceIntersection
    #[test]
    fn cylinder_cylinder_unsupported() {
        let cyl1 = cylinder_z(2.0, Point::origin());
        let cyl2 = cylinder_z(3.0, Point::new(0.0, 0.0, 1.0));
        let result = intersect_surfaces(&cyl1, &cyl2);
        assert!(matches!(
            result,
            Err(KernelError::UnsupportedSurfaceIntersection { .. })
        ));
    }

    // T01b: Plane×Plane intersection origin must lie on both planes
    #[test]
    fn t01b_plane_plane_intersection_origin_on_both_planes() {
        let plane_z = Surface::Plane {
            origin: Point::new(0.0, 0.0, 5.0),
            normal: Vec3::z(),
            u_axis: Vec3::x(),
            v_axis: Vec3::y(),
        };
        let plane_x = Surface::Plane {
            origin: Point::new(3.0, 0.0, 0.0),
            normal: Vec3::x(),
            u_axis: Vec3::y(),
            v_axis: Vec3::z(),
        };
        let loops = intersect_surfaces(&plane_z, &plane_x).unwrap();
        assert_eq!(loops.len(), 1);
        let il = &loops[0];
        // intersection line must be z=5, x=3, y=free
        if let Curve::Line { origin, direction } = &il.curve_3d {
            // origin must be on both planes
            assert!(
                (origin.z - 5.0).abs() < LENGTH_TOLERANCE,
                "origin.z must be 5.0, got {}",
                origin.z
            );
            assert!(
                (origin.x - 3.0).abs() < LENGTH_TOLERANCE,
                "origin.x must be 3.0, got {}",
                origin.x
            );
            // direction must be Y axis
            assert!(
                (direction.y.abs() - 1.0).abs() < LENGTH_TOLERANCE,
                "direction must be ±Y"
            );
        } else {
            panic!("expected Line");
        }
        // pcurve_on_a (plane_z UV): origin.u must be 3.0
        if let Curve2D::Line2D { origin, .. } = &il.pcurve_on_a {
            assert!(
                (origin.0 - 3.0).abs() < LENGTH_TOLERANCE,
                "pcurve_on_a origin.u must be 3.0, got {}",
                origin.0
            );
        }
    }

    // Additional Plane×Plane test: arbitrary offset planes
    #[test]
    fn t01c_plane_plane_arbitrary_offsets() {
        let plane_a = Surface::Plane {
            origin: Point::new(1.0, 2.0, 3.0),
            normal: Vec3::new(0.0, 0.0, 1.0),
            u_axis: Vec3::new(1.0, 0.0, 0.0),
            v_axis: Vec3::new(0.0, 1.0, 0.0),
        };
        let plane_b = Surface::Plane {
            origin: Point::new(4.0, 5.0, 6.0),
            normal: Vec3::new(1.0, 0.0, 0.0),
            u_axis: Vec3::new(0.0, 1.0, 0.0),
            v_axis: Vec3::new(0.0, 0.0, 1.0),
        };
        let loops = intersect_surfaces(&plane_a, &plane_b).unwrap();
        assert_eq!(loops.len(), 1);
        let il = &loops[0];
        if let Curve::Line { origin, .. } = &il.curve_3d {
            // origin must be on plane_a (z=3) and plane_b (x=4)
            assert!(
                (origin.z - 3.0).abs() < LENGTH_TOLERANCE,
                "origin.z must be 3.0, got {}",
                origin.z
            );
            assert!(
                (origin.x - 4.0).abs() < LENGTH_TOLERANCE,
                "origin.x must be 4.0, got {}",
                origin.x
            );
        }
    }

    // ========== Adversarial edge case tests ==========

    // Determinism: same input → same output (100 runs)
    #[test]
    fn edge_determinism_100_runs() {
        let plane = plane_z(0.0);
        let cyl = cylinder_z(2.0, Point::origin());
        let first = intersect_surfaces(&plane, &cyl).unwrap();
        for _ in 0..99 {
            let result = intersect_surfaces(&plane, &cyl).unwrap();
            assert_eq!(result.len(), first.len());
            for (a, b) in result.iter().zip(first.iter()) {
                assert_eq!(a, b, "determinism violation in surface intersection");
            }
        }
    }

    // Degenerate: sphere with radius at f64::MIN_POSITIVE — tangent should return empty
    #[test]
    fn edge_sphere_min_positive_radius() {
        let plane = plane_z(0.0);
        let sph = sphere(Point::origin(), f64::MIN_POSITIVE);
        let result = intersect_surfaces(&plane, &sph);
        // Radius is so tiny it's effectively zero — either Ok(empty) or just works
        assert!(result.is_ok());
        assert!(result.unwrap().is_empty());
    }

    // Numerical: plane at -0.0 should behave the same as 0.0
    #[test]
    fn edge_negative_zero_plane() {
        let plane_neg = Surface::Plane {
            origin: Point::new(0.0, 0.0, -0.0),
            normal: Vec3::z(),
            u_axis: Vec3::x(),
            v_axis: Vec3::y(),
        };
        let plane_pos = plane_z(0.0);
        let cyl = cylinder_z(2.0, Point::origin());
        let r_neg = intersect_surfaces(&plane_neg, &cyl).unwrap();
        let r_pos = intersect_surfaces(&plane_pos, &cyl).unwrap();
        assert_eq!(r_neg.len(), r_pos.len());
    }

    // Degenerate: cylinder with very large radius
    #[test]
    fn edge_cylinder_large_radius() {
        let plane = plane_z(0.0);
        let cyl = cylinder_z(1e10, Point::origin());
        let result = intersect_surfaces(&plane, &cyl).unwrap();
        assert_eq!(result.len(), 1);
        if let Curve::Circle { radius, .. } = &result[0].curve_3d {
            assert!((radius - 1e10).abs() < 1.0); // relative tolerance for large values
        }
    }

    // Degenerate: sphere centered at origin with plane at origin (d=0, max circle)
    #[test]
    fn edge_plane_through_sphere_center() {
        let plane = plane_z(0.0);
        let sph = sphere(Point::origin(), 5.0);
        let loops = intersect_surfaces(&plane, &sph).unwrap();
        assert_eq!(loops.len(), 1);
        if let Curve::Circle { radius, center, .. } = &loops[0].curve_3d {
            assert!((radius - 5.0).abs() < LENGTH_TOLERANCE);
            assert!((center - Point::origin()).norm() < LENGTH_TOLERANCE);
        }
    }

    // Degenerate: coaxial cylinder×sphere where cylinder barely penetrates
    #[test]
    fn edge_cyl_sphere_near_tangent() {
        // cylinder radius = sphere radius: tangent at the equator
        let cyl = cylinder_z(5.0, Point::origin());
        let sph = sphere(Point::origin(), 5.0);
        let result = intersect_surfaces(&cyl, &sph).unwrap();
        // tangent: r_sq = 25 - 25 = 0, should return empty
        assert!(result.is_empty());
    }

    // Degenerate: coaxial cylinder×sphere where cylinder is bigger
    #[test]
    fn edge_cyl_sphere_cylinder_bigger() {
        let cyl = cylinder_z(6.0, Point::origin());
        let sph = sphere(Point::origin(), 5.0);
        // r_sq = 25 - 36 = -11 < 0 → no intersection
        let result = intersect_surfaces(&cyl, &sph).unwrap();
        assert!(result.is_empty());
    }

    // Symmetry: Plane×Cylinder and Cylinder×Plane give equivalent results
    #[test]
    fn edge_plane_cylinder_symmetry() {
        let plane = plane_z(0.0);
        let cyl = cylinder_z(2.0, Point::new(0.0, 0.0, 3.0));
        let r1 = intersect_surfaces(&plane, &cyl).unwrap();
        let r2 = intersect_surfaces(&cyl, &plane).unwrap();
        assert_eq!(r1.len(), r2.len());
        if r1.is_empty() {
            return;
        }
        let get_radius = |v: &[IntersectionLoop]| -> f64 {
            match &v[0].curve_3d {
                Curve::Circle { radius, .. } => *radius,
                _ => panic!("expected Circle"),
            }
        };
        assert!((get_radius(&r1) - get_radius(&r2)).abs() < LENGTH_TOLERANCE);
    }

    // Numerical: sphere × sphere → UnsupportedSurfaceIntersection
    #[test]
    fn edge_sphere_sphere_unsupported() {
        let s1 = sphere(Point::origin(), 3.0);
        let s2 = sphere(Point::new(1.0, 0.0, 0.0), 2.0);
        let result = intersect_surfaces(&s1, &s2);
        assert!(matches!(
            result,
            Err(KernelError::UnsupportedSurfaceIntersection { .. })
        ));
    }

    // Numerical: Plane×Plane with coplanar normals → empty
    #[test]
    fn edge_plane_plane_coplanar() {
        let p1 = plane_z(0.0);
        let p2 = plane_z(5.0);
        let result = intersect_surfaces(&p1, &p2).unwrap();
        assert!(result.is_empty());
    }

    // Numerical: Plane×Plane with anti-parallel normals at same offset → empty
    #[test]
    fn edge_plane_plane_antiparallel_same_offset() {
        let p1 = Surface::Plane {
            origin: Point::new(0.0, 0.0, 5.0),
            normal: Vec3::z(),
            u_axis: Vec3::x(),
            v_axis: Vec3::y(),
        };
        let p2 = Surface::Plane {
            origin: Point::new(0.0, 0.0, 5.0),
            normal: -Vec3::z(),
            u_axis: Vec3::x(),
            v_axis: Vec3::y(),
        };
        let result = intersect_surfaces(&p1, &p2).unwrap();
        assert!(result.is_empty());
    }

    // Roundtrip: IntersectionLoop serde roundtrip
    #[test]
    fn edge_intersection_loop_serde_roundtrip() {
        let plane = plane_z(0.0);
        let cyl = cylinder_z(2.0, Point::origin());
        let loops = intersect_surfaces(&plane, &cyl).unwrap();
        let il = &loops[0];
        let yaml = serde_yaml::to_string(il).unwrap();
        let deserialized: IntersectionLoop = serde_yaml::from_str(&yaml).unwrap();
        assert_eq!(il, &deserialized);
    }

    // Numerical: very small cylinder radius
    #[test]
    fn edge_tiny_cylinder_radius() {
        let plane = plane_z(0.0);
        let cyl = cylinder_z(LENGTH_TOLERANCE, Point::origin());
        let result = intersect_surfaces(&plane, &cyl).unwrap();
        assert_eq!(result.len(), 1);
        if let Curve::Circle { radius, .. } = &result[0].curve_3d {
            assert!((radius - LENGTH_TOLERANCE).abs() < LENGTH_TOLERANCE);
        }
    }

    // Cone × anything → UnsupportedSurfaceIntersection
    #[test]
    fn edge_cone_rejected() {
        let cone = Surface::Cone {
            apex: Point::origin(),
            axis: Vec3::z(),
            half_angle: std::f64::consts::FRAC_PI_4,
        };
        let plane = plane_z(0.0);
        let result = intersect_surfaces(&cone, &plane);
        assert!(matches!(
            result,
            Err(KernelError::UnsupportedSurfaceIntersection { .. })
        ));
    }

    // Cylinder×Sphere: sphere center slightly off-axis → non-coaxial
    #[test]
    fn edge_cyl_sphere_slightly_off_axis() {
        let cyl = cylinder_z(2.0, Point::origin());
        let sph = sphere(Point::new(LENGTH_TOLERANCE * 10.0, 0.0, 0.0), 5.0);
        let result = intersect_surfaces(&cyl, &sph);
        assert!(matches!(
            result,
            Err(KernelError::UnsupportedSurfaceIntersection { .. })
        ));
    }

    // Plane×Plane: arbitrary oblique intersection (angled planes)
    #[test]
    fn edge_oblique_plane_intersection() {
        let p1 = Surface::Plane {
            origin: Point::origin(),
            normal: Vec3::new(0.0, 0.0, 1.0),
            u_axis: Vec3::new(1.0, 0.0, 0.0),
            v_axis: Vec3::new(0.0, 1.0, 0.0),
        };
        let p2 = Surface::Plane {
            origin: Point::origin(),
            normal: Vec3::new(0.0, 1.0, 1.0).normalize(),
            u_axis: Vec3::new(1.0, 0.0, 0.0),
            v_axis: Vec3::new(0.0, -1.0, 1.0).normalize(),
        };
        let loops = intersect_surfaces(&p1, &p2).unwrap();
        assert_eq!(loops.len(), 1);
        // origin must be on both planes
        if let Curve::Line { origin, .. } = &loops[0].curve_3d {
            // Both planes pass through origin, so intersection must too
            assert!(
                origin.coords.norm() < LENGTH_TOLERANCE,
                "origin must be at origin, got {:?}",
                origin
            );
        }
    }
}
