use crate::geometry::math::LENGTH_TOLERANCE;
use crate::geometry::surface::Surface;
use crate::geometry::{Point, Vec3};

use super::partition::{project_to_face_uv, unproject_from_face_uv, FaceFragment};
use super::types::{BooleanOp, FragmentLabel};

pub struct ClassifiedFragment {
    pub fragment: FaceFragment,
    pub label: FragmentLabel,
}

pub fn classify_fragments(
    target_fragments: &[FaceFragment],
    tool_fragments: &[FaceFragment],
    target: &crate::brep::topology::Solid,
    tool: &crate::brep::topology::Solid,
    op: BooleanOp,
    _coplanar_pairs: &[(usize, usize)],
) -> Result<Vec<ClassifiedFragment>, String> {
    let mut result = Vec::new();
    let len_eps = LENGTH_TOLERANCE;

    // Classify target fragments against tool
    for frag in target_fragments {
        let label = classify_fragment_against_solid(frag, tool, len_eps)?;
        result.push(ClassifiedFragment {
            fragment: FaceFragment {
                is_tool_side: false,
                ..frag.clone()
            },
            label,
        });
    }

    // Classify tool fragments against target
    for frag in tool_fragments {
        let label = classify_fragment_against_solid(frag, target, len_eps)?;
        result.push(ClassifiedFragment {
            fragment: FaceFragment {
                is_tool_side: true,
                ..frag.clone()
            },
            label,
        });
    }

    let _ = op;
    Ok(result)
}

fn classify_fragment_against_solid(
    frag: &FaceFragment,
    other: &crate::brep::topology::Solid,
    len_eps: f64,
) -> Result<FragmentLabel, String> {
    // Coplanar detection: only Plane × Plane
    if let Surface::Plane {
        normal: frag_normal,
        origin: frag_origin,
        ..
    } = &frag.surface
    {
        for face in &other.faces {
            if let Surface::Plane {
                normal: other_normal,
                origin: other_origin,
                ..
            } = &face.surface
            {
                if frag_normal.dot(other_normal).abs() > 1.0 - len_eps {
                    let dist = (frag_origin - other_origin).dot(other_normal).abs();
                    if dist < len_eps {
                        let other_loop_verts = get_loop_vertex_points(other, face.outer_loop);
                        if polygons_have_2d_overlap(
                            &frag.polygon_3d,
                            &other_loop_verts,
                            other_normal,
                            len_eps,
                        ) {
                            let dot = frag_normal.dot(other_normal);
                            if dot > 0.0 {
                                return Ok(FragmentLabel::SharedSameDirection);
                            } else {
                                return Ok(FragmentLabel::SharedOppositeDirection);
                            }
                        }
                    }
                }
            }
        }
    }

    // Point-in-polyhedron test
    let interior_point = get_fragment_interior_point(frag, len_eps)?;

    if point_in_polyhedron(&interior_point, other, len_eps) {
        Ok(FragmentLabel::InsideOther)
    } else {
        Ok(FragmentLabel::OutsideOther)
    }
}

fn get_fragment_interior_point(frag: &FaceFragment, _len_eps: f64) -> Result<Point, String> {
    let poly = &frag.polygon_3d;
    if poly.len() < 3 {
        return Err("fragment has < 3 vertices".to_string());
    }

    let surface = &frag.surface;
    let poly2d: Vec<(f64, f64)> = poly
        .iter()
        .map(|p| project_to_face_uv(surface, p))
        .collect();
    let n = poly2d.len();

    let mut area2 = 0.0_f64;
    let mut cx = 0.0_f64;
    let mut cy = 0.0_f64;
    for i in 0..n {
        let j = (i + 1) % n;
        let cross = poly2d[i].0 * poly2d[j].1 - poly2d[j].0 * poly2d[i].1;
        area2 += cross;
        cx += (poly2d[i].0 + poly2d[j].0) * cross;
        cy += (poly2d[i].1 + poly2d[j].1) * cross;
    }

    if area2.abs() < 1e-14 {
        let mut sum = Vec3::new(0.0, 0.0, 0.0);
        for p in poly {
            sum += p.coords;
        }
        return Ok(Point::from(sum / n as f64));
    }

    let u = cx / (3.0 * area2);
    let v = cy / (3.0 * area2);
    Ok(unproject_from_face_uv(surface, u, v))
}

fn point_in_polyhedron(point: &Point, solid: &crate::brep::topology::Solid, len_eps: f64) -> bool {
    let directions = [
        Vec3::new(1.0, 0.0, 0.0),
        Vec3::new(0.0, 1.0, 0.0),
        Vec3::new(0.0, 0.0, 1.0),
    ];

    let mut odd_count = 0usize;
    for dir in &directions {
        let count = count_ray_face_intersections(point, dir, solid, len_eps);
        if count % 2 == 1 {
            odd_count += 1;
        }
    }
    odd_count >= 2
}

