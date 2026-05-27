use crate::brep::topology::{IdGenerator, Solid};
use crate::error::KernelError;
use crate::geometry::curve::Curve;
use crate::geometry::math::LENGTH_TOLERANCE;
use crate::geometry::surface::Surface;
#[cfg(test)]
use crate::geometry::Vec3;

/// Signed area of the 2D polygon (shoelace formula).
fn signed_area(profile: &[(f64, f64)]) -> f64 {
    let n = profile.len();
    let mut area = 0.0;
    for i in 0..n {
        let j = (i + 1) % n;
        area += profile[i].0 * profile[j].1;
        area -= profile[j].0 * profile[i].1;
    }
    area / 2.0
}

/// Cross product of consecutive edge vectors at vertex i (2× triangle area).
fn turn_cross(profile: &[(f64, f64)], i: usize) -> f64 {
    let n = profile.len();
    let prev = (i + n - 1) % n;
    let next = (i + 1) % n;
    let d1 = (
        profile[i].0 - profile[prev].0,
        profile[i].1 - profile[prev].1,
    );
    let d2 = (
        profile[next].0 - profile[i].0,
        profile[next].1 - profile[i].1,
    );
    d1.0 * d2.1 - d1.1 * d2.0
}

/// Check convexity: all turn cross products have the same sign AND winding number is ±1.
fn is_convex(profile: &[(f64, f64)], area_eps: f64) -> bool {
    let n = profile.len();
    if n < 3 {
        return false;
    }

    let mut angle_sum = 0.0f64;
    for i in 0..n {
        let prev = (i + n - 1) % n;
        let next = (i + 1) % n;
        let d1 = (
            profile[i].0 - profile[prev].0,
            profile[i].1 - profile[prev].1,
        );
        let d2 = (
            profile[next].0 - profile[i].0,
            profile[next].1 - profile[i].1,
        );
        let angle = d1.0.atan2(d1.1) - d2.0.atan2(d2.1);
        let angle = if angle > std::f64::consts::PI {
            angle - 2.0 * std::f64::consts::PI
        } else if angle < -std::f64::consts::PI {
            angle + 2.0 * std::f64::consts::PI
        } else {
            angle
        };
        angle_sum += angle;
    }
    let winding = angle_sum / (2.0 * std::f64::consts::PI);
    if (winding.abs() - 1.0).abs() > 0.01 {
        return false;
    }

    let mut first_sign: Option<f64> = None;
    for i in 0..n {
        let cross = turn_cross(profile, i);
        if cross.abs() <= area_eps {
            continue;
        }
        let sign = cross.signum();
        match first_sign {
            None => first_sign = Some(sign),
            Some(fs) if fs != sign => return false,
            _ => {}
        }
    }
    true
}

/// Check simplicity: no non-adjacent edges intersect.
fn is_simple(profile: &[(f64, f64)], eps: f64) -> bool {
    let n = profile.len();
    for i in 0..n {
        for j in (i + 2)..n {
            if i == 0 && j == n - 1 {
                continue;
            }
            let a1 = profile[i];
            let a2 = profile[(i + 1) % n];
            let b1 = profile[j];
            let b2 = profile[(j + 1) % n];

            if segments_intersect_open(a1, a2, b1, b2, eps) {
                return false;
            }
        }
    }
    true
}

/// Check if two 2D segments intersect in their open interiors (excluding endpoints).
fn segments_intersect_open(
    a1: (f64, f64),
    a2: (f64, f64),
    b1: (f64, f64),
    b2: (f64, f64),
    eps: f64,
) -> bool {
    let d1 = (a2.0 - a1.0, a2.1 - a1.1);
    let d2 = (b2.0 - b1.0, b2.1 - b1.1);

    let denom = d1.0 * d2.1 - d1.1 * d2.0;
    if denom.abs() < eps * eps {
        return false;
    }

    let dx = b1.0 - a1.0;
    let dy = b1.1 - a1.1;
    let t = (dx * d2.1 - dy * d2.0) / denom;
    let u = (dx * d1.1 - dy * d1.0) / denom;

    let margin = eps;
    t > margin && t < 1.0 - margin && u > margin && u < 1.0 - margin
}

