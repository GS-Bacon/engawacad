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
        // Self-adjacent periodic sphere face: outer_loop has only 2 vertices (poles).
        // Use the equatorial point at u=π/2, v=0 (away from the +X seam) as interior point.
        if let Surface::Sphere { center, radius } = &frag.surface {
            return Ok(Point::new(center.x, center.y + radius, center.z));
        }
        return Err("fragment has < 3 vertices".to_string());
    }

    // Ring fragment: outer centroid may fall inside the circular hole.
    // Use the midpoint between a corner of the outer polygon and the first
    // inner polygon vertex instead — guaranteed to be in the annular region.
    if !frag.inner_polygons_3d.is_empty() && !frag.inner_polygons_3d[0].is_empty() {
        let outer_pt = &poly[0];
        let inner_pt = &frag.inner_polygons_3d[0][0];
        return Ok(Point::from((outer_pt.coords + inner_pt.coords) / 2.0));
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
                Surface::Cylinder {
                    origin: cyl_orig,
                    axis: cyl_ax,
                    ..
                } => {
                    // UV parameterization of a cylinder has a seam jump at ±π that
                    // makes the UV polygon self-intersecting. Use an axial height
                    // range test instead: a ray hit is inside the face iff its
                    // projected height v lies within [v_min, v_max] of the loop.
                    let ax = cyl_ax.normalize();
                    let v_hit = (hit.coords - cyl_orig.coords).dot(&ax);
                    let v_min = loop_verts
                        .iter()
                        .map(|p| (p.coords - cyl_orig.coords).dot(&ax))
                        .fold(f64::MAX, f64::min);
                    let v_max = loop_verts
                        .iter()
                        .map(|p| (p.coords - cyl_orig.coords).dot(&ax))
                        .fold(f64::MIN, f64::max);
                    if v_hit >= v_min - len_eps && v_hit <= v_max + len_eps {
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
    use crate::geometry::curve::Curve;
    let lp = &solid.loops[loop_idx];
    // Self-adjacent seam loops (e.g. full sphere: 2 HEs on the SAME edge) must not
    // be densified — the folded polygon has no area and breaks point-in-polygon checks.
    if lp.half_edges.len() == 2 {
        let e0 = solid.half_edges[lp.half_edges[0]].edge;
        let e1 = solid.half_edges[lp.half_edges[1]].edge;
        if e0 == e1 {
            return lp
                .half_edges
                .iter()
                .map(|&he_idx| solid.vertices[solid.half_edges[he_idx].start_vertex].point)
                .collect();
        }
    }
    let mut pts = Vec::new();
    for &he_idx in &lp.half_edges {
        let he = &solid.half_edges[he_idx];
        let edge = &solid.edges[he.edge];
        pts.push(solid.vertices[he.start_vertex].point);
        if let Curve::Circle { .. } = &edge.curve {
            let [t0, t1] = edge.t_range;
            let (ta, tb) = if he.forward { (t0, t1) } else { (t1, t0) };
            const N: usize = 64;
            for k in 1..N {
                let t = ta + (tb - ta) * k as f64 / N as f64;
                pts.push(edge.curve.evaluate(t));
            }
        }
    }
    pts
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
            inner_polygons_3d: self.inner_polygons_3d.clone(),
            surface: self.surface.clone(),
            parent_name: self.parent_name.clone(),
            traversal_index: self.traversal_index,
            is_tool_side: self.is_tool_side,
            boundary_partners: self.boundary_partners.clone(),
            boundary_curves: self.boundary_curves.clone(),
            boundary_t_ranges: self.boundary_t_ranges.clone(),
            boundary_pcurves_a: self.boundary_pcurves_a.clone(),
            boundary_pcurves_b: self.boundary_pcurves_b.clone(),
            inner_boundary_partners: self.inner_boundary_partners.clone(),
            inner_boundary_curves: self.inner_boundary_curves.clone(),
            inner_boundary_t_ranges: self.inner_boundary_t_ranges.clone(),
            inner_boundary_pcurves_a: self.inner_boundary_pcurves_a.clone(),
            inner_boundary_pcurves_b: self.inner_boundary_pcurves_b.clone(),
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

    fn make_test_frag(
        polygon_3d: Vec<Point>,
        surface: Surface,
        is_tool_side: bool,
    ) -> crate::booleans::partition::FaceFragment {
        use mycad_format::{EntityKind, EntityRef};
        let n = polygon_3d.len();
        crate::booleans::partition::FaceFragment {
            source_face_index: 0,
            polygon_3d,
            inner_polygons_3d: vec![],
            surface,
            parent_name: EntityRef::try_named("s", EntityKind::Face, "f").unwrap(),
            traversal_index: 0,
            is_tool_side,
            boundary_partners: vec![None; n],
            boundary_curves: vec![None; n],
            boundary_t_ranges: vec![[0.0, 1.0]; n],
            boundary_pcurves_a: vec![None; n],
            boundary_pcurves_b: vec![None; n],
            inner_boundary_partners: vec![],
            inner_boundary_curves: vec![],
            inner_boundary_t_ranges: vec![],
            inner_boundary_pcurves_a: vec![],
            inner_boundary_pcurves_b: vec![],
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

    #[test]
    fn t14b_sphere_fragment_interior_point() {
        let frag = make_test_frag(
            vec![Point::new(0.0, 0.0, -3.0), Point::new(0.0, 0.0, 3.0)],
            Surface::Sphere {
                center: Point::origin(),
                radius: 3.0,
            },
            true,
        );
        let result = get_fragment_interior_point(&frag, 1e-9);
        assert!(result.is_ok(), "sphere face interior point must succeed");
        let pt = result.unwrap();
        let dist = pt.coords.norm();
        assert!(
            (dist - 3.0).abs() < 1e-9,
            "interior point must be on sphere surface, got dist={dist}"
        );
    }

    /// TX3 — sphere fragment classify returns InsideOther when inside a box
    #[test]
    fn tx3_sphere_fragment_classify_inside_box() {
        let frag = make_test_frag(
            vec![Point::new(0.0, 0.0, -3.0), Point::new(0.0, 0.0, 3.0)],
            Surface::Sphere {
                center: Point::origin(),
                radius: 3.0,
            },
            true,
        );

        let mut gen = crate::brep::topology::IdGenerator::new(0);
        let box_solid = crate::primitives::make_cuboid(10.0, 10.0, 10.0, &mut gen).unwrap();

        let result = classify_fragment_against_solid(&frag, &box_solid, LENGTH_TOLERANCE);
        assert!(result.is_ok(), "classify should succeed: {:?}", result);
        assert_eq!(
            result.unwrap(),
            FragmentLabel::InsideOther,
            "sphere inside box should be InsideOther"
        );
    }

    /// TX5 — sphere interior point for non-origin center
    #[test]
    fn tx5_sphere_fragment_interior_point_non_origin() {
        let frag = make_test_frag(
            vec![Point::new(3.0, 4.0, 3.0), Point::new(3.0, 4.0, 7.0)],
            Surface::Sphere {
                center: Point::new(3.0, 4.0, 5.0),
                radius: 2.0,
            },
            true,
        );

        let result = get_fragment_interior_point(&frag, LENGTH_TOLERANCE);
        assert!(result.is_ok(), "should compute interior point");
        let pt = result.unwrap();
        assert!((pt.x - 3.0).abs() < 1e-9, "x should be 3.0, got {}", pt.x);
        assert!((pt.y - 6.0).abs() < 1e-9, "y should be 6.0, got {}", pt.y);
        assert!((pt.z - 5.0).abs() < 1e-9, "z should be 5.0, got {}", pt.z);
    }
}