fn count_ray_face_intersections(
    origin: &Point,
    direction: &Vec3,
    solid: &crate::brep::topology::Solid,
    len_eps: f64,
) -> usize {
    let mut count = 0;

    for face in &solid.faces {
        let t_values = ray_intersect_surface(origin, direction, &face.surface);
        for t in t_values {
            if t < len_eps {
                continue;
            }
            let hit = origin + t * direction;

            let loop_verts = get_loop_vertex_points(solid, face.outer_loop);

            match &face.surface {
                Surface::Plane { normal, .. } => {
                    if point_in_polygon_3d(&hit, &loop_verts, normal, len_eps) {
                        count += 1;
                    }
                }
                _ => {
                    // Curved face: project to 2D using UV and do 2D point-in-polygon
                    let poly_2d: Vec<(f64, f64)> =
                        loop_verts.iter().map(|p| face.surface.uv_of(p)).collect();
                    let (hu, hv) = face.surface.uv_of(&hit);
                    if point_in_polygon_2d((hu, hv), &poly_2d) {
                        count += 1;
                    }
                }
            }
        }
    }

    count
}

/// Ray-surface intersection. Returns sorted t values where origin + t*dir is on the surface.
fn ray_intersect_surface(origin: &Point, dir: &Vec3, surface: &Surface) -> Vec<f64> {
    match surface {
        Surface::Plane {
            origin: p_origin,
            normal,
            ..
        } => {
            let denom = dir.dot(normal);
            if denom.abs() < LENGTH_TOLERANCE {
                return vec![];
            }
            let t = (p_origin - origin).dot(normal) / denom;
            if t > 0.0 {
                vec![t]
            } else {
                vec![]
            }
        }
        Surface::Cylinder {
            origin: c_origin,
            axis,
            radius,
        } => {
            let a = axis.normalize();
            // Project into plane perpendicular to axis
            let d_perp = *dir - dir.dot(&a) * a;
            let o_perp = (*origin - *c_origin) - (*origin - *c_origin).dot(&a) * a;

            let a_coeff = d_perp.dot(&d_perp);
            let b_coeff = 2.0 * o_perp.dot(&d_perp);
            let c_coeff = o_perp.dot(&o_perp) - radius * radius;

            solve_quadratic(a_coeff, b_coeff, c_coeff)
        }
        Surface::Sphere { center, radius } => {
            let oc = *origin - *center;
            let a_coeff = dir.dot(dir);
            let b_coeff = 2.0 * oc.dot(dir);
            let c_coeff = oc.dot(&oc) - radius * radius;

            solve_quadratic(a_coeff, b_coeff, c_coeff)
        }
        Surface::Cone { .. } => vec![],
    }
}

fn solve_quadratic(a: f64, b: f64, c: f64) -> Vec<f64> {
    if a.abs() < LENGTH_TOLERANCE {
        return vec![];
    }
    let disc = b * b - 4.0 * a * c;
    if disc < -LENGTH_TOLERANCE * LENGTH_TOLERANCE {
        return vec![];
    }
    if disc.abs() < LENGTH_TOLERANCE * LENGTH_TOLERANCE {
        let t = -b / (2.0 * a);
        return if t > 0.0 { vec![t] } else { vec![] };
    }
    let sqrt_disc = disc.sqrt();
    let t1 = (-b - sqrt_disc) / (2.0 * a);
    let t2 = (-b + sqrt_disc) / (2.0 * a);
    let mut ts = Vec::new();
    if t1 > 0.0 {
        ts.push(t1);
    }
    if t2 > 0.0 {
        ts.push(t2);
    }
    ts.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    ts
}

fn get_loop_vertex_points(solid: &crate::brep::topology::Solid, loop_idx: usize) -> Vec<Point> {
    let lp = &solid.loops[loop_idx];
    lp.half_edges
        .iter()
        .map(|&he_idx| {
            let he = &solid.half_edges[he_idx];
            solid.vertices[he.start_vertex].point
        })
        .collect()
}

fn point_in_polygon_3d(point: &Point, polygon: &[Point], normal: &Vec3, _len_eps: f64) -> bool {
    if polygon.len() < 3 {
        return false;
    }

    let (u_idx, v_idx) = if normal.x.abs() >= normal.y.abs() && normal.x.abs() >= normal.z.abs() {
        (1, 2)
    } else if normal.y.abs() >= normal.z.abs() {
        (0, 2)
    } else {
        (0, 1)
    };

    let px = point.coords[u_idx];
    let py = point.coords[v_idx];

    let poly_2d: Vec<(f64, f64)> = polygon
        .iter()
        .map(|p| (p.coords[u_idx], p.coords[v_idx]))
        .collect();

    point_in_polygon_2d((px, py), &poly_2d)
}