/// Create an extrusion solid from a 2D profile on a plane.
///
/// The profile is a list of UV coordinates forming a closed polygon on the given plane.
/// The extrusion extends along the plane's normal by `depth`.
pub fn make_extrusion(
    plane: &crate::geometry::Plane,
    profile: &[(f64, f64)],
    depth: f64,
    id_gen: &mut IdGenerator,
) -> Result<Solid, KernelError> {
    let length_eps = LENGTH_TOLERANCE;
    let area_eps = LENGTH_TOLERANCE * LENGTH_TOLERANCE;

    if !depth.is_finite() || depth <= length_eps {
        return Err(KernelError::InvalidParameter { kind: "depth" });
    }

    let n = profile.len();
    if n < 3 {
        return Err(KernelError::InvalidParameter { kind: "profile" });
    }

    for &(u, v) in profile {
        if !u.is_finite() || !v.is_finite() {
            return Err(KernelError::InvalidParameter { kind: "profile" });
        }
    }

    for i in 0..n {
        let j = (i + 1) % n;
        let du = profile[j].0 - profile[i].0;
        let dv = profile[j].1 - profile[i].1;
        if (du * du + dv * dv).sqrt() <= length_eps {
            return Err(KernelError::InvalidParameter { kind: "profile" });
        }
    }

    for i in 0..n {
        let cross = turn_cross(profile, i);
        if cross.abs() <= area_eps {
            return Err(KernelError::InvalidParameter { kind: "profile" });
        }
    }

    let sa = signed_area(profile);
    if sa.abs() <= area_eps {
        return Err(KernelError::InvalidParameter { kind: "profile" });
    }

    if !is_convex(profile, area_eps) {
        return Err(KernelError::InvalidParameter { kind: "profile" });
    }
    if !is_simple(profile, length_eps) {
        return Err(KernelError::InvalidParameter { kind: "profile" });
    }

    let mut solid = Solid::new(id_gen.next());

    // Handedness of the plane's coordinate system
    let cross_uv = plane.u_axis.cross(&plane.v_axis);
    let handedness = cross_uv.dot(&plane.normal).signum();
    let winding = sa.signum() * handedness;

    // Build 3D positions for bottom and top vertices
    let mut bottom_v = Vec::with_capacity(n);
    let mut top_v = Vec::with_capacity(n);
    for &(u, v) in profile {
        let p = plane.origin + u * plane.u_axis + v * plane.v_axis;
        bottom_v.push(solid.add_vertex(id_gen.next(), p));
        top_v.push(solid.add_vertex(id_gen.next(), p + depth * plane.normal));
    }

    // Edges: bottom n, top n, vertical n
    let mut bottom_edges = Vec::with_capacity(n);
    let mut top_edges = Vec::with_capacity(n);
    let mut vertical_edges = Vec::with_capacity(n);

    for i in 0..n {
        let j = (i + 1) % n;
        let p0 = solid.vertices[bottom_v[i]].point;
        let p1 = solid.vertices[bottom_v[j]].point;
        bottom_edges.push(solid.add_edge(
            id_gen.next(),
            [bottom_v[i], bottom_v[j]],
            Curve::Line {
                origin: p0,
                direction: p1 - p0,
            },
            [0.0, 1.0],
        ));
    }

    for i in 0..n {
        let j = (i + 1) % n;
        let p0 = solid.vertices[top_v[i]].point;
        let p1 = solid.vertices[top_v[j]].point;
        top_edges.push(solid.add_edge(
            id_gen.next(),
            [top_v[i], top_v[j]],
            Curve::Line {
                origin: p0,
                direction: p1 - p0,
            },
            [0.0, 1.0],
        ));
    }

    for i in 0..n {
        let p0 = solid.vertices[bottom_v[i]].point;
        let p1 = solid.vertices[top_v[i]].point;
        vertical_edges.push(solid.add_edge(
            id_gen.next(),
            [bottom_v[i], top_v[i]],
            Curve::Line {
                origin: p0,
                direction: p1 - p0,
            },
            [0.0, 1.0],
        ));
    }

    // Cap HE direction logic:
    // Side face k uses bottom_edge[k] forward when winding > 0, backward when winding < 0.
    // Each edge needs exactly 2 HEs with opposite orientation.
    // Bottom cap uses the opposite direction of the side face on bottom edges.
    // Top cap uses the opposite direction of the side face on top edges.
    let side_bottom_fwd = winding > 0.0;
    let side_top_fwd = winding < 0.0;

    // Bottom cap: opposite of side face bottom direction
    let mut bottom_hes = Vec::with_capacity(n);
    if !side_bottom_fwd {
        // Cap uses forward, iterate edges 0..n
        for i in 0..n {
            bottom_hes.push(solid.add_half_edge(id_gen.next(), bottom_v[i], bottom_edges[i], true));
        }
    } else {
        // Cap uses backward, iterate edges in reverse for loop closure
        for i in (0..n).rev() {
            let j = (i + 1) % n;
            bottom_hes.push(solid.add_half_edge(
                id_gen.next(),
                bottom_v[j],
                bottom_edges[i],
                false,
            ));
        }
    }
    let bottom_loop = solid.add_loop(id_gen.next(), bottom_hes);
    let bottom_face = solid.add_face(
        id_gen.next(),
        Surface::Plane {
            origin: plane.origin,
            normal: -plane.normal,
            u_axis: plane.u_axis,
            v_axis: plane.v_axis,
        },
        bottom_loop,
        vec![],
        true,
    );

    // Top cap: opposite of side face top direction
    let mut top_hes = Vec::with_capacity(n);
    if !side_top_fwd {
        // Cap uses forward
        for i in 0..n {
            top_hes.push(solid.add_half_edge(id_gen.next(), top_v[i], top_edges[i], true));
        }
    } else {
        // Cap uses backward, reverse order
        for i in (0..n).rev() {
            let j = (i + 1) % n;
            top_hes.push(solid.add_half_edge(id_gen.next(), top_v[j], top_edges[i], false));
        }
    }
    let top_loop = solid.add_loop(id_gen.next(), top_hes);
    let top_face = solid.add_face(
        id_gen.next(),
        Surface::Plane {
            origin: plane.origin + depth * plane.normal,
            normal: plane.normal,
            u_axis: plane.u_axis,
            v_axis: plane.v_axis,
        },
        top_loop,
        vec![],
        true,
    );

    // Side faces
    let mut side_faces = Vec::with_capacity(n);
    for k in 0..n {
        let j = (k + 1) % n;
        let p_bk = solid.vertices[bottom_v[k]].point;
        let p_bj = solid.vertices[bottom_v[j]].point;
        let p_tk = solid.vertices[top_v[k]].point;

        let side_normal = if side_bottom_fwd {
            (p_bj - p_bk).cross(&(p_tk - p_bk))
        } else {
            (p_tk - p_bk).cross(&(p_bj - p_bk))
        };
        let side_normal = if side_normal.norm() > area_eps {
            side_normal.normalize()
        } else {
            return Err(KernelError::InvalidParameter { kind: "profile" });
        };

        let he_loop = if side_bottom_fwd {
            // bottom_fwd, vert_j_fwd, top_bwd, vert_k_bwd
            let he0 = solid.add_half_edge(id_gen.next(), bottom_v[k], bottom_edges[k], true);
            let he1 = solid.add_half_edge(id_gen.next(), bottom_v[j], vertical_edges[j], true);
            let he2 = solid.add_half_edge(id_gen.next(), top_v[j], top_edges[k], false);
            let he3 = solid.add_half_edge(id_gen.next(), top_v[k], vertical_edges[k], false);
            vec![he0, he1, he2, he3]
        } else {
            // vert_k_fwd, top_fwd, vert_j_bwd, bottom_bwd
            let he0 = solid.add_half_edge(id_gen.next(), bottom_v[k], vertical_edges[k], true);
            let he1 = solid.add_half_edge(id_gen.next(), top_v[k], top_edges[k], true);
            let he2 = solid.add_half_edge(id_gen.next(), top_v[j], vertical_edges[j], false);
            let he3 = solid.add_half_edge(id_gen.next(), bottom_v[j], bottom_edges[k], false);
            vec![he0, he1, he2, he3]
        };

        let side_loop = solid.add_loop(id_gen.next(), he_loop);
        let side_face = solid.add_face(
            id_gen.next(),
            Surface::Plane {
                origin: p_bk,
                normal: side_normal,
                u_axis: (p_bj - p_bk).normalize(),
                v_axis: (p_tk - p_bk).normalize(),
            },
            side_loop,
            vec![],
            true,
        );
        side_faces.push(side_face);
    }

    let mut all_faces = vec![bottom_face, top_face];
    all_faces.extend(side_faces);

    solid.add_shell(id_gen.next(), all_faces, true);

    Ok(solid)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::brep::topology::IdGenerator;
    use crate::error::KernelError;
    use crate::geometry::Plane;

    fn rect_profile() -> Vec<(f64, f64)> {
        vec![(0.0, 0.0), (10.0, 0.0), (10.0, 5.0), (0.0, 5.0)]
    }

    // T01: Determinism
    #[test]
    fn test_deterministic() {
        let plane = Plane::xy();
        let profile = rect_profile();
        let mut gen1 = IdGenerator::new(0);
        let mut gen2 = IdGenerator::new(0);

        let s1 = make_extrusion(&plane, &profile, 8.0, &mut gen1).unwrap();
        let s2 = make_extrusion(&plane, &profile, 8.0, &mut gen2).unwrap();

        assert_eq!(s1.vertices.len(), s2.vertices.len());
        assert_eq!(s1.edges.len(), s2.edges.len());
        assert_eq!(s1.faces.len(), s2.faces.len());
        assert_eq!(s1.half_edges.len(), s2.half_edges.len());
        assert_eq!(s1.loops.len(), s2.loops.len());
        assert_eq!(s1.shells.len(), s2.shells.len());

        for (v1, v2) in s1.vertices.iter().zip(s2.vertices.iter()) {
            assert_eq!(v1.id, v2.id);
            assert_eq!(v1.point, v2.point);
        }
        for (e1, e2) in s1.edges.iter().zip(s2.edges.iter()) {
            assert_eq!(e1.id, e2.id);
        }
        for (f1, f2) in s1.faces.iter().zip(s2.faces.iter()) {
            assert_eq!(f1.id, f2.id);
        }
    }

    // T02: Rect profile topology
    #[test]
    fn test_rect_topology() {
        let plane = Plane::xy();
        let profile = rect_profile();
        let mut gen = IdGenerator::new(0);
        let solid = make_extrusion(&plane, &profile, 8.0, &mut gen).unwrap();

        assert_eq!(solid.vertices.len(), 8, "V=2N=8");
        assert_eq!(solid.edges.len(), 12, "E=3N=12");
        assert_eq!(solid.faces.len(), 6, "F=N+2=6");
        assert_eq!(solid.shells.len(), 1);
        assert!(solid.shells[0].closed);
        assert_eq!(solid.half_edges.len(), 24, "HE=6N=24");
        assert_eq!(solid.loops.len(), 6, "loops=N+2=6");
    }

    // T03: Euler-Poincaré
    #[test]
    fn test_euler() {
        let plane = Plane::xy();
        let profile = rect_profile();
        let mut gen = IdGenerator::new(0);
        let solid = make_extrusion(&plane, &profile, 8.0, &mut gen).unwrap();

        let v = solid.vertices.len();
        let e = solid.edges.len();
        let f = solid.faces.len();
        let s = solid.shells.len() as i64;
        assert_eq!((v as i64) - (e as i64) + (f as i64), 2 * s);
    }

    // T04: Manifold validation
    #[test]
    fn test_manifold() {
        let plane = Plane::xy();
        let profile = rect_profile();
        let mut gen = IdGenerator::new(0);
        let solid = make_extrusion(&plane, &profile, 8.0, &mut gen).unwrap();
        assert!(solid.validate_manifold().is_ok());
    }

    // T05: Geometry — cap normals and top vertex Z
    #[test]
    fn test_cap_geometry_xy() {
        let plane = Plane::xy();
        let profile = rect_profile();
        let mut gen = IdGenerator::new(0);
        let solid = make_extrusion(&plane, &profile, 8.0, &mut gen).unwrap();

        if let Surface::Plane { normal, .. } = &solid.faces[0].surface {
            let diff = (*normal + Vec3::z()).norm();
            assert!(
                diff < 1e-12,
                "bottom cap normal should be -Z, got {:?}",
                normal
            );
        }

        if let Surface::Plane { normal, .. } = &solid.faces[1].surface {
            let diff = (*normal - Vec3::z()).norm();
            assert!(
                diff < 1e-12,
                "top cap normal should be +Z, got {:?}",
                normal
            );
        }

        // Top vertices are at odd indices (bottom_v[0]=idx0, top_v[0]=idx1, ...)
        let top_count = solid
            .vertices
            .iter()
            .filter(|v| (v.point.z - 8.0).abs() < 1e-12)
            .count();
        assert_eq!(top_count, 4, "should have 4 top vertices at z=8.0");
    }

    // T06: Degenerate inputs
    #[test]
    fn test_degenerate_depth() {
        let plane = Plane::xy();
        let profile = rect_profile();
        let mut gen = IdGenerator::new(0);

        assert!(matches!(
            make_extrusion(&plane, &profile, 0.0, &mut gen),
            Err(KernelError::InvalidParameter { kind: "depth" })
        ));
        assert!(matches!(
            make_extrusion(&plane, &profile, -1.0, &mut gen),
            Err(KernelError::InvalidParameter { kind: "depth" })
        ));
        assert!(matches!(
            make_extrusion(&plane, &profile, f64::NAN, &mut gen),
            Err(KernelError::InvalidParameter { kind: "depth" })
        ));
        assert!(matches!(
            make_extrusion(&plane, &profile, f64::INFINITY, &mut gen),
            Err(KernelError::InvalidParameter { kind: "depth" })
        ));
    }

    #[test]
    fn test_degenerate_profile_few_vertices() {
        let plane = Plane::xy();
        let mut gen = IdGenerator::new(0);

        assert!(matches!(
            make_extrusion(&plane, &[(0.0, 0.0)], 5.0, &mut gen),
            Err(KernelError::InvalidParameter { kind: "profile" })
        ));
        assert!(matches!(
            make_extrusion(&plane, &[(0.0, 0.0), (1.0, 0.0)], 5.0, &mut gen),
            Err(KernelError::InvalidParameter { kind: "profile" })
        ));
    }

    #[test]
    fn test_degenerate_zero_length_edge() {
        let plane = Plane::xy();
        let profile = vec![(0.0, 0.0), (0.0, 0.0), (1.0, 0.0), (1.0, 1.0), (0.0, 1.0)];
        let mut gen = IdGenerator::new(0);
        assert!(matches!(
            make_extrusion(&plane, &profile, 5.0, &mut gen),
            Err(KernelError::InvalidParameter { kind: "profile" })
        ));
    }

    #[test]
    fn test_degenerate_collinear() {
        let plane = Plane::xy();
        let profile = vec![(0.0, 0.0), (5.0, 0.0), (10.0, 0.0), (10.0, 5.0), (0.0, 5.0)];
        let mut gen = IdGenerator::new(0);
        assert!(matches!(
            make_extrusion(&plane, &profile, 5.0, &mut gen),
            Err(KernelError::InvalidParameter { kind: "profile" })
        ));
    }

    #[test]
    fn test_degenerate_zero_area() {
        let plane = Plane::xy();
        let profile = vec![(0.0, 0.0), (1.0, 0.0), (2.0, 0.0), (1.0, 0.0)];
        let mut gen = IdGenerator::new(0);
        assert!(matches!(
            make_extrusion(&plane, &profile, 5.0, &mut gen),
            Err(KernelError::InvalidParameter { kind: "profile" })
        ));
    }

    // T07: Orientation — xy, xz, yz planes with CW and CCW input
    #[test]
    fn test_orientation_xy_ccw() {
        let plane = Plane::xy();
        let profile = vec![(0.0, 0.0), (10.0, 0.0), (10.0, 5.0), (0.0, 5.0)];
        let mut gen = IdGenerator::new(0);
        let solid = make_extrusion(&plane, &profile, 8.0, &mut gen).unwrap();
        assert!(solid.validate_manifold().is_ok());

        if let Surface::Plane { normal, .. } = &solid.faces[0].surface {
            assert!((*normal + Vec3::z()).norm() < 1e-12);
        }
        if let Surface::Plane { normal, .. } = &solid.faces[1].surface {
            assert!((*normal - Vec3::z()).norm() < 1e-12);
        }
    }

    #[test]
    fn test_orientation_xy_cw() {
        let plane = Plane::xy();
        let profile = vec![(0.0, 0.0), (0.0, 5.0), (10.0, 5.0), (10.0, 0.0)];
        let mut gen = IdGenerator::new(0);
        let solid = make_extrusion(&plane, &profile, 8.0, &mut gen).unwrap();
        assert!(solid.validate_manifold().is_ok());

        if let Surface::Plane { normal, .. } = &solid.faces[0].surface {
            assert!((*normal + Vec3::z()).norm() < 1e-12);
        }
        if let Surface::Plane { normal, .. } = &solid.faces[1].surface {
            assert!((*normal - Vec3::z()).norm() < 1e-12);
        }
    }

    #[test]
    fn test_orientation_xz_plane() {
        let plane = Plane::xz();
        let profile = vec![(0.0, 0.0), (10.0, 0.0), (10.0, 5.0), (0.0, 5.0)];
        let mut gen = IdGenerator::new(0);
        let solid = make_extrusion(&plane, &profile, 8.0, &mut gen).unwrap();
        assert!(solid.validate_manifold().is_ok());

        if let Surface::Plane { normal, .. } = &solid.faces[0].surface {
            assert!(
                (*normal + Vec3::y()).norm() < 1e-12,
                "xz bottom cap normal should be -Y, got {:?}",
                normal
            );
        }
        if let Surface::Plane { normal, .. } = &solid.faces[1].surface {
            assert!(
                (*normal - Vec3::y()).norm() < 1e-12,
                "xz top cap normal should be +Y, got {:?}",
                normal
            );
        }
    }

    #[test]
    fn test_orientation_xz_plane_cw() {
        let plane = Plane::xz();
        // CW in xz (left-handed plane) — bottom cap must still be -Y, top +Y
        let profile = vec![(0.0, 0.0), (0.0, 5.0), (10.0, 5.0), (10.0, 0.0)];
        let mut gen = IdGenerator::new(0);
        let solid = make_extrusion(&plane, &profile, 8.0, &mut gen).unwrap();
        assert!(solid.validate_manifold().is_ok());
        if let Surface::Plane { normal, .. } = &solid.faces[0].surface {
            assert!(
                (*normal + Vec3::y()).norm() < 1e-12,
                "xz CW bottom cap normal should be -Y, got {:?}",
                normal
            );
        }
        if let Surface::Plane { normal, .. } = &solid.faces[1].surface {
            assert!(
                (*normal - Vec3::y()).norm() < 1e-12,
                "xz CW top cap normal should be +Y, got {:?}",
                normal
            );
        }
    }
    #[test]
    fn test_orientation_yz_plane() {
        let plane = Plane::yz();
        let profile = vec![(0.0, 0.0), (10.0, 0.0), (10.0, 5.0), (0.0, 5.0)];
        let mut gen = IdGenerator::new(0);
        let solid = make_extrusion(&plane, &profile, 8.0, &mut gen).unwrap();
        assert!(solid.validate_manifold().is_ok());

        if let Surface::Plane { normal, .. } = &solid.faces[0].surface {
            assert!(
                (*normal + Vec3::x()).norm() < 1e-12,
                "yz bottom cap normal should be -X, got {:?}",
                normal
            );
        }
        if let Surface::Plane { normal, .. } = &solid.faces[1].surface {
            assert!(
                (*normal - Vec3::x()).norm() < 1e-12,
                "yz top cap normal should be +X, got {:?}",
                normal
            );
        }
    }

    #[test]
    fn test_orientation_yz_plane_cw() {
        let plane = Plane::yz();
        // CW input in yz — bottom cap must still be -X, top +X
        let profile = vec![(0.0, 0.0), (0.0, 5.0), (10.0, 5.0), (10.0, 0.0)];
        let mut gen = IdGenerator::new(0);
        let solid = make_extrusion(&plane, &profile, 8.0, &mut gen).unwrap();
        assert!(solid.validate_manifold().is_ok());
        if let Surface::Plane { normal, .. } = &solid.faces[0].surface {
            assert!(
                (*normal + Vec3::x()).norm() < 1e-12,
                "yz CW bottom cap normal should be -X, got {:?}",
                normal
            );
        }
        if let Surface::Plane { normal, .. } = &solid.faces[1].surface {
            assert!(
                (*normal - Vec3::x()).norm() < 1e-12,
                "yz CW top cap normal should be +X, got {:?}",
                normal
            );
        }
    }

    // T13: Non-convex and self-intersecting profiles rejected
    #[test]
    fn test_non_convex_rejected() {
        let plane = Plane::xy();
        let profile = vec![
            (0.0, 0.0),
            (10.0, 0.0),
            (10.0, 5.0),
            (5.0, 5.0),
            (5.0, 10.0),
            (0.0, 10.0),
        ];
        let mut gen = IdGenerator::new(0);
        assert!(matches!(
            make_extrusion(&plane, &profile, 5.0, &mut gen),
            Err(KernelError::InvalidParameter { kind: "profile" })
        ));
    }

    #[test]
    fn test_self_intersecting_rejected() {
        let plane = Plane::xy();
        let profile = vec![(0.0, 0.0), (10.0, 10.0), (10.0, 0.0), (0.0, 10.0)];
        let mut gen = IdGenerator::new(0);
        assert!(matches!(
            make_extrusion(&plane, &profile, 5.0, &mut gen),
            Err(KernelError::InvalidParameter { kind: "profile" })
        ));
    }

    // T15: Side face ↔ segment correspondence (bottom_edge[k] is in side face k)
    #[test]
    fn test_side_face_segment_order() {
        let plane = Plane::xy();
        let profile = rect_profile();
        let mut gen = IdGenerator::new(0);
        let solid = make_extrusion(&plane, &profile, 8.0, &mut gen).unwrap();

        for k in 0..4 {
            let face = &solid.faces[2 + k];
            let lp = &solid.loops[face.outer_loop];
            // Check that bottom_edges[k] appears in this side face's loop
            let mut found = false;
            for &he_idx in &lp.half_edges {
                let he = &solid.half_edges[he_idx];
                if he.edge == k {
                    found = true;
                    break;
                }
            }
            assert!(found, "side face {} should use bottom_edge {}", k, k);
        }
    }

    // Edge case: 100-run determinism
    #[test]
    fn test_determinism_100_runs() {
        let plane = Plane::xy();
        let profile = rect_profile();

        let mut gen0 = IdGenerator::new(0);
        let reference = make_extrusion(&plane, &profile, 8.0, &mut gen0).unwrap();
        let ref_ids: Vec<_> = reference.vertices.iter().map(|v| v.id).collect();

        for i in 0..100 {
            let mut gen = IdGenerator::new(0);
            let solid = make_extrusion(&plane, &profile, 8.0, &mut gen).unwrap();
            let ids: Vec<_> = solid.vertices.iter().map(|v| v.id).collect();
            assert_eq!(ids, ref_ids, "run {i}: vertex IDs differ");
        }
    }

    // Edge case: pentagon (non-rectangular convex)
    #[test]
    fn test_pentagon() {
        let plane = Plane::xy();
        let profile = vec![(0.0, 0.0), (2.0, 0.0), (3.0, 1.5), (1.5, 3.0), (-0.5, 2.0)];
        let mut gen = IdGenerator::new(0);
        let solid = make_extrusion(&plane, &profile, 5.0, &mut gen).unwrap();

        assert_eq!(solid.vertices.len(), 10, "V=2*5=10");
        assert_eq!(solid.edges.len(), 15, "E=3*5=15");
        assert_eq!(solid.faces.len(), 7, "F=5+2=7");
        assert_eq!(solid.half_edges.len(), 30, "HE=6*5=30");
        assert!(solid.validate_manifold().is_ok());
    }

    // Edge case: f64 boundary values for coordinates
    #[test]
    fn test_large_coordinates() {
        let plane = Plane::xy();
        let profile = vec![(0.0, 0.0), (1e10, 0.0), (1e10, 1e10), (0.0, 1e10)];
        let mut gen = IdGenerator::new(0);
        let solid = make_extrusion(&plane, &profile, 1.0, &mut gen).unwrap();
        assert_eq!(solid.vertices.len(), 8);
        assert!(solid.validate_manifold().is_ok());
    }

    #[test]
    fn test_small_but_valid() {
        let plane = Plane::xy();
        let profile = vec![(0.0, 0.0), (1e-6, 0.0), (1e-6, 1e-6), (0.0, 1e-6)];
        let mut gen = IdGenerator::new(0);
        let solid = make_extrusion(&plane, &profile, 1e-6, &mut gen).unwrap();
        assert_eq!(solid.vertices.len(), 8);
        assert!(solid.validate_manifold().is_ok());
    }

    #[test]
    fn test_too_small_rejected() {
        let plane = Plane::xy();
        let profile = vec![(0.0, 0.0), (1e-12, 0.0), (1e-12, 1e-12), (0.0, 1e-12)];
        let mut gen = IdGenerator::new(0);
        assert!(make_extrusion(&plane, &profile, 1.0, &mut gen).is_err());
    }
}
