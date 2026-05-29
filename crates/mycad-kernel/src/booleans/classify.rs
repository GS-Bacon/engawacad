use crate::geometry::math::LENGTH_TOLERANCE;
use crate::geometry::{Point, Vec3};

use super::partition::{FaceFragment, PlaneData};
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

    // Get target and tool face planes for coplanar detection
    let target_planes: Vec<Option<PlaneData>> = target
        .faces
        .iter()
        .map(|f| PlaneData::from_surface(&f.surface))
        .collect();
    let tool_planes: Vec<Option<PlaneData>> = tool
        .faces
        .iter()
        .map(|f| PlaneData::from_surface(&f.surface))
        .collect();

    // Classify target fragments against tool
    for frag in target_fragments {
        let label = classify_fragment_against_solid(frag, tool, &tool_planes, len_eps)?;
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
        let label = classify_fragment_against_solid(frag, target, &target_planes, len_eps)?;
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
    other_planes: &[Option<PlaneData>],
    len_eps: f64,
) -> Result<FragmentLabel, String> {
    // Check if this fragment's face is coplanar with any face in other solid
    for (fi, other_plane_opt) in other_planes.iter().enumerate() {
        let Some(other_plane) = other_plane_opt else {
            continue;
        };

        if frag.plane.normal.dot(&other_plane.normal).abs() > 1.0 - len_eps {
            let dist = (frag.plane.origin - other_plane.origin)
                .dot(&other_plane.normal)
                .abs();
            if dist < len_eps {
                // Coplanar candidate — verify 2D polygon overlap before classifying
                let other_loop_verts = get_loop_vertex_points(other, other.faces[fi].outer_loop);
                if polygons_have_2d_overlap(
                    &frag.polygon_3d,
                    &other_loop_verts,
                    &other_plane.normal,
                    len_eps,
                ) {
                    let dot = frag.plane.normal.dot(&other_plane.normal);
                    if dot > 0.0 {
                        return Ok(FragmentLabel::SharedSameDirection);
                    } else {
                        return Ok(FragmentLabel::SharedOppositeDirection);
                    }
                }
                // No overlap — not truly coplanar with this face, continue checking
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

    // Use the true area centroid (not vertex average) so that non-convex fragments
    // (e.g. an annular region whose vertex mean falls in the "hole") classify correctly.
    let plane = &frag.plane;
    let poly2d: Vec<(f64, f64)> = poly.iter().map(|p| plane.project_2d(p)).collect();
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
        // Degenerate: fall back to vertex average
        let mut sum = Vec3::new(0.0, 0.0, 0.0);
        for p in poly {
            sum += p.coords;
        }
        return Ok(Point::from(sum / n as f64));
    }

    let u = cx / (3.0 * area2);
    let v = cy / (3.0 * area2);
    Ok(plane.unproject_3d(u, v))
}

fn point_in_polyhedron(point: &Point, solid: &crate::brep::topology::Solid, len_eps: f64) -> bool {
    // Use majority vote over 3 axis-aligned rays. A single ray can misfire when
    // the test point lies on or very near a face/edge of the solid.
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
        let loop_verts = get_loop_vertex_points(solid, face.outer_loop);
        if loop_verts.len() < 3 {
            continue;
        }

        // Get plane of the face
        let (plane_origin, plane_normal) = match &face.surface {
            crate::geometry::surface::Surface::Plane { origin, normal, .. } => (*origin, *normal),
            _ => continue,
        };

        let denom = direction.dot(&plane_normal);
        if denom.abs() < len_eps {
            continue; // Ray parallel to face
        }

        let t = (plane_origin - origin).dot(&plane_normal) / denom;
        // Skip intersections at t ≈ 0: the test point is on or touching this face.
        if t < len_eps {
            continue;
        }

        // Intersection point
        let hit = origin + t * direction;

        // Check if hit is inside the face polygon (2D projection)
        if point_in_polygon_3d(&hit, &loop_verts, &plane_normal, len_eps) {
            count += 1;
        }
    }

    count
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

    // Project to 2D using the dominant axis of the normal
    let (u_idx, v_idx) = if normal.x.abs() >= normal.y.abs() && normal.x.abs() >= normal.z.abs() {
        (1, 2) // YZ plane
    } else if normal.y.abs() >= normal.z.abs() {
        (0, 2) // XZ plane
    } else {
        (0, 1) // XY plane
    };

    let px = point.coords[u_idx];
    let py = point.coords[v_idx];

    let poly_2d: Vec<(f64, f64)> = polygon
        .iter()
        .map(|p| (p.coords[u_idx], p.coords[v_idx]))
        .collect();

    // Ray casting in 2D
    let n = poly_2d.len();
    let mut inside = false;
    let mut j = n - 1;

    for i in 0..n {
        let xi = poly_2d[i].0;
        let yi = poly_2d[i].1;
        let xj = poly_2d[j].0;
        let yj = poly_2d[j].1;

        if ((yi > py) != (yj > py)) && (px < (xj - xi) * (py - yi) / (yj - yi) + xi) {
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

    // Compute AABB for poly_a
    let (min_a_u, max_a_u) = poly_a
        .iter()
        .map(|p| p.coords[u_idx])
        .fold((f64::MAX, f64::MIN), |(mn, mx), v| (mn.min(v), mx.max(v)));
    let (min_a_v, max_a_v) = poly_a
        .iter()
        .map(|p| p.coords[v_idx])
        .fold((f64::MAX, f64::MIN), |(mn, mx), v| (mn.min(v), mx.max(v)));

    // Compute AABB for poly_b
    let (min_b_u, max_b_u) = poly_b
        .iter()
        .map(|p| p.coords[u_idx])
        .fold((f64::MAX, f64::MIN), |(mn, mx), v| (mn.min(v), mx.max(v)));
    let (min_b_v, max_b_v) = poly_b
        .iter()
        .map(|p| p.coords[v_idx])
        .fold((f64::MAX, f64::MIN), |(mn, mx), v| (mn.min(v), mx.max(v)));

    // AABB intersection test (strict: touching at a single edge doesn't count as overlap)
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
            plane: self.plane.clone(),
            parent_name: self.parent_name.clone(),
            traversal_index: self.traversal_index,
            is_tool_side: self.is_tool_side,
            boundary_partners: self.boundary_partners.clone(),
        }
    }
}