fn point_in_polygon_2d(point: (f64, f64), polygon: &[(f64, f64)]) -> bool {
    if polygon.len() < 3 {
        return false;
    }
    let n = polygon.len();
    let mut inside = false;
    let mut j = n - 1;

    for i in 0..n {
        let xi = polygon[i].0;
        let yi = polygon[i].1;
        let xj = polygon[j].0;
        let yj = polygon[j].1;

        if ((yi > point.1) != (yj > point.1))
            && (point.0 < (xj - xi) * (point.1 - yi) / (yj - yi) + xi)
        {
            inside = !inside;
        }
        j = i;
    }

    inside
}

/// Check if two 3D polygons have 2D overlap by projecting onto the dominant axis
/// of the given normal. Uses AABB intersection which is sufficient for convex inputs.
fn polygons_have_2d_overlap(
    poly_a: &[Point],
    poly_b: &[Point],
    normal: &Vec3,
    _len_eps: f64,
) -> bool {
    if poly_a.is_empty() || poly_b.is_empty() {
        return false;
    }

    let (u_idx, v_idx) = if normal.x.abs() >= normal.y.abs() && normal.x.abs() >= normal.z.abs() {
        (1, 2)
    } else if normal.y.abs() >= normal.z.abs() {
        (0, 2)
    } else {
        (0, 1)
    };

    let (min_a_u, max_a_u) = poly_a
        .iter()
        .map(|p| p.coords[u_idx])
        .fold((f64::MAX, f64::MIN), |(mn, mx), v| (mn.min(v), mx.max(v)));
    let (min_a_v, max_a_v) = poly_a
        .iter()
        .map(|p| p.coords[v_idx])
        .fold((f64::MAX, f64::MIN), |(mn, mx), v| (mn.min(v), mx.max(v)));

    let (min_b_u, max_b_u) = poly_b
        .iter()
        .map(|p| p.coords[u_idx])
        .fold((f64::MAX, f64::MIN), |(mn, mx), v| (mn.min(v), mx.max(v)));
    let (min_b_v, max_b_v) = poly_b
        .iter()
        .map(|p| p.coords[v_idx])
        .fold((f64::MAX, f64::MIN), |(mn, mx), v| (mn.min(v), mx.max(v)));

    max_a_u - min_b_u > _len_eps
        && max_b_u - min_a_u > _len_eps
        && max_a_v - min_b_v > _len_eps
        && max_b_v - min_a_v > _len_eps
}

impl Clone for FaceFragment {
    fn clone(&self) -> Self {
        FaceFragment {
            source_face_index: self.source_face_index,
            polygon_3d: self.polygon_3d.clone(),
            surface: self.surface.clone(),
            parent_name: self.parent_name.clone(),
            traversal_index: self.traversal_index,
            is_tool_side: self.is_tool_side,
            boundary_partners: self.boundary_partners.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::geometry::surface::Surface;

    fn plane_z(z: f64) -> Surface {
        Surface::Plane {
            origin: Point::new(0.0, 0.0, z),
            normal: Vec3::z(),
            u_axis: Vec3::x(),
            v_axis: Vec3::y(),
        }
    }

    #[test]
    fn t09_ray_intersect_sphere() {
        let sph = Surface::Sphere {
            center: Point::origin(),
            radius: 2.0,
        };
        let origin = Point::new(0.0, 0.0, 5.0);
        let dir = Vec3::new(0.0, 0.0, -1.0);
        let ts = ray_intersect_surface(&origin, &dir, &sph);
        assert_eq!(ts.len(), 2);
        assert!((ts[0] - 3.0).abs() < LENGTH_TOLERANCE);
        assert!((ts[1] - 7.0).abs() < LENGTH_TOLERANCE);
    }

    #[test]
    fn t10_ray_intersect_cylinder() {
        let cyl = Surface::Cylinder {
            origin: Point::origin(),
            axis: Vec3::z(),
            radius: 2.0,
        };
        let origin = Point::new(5.0, 0.0, 0.0);
        let dir = Vec3::new(-1.0, 0.0, 0.0);
        let ts = ray_intersect_surface(&origin, &dir, &cyl);
        assert_eq!(ts.len(), 2);
        assert!((ts[0] - 3.0).abs() < LENGTH_TOLERANCE);
        assert!((ts[1] - 7.0).abs() < LENGTH_TOLERANCE);
    }

    #[test]
    fn t11_ray_tangent_returns_empty() {
        let cyl = Surface::Cylinder {
            origin: Point::origin(),
            axis: Vec3::z(),
            radius: 2.0,
        };
        let origin = Point::new(2.0, 0.0, 0.0);
        let dir = Vec3::new(1.0, 0.0, 0.0);
        let ts = ray_intersect_surface(&origin, &dir, &cyl);
        assert!(ts.is_empty());
    }
}
