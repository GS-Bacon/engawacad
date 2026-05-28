use crate::geometry::math::{length_near, LENGTH_TOLERANCE};
use crate::geometry::surface::Surface;
use crate::geometry::{Point, Vec3};
use mycad_format::EntityRef;

use super::types::BooleanOp;

type Segment2D = ((f64, f64), (f64, f64));

pub struct FaceFragment {
    pub source_face_index: usize,
    pub polygon_3d: Vec<Point>,
    pub plane: PlaneData,
    pub parent_name: EntityRef,
    pub traversal_index: u32,
    pub is_tool_side: bool,
}

#[derive(Debug, Clone)]
pub struct PlaneData {
    pub origin: Point,
    pub normal: Vec3,
    pub u_axis: Vec3,
    pub v_axis: Vec3,
}

impl PlaneData {
    pub fn from_surface(surface: &Surface) -> Option<Self> {
        match surface {
            Surface::Plane {
                origin,
                normal,
                u_axis,
                v_axis,
            } => Some(PlaneData {
                origin: *origin,
                normal: *normal,
                u_axis: *u_axis,
                v_axis: *v_axis,
            }),
            _ => None,
        }
    }

    pub fn project_2d(&self, p: &Point) -> (f64, f64) {
        let d = p - self.origin;
        (d.dot(&self.u_axis), d.dot(&self.v_axis))
    }

    pub fn unproject_3d(&self, u: f64, v: f64) -> Point {
        self.origin + u * self.u_axis + v * self.v_axis
    }
}

pub struct CoplanarPair {
    pub target_face: usize,
    #[allow(dead_code)]
    pub tool_face: usize,
    /// Overlap polygon in target plane 2D coords (from sutherland_hodgman_clip).
    pub overlap_poly_2d: Vec<(f64, f64)>,
}

pub fn partition_faces(
    target: &crate::brep::topology::Solid,
    tool: &crate::brep::topology::Solid,
    _op: BooleanOp,
) -> Result<(Vec<FaceFragment>, Vec<FaceFragment>), String> {
    let len_eps = LENGTH_TOLERANCE;
    let area_eps = LENGTH_TOLERANCE * LENGTH_TOLERANCE;

    // Collect plane data for all faces
    let target_planes: Vec<PlaneData> = target
        .faces
        .iter()
        .map(|f| PlaneData::from_surface(&f.surface).unwrap())
        .collect();
    let tool_planes: Vec<PlaneData> = tool
        .faces
        .iter()
        .map(|f| PlaneData::from_surface(&f.surface).unwrap())
        .collect();

    // Get outer loop polygons for all faces (in 3D and 2D)
    let target_polygons_3d = get_all_face_polygons(target);
    let tool_polygons_3d = get_all_face_polygons(tool);

    let target_polygons_2d: Vec<Vec<(f64, f64)>> = target
        .faces
        .iter()
        .zip(target_planes.iter())
        .map(|(f, pl)| {
            let verts = get_loop_vertices(target, f.outer_loop);
            verts.iter().map(|p| pl.project_2d(p)).collect()
        })
        .collect();

    let tool_polygons_2d: Vec<Vec<(f64, f64)>> = tool
        .faces
        .iter()
        .zip(tool_planes.iter())
        .map(|(f, pl)| {
            let verts = get_loop_vertices(tool, f.outer_loop);
            verts.iter().map(|p| pl.project_2d(p)).collect()
        })
        .collect();

    // Identify coplanar pairs
    let mut coplanar_pairs: Vec<CoplanarPair> = Vec::new();
    let mut coplanar_target_set: Vec<bool> = vec![false; target.faces.len()];
    let mut coplanar_tool_set: Vec<bool> = vec![false; tool.faces.len()];

    for ti in 0..target.faces.len() {
        for ui in 0..tool.faces.len() {
            let tp = &target_planes[ti];
            let up = &tool_planes[ui];

            // Check if normals are parallel
            if !angle_near(tp.normal, up.normal) {
                continue;
            }

            // Check coplanarity
            let dist = (up.origin - tp.origin).dot(&tp.normal).abs();
            if !length_near(dist, 0.0) {
                continue;
            }

            // Check 2D overlap — project tool polygon into target plane's coordinate system
            // so both polygons are in the same 2D basis before clipping.
            let tool_poly_in_target_2d: Vec<(f64, f64)> = tool_polygons_3d[ui]
                .iter()
                .map(|p| tp.project_2d(p))
                .collect();
            let overlap = sutherland_hodgman_clip(&target_polygons_2d[ti], &tool_poly_in_target_2d);
            if overlap.len() < 3 {
                continue;
            }
            let overlap_area = signed_area_2d(&overlap).abs();
            if overlap_area <= area_eps {
                continue;
            }

            coplanar_pairs.push(CoplanarPair {
                target_face: ti,
                tool_face: ui,
                overlap_poly_2d: overlap,
            });
            coplanar_target_set[ti] = true;
            coplanar_tool_set[ui] = true;
        }
    }

    // Phase A: For each face, collect intersection segments with all non-coplanar partner faces
    let mut target_fragments = Vec::new();
    let mut tool_fragments = Vec::new();

    // Process target faces
    for fi in 0..target.faces.len() {
        // Skip faces that are coplanar with a tool face — they're handled in the coplanar pair section
        if coplanar_target_set[fi] {
            continue;
        }
        let plane = &target_planes[fi];
        let polygon_2d = &target_polygons_2d[fi];
        let polygon_3d = &target_polygons_3d[fi];
        let face = &target.faces[fi];

        // Collect intersection segments from tool faces
        let mut segments: Vec<((f64, f64), (f64, f64))> = Vec::new();

        for ui in 0..tool.faces.len() {
            if coplanar_tool_set[ui] {
                continue;
            }
            let uplane = &tool_planes[ui];
            let tool_poly_2d = &tool_polygons_2d[ui];

            // Intersect two planes
            let line = intersect_planes(plane, uplane);
            let Some((line_origin_2d, line_dir_2d)) = line else {
                continue;
            };

            // Clip line to both polygons
            let seg_opt = clip_line_to_polygon_2d(line_origin_2d, line_dir_2d, polygon_2d);
            let Some((s_start, s_end)) = seg_opt else {
                continue;
            };

            // Check segment length
            let dx = s_end.0 - s_start.0;
            let dy = s_end.1 - s_start.1;
            if (dx * dx + dy * dy).sqrt() < len_eps {
                continue;
            }

            // Also clip to tool polygon in tool 2D
            let tool_proj_start = uplane.project_2d(&plane.unproject_3d(s_start.0, s_start.1));
            let tool_proj_end = uplane.project_2d(&plane.unproject_3d(s_end.0, s_end.1));

            let tool_seg =
                clip_line_to_polygon_2d_given_points(tool_proj_start, tool_proj_end, tool_poly_2d);
            let Some((ts, te)) = tool_seg else { continue };

            // Check tool segment length in tool 2D
            let tdx = te.0 - ts.0;
            let tdy = te.1 - ts.1;
            if (tdx * tdx + tdy * tdy).sqrt() < len_eps {
                continue;
            }

            // Map back to target 2D
            let p_start_3d = uplane.unproject_3d(ts.0, ts.1);
            let p_end_3d = uplane.unproject_3d(te.0, te.1);
            let s_start_final = plane.project_2d(&p_start_3d);
            let s_end_final = plane.project_2d(&p_end_3d);

            // Re-clip to face polygon: the tool-polygon clip operates on an infinite
            // line and can extend beyond the original face boundary.
            let Some((s_start_final, s_end_final)) =
                clip_line_to_polygon_2d_given_points(s_start_final, s_end_final, polygon_2d)
            else {
                continue;
            };

            segments.push((s_start_final, s_end_final));
        }

        // Build PSLG and subdivide
        let name = face
            .name
            .clone()
            .ok_or_else(|| format!("target face {} missing name", fi))?;

        if segments.is_empty() {
            // No subdivision needed — single fragment = original face
            target_fragments.push(FaceFragment {
                source_face_index: fi,
                polygon_3d: polygon_3d.clone(),
                plane: plane.clone(),
                parent_name: name,
                traversal_index: 0,
                is_tool_side: false,
            });
        } else {
            // Subdivide the polygon using PSLG
            let sub_faces = pslg_subdivide(polygon_2d, &segments, plane);
            for (idx, sub_poly_2d) in sub_faces.into_iter().enumerate() {
                let poly_3d: Vec<Point> = sub_poly_2d
                    .iter()
                    .map(|(u, v)| plane.unproject_3d(*u, *v))
                    .collect();
                target_fragments.push(FaceFragment {
                    source_face_index: fi,
                    polygon_3d: poly_3d,
                    plane: plane.clone(),
                    parent_name: name.clone(),
                    traversal_index: idx as u32,
                    is_tool_side: false,
                });
            }
        }
    }

    // Process tool faces (symmetric)
    for fi in 0..tool.faces.len() {
        if coplanar_tool_set[fi] {
            continue;
        }
        let plane = &tool_planes[fi];
        let polygon_2d = &tool_polygons_2d[fi];
        let polygon_3d = &tool_polygons_3d[fi];
        let face = &tool.faces[fi];

        let mut segments: Vec<((f64, f64), (f64, f64))> = Vec::new();

        for ti in 0..target.faces.len() {
            if coplanar_target_set[ti] {
                continue;
            }
            let tplane = &target_planes[ti];

            let line = intersect_planes(plane, tplane);
            let Some((line_origin_2d, line_dir_2d)) = line else {
                continue;
            };

            let seg_opt = clip_line_to_polygon_2d(line_origin_2d, line_dir_2d, polygon_2d);
            let Some((s_start, s_end)) = seg_opt else {
                continue;
            };

            let dx = s_end.0 - s_start.0;
            let dy = s_end.1 - s_start.1;
            if (dx * dx + dy * dy).sqrt() < len_eps {
                continue;
            }

            let target_proj_start = tplane.project_2d(&plane.unproject_3d(s_start.0, s_start.1));
            let target_proj_end = tplane.project_2d(&plane.unproject_3d(s_end.0, s_end.1));

            let target_seg = clip_line_to_polygon_2d_given_points(
                target_proj_start,
                target_proj_end,
                &target_polygons_2d[ti],
            );
            let Some((ts, te)) = target_seg else { continue };

            let tdx = te.0 - ts.0;
            let tdy = te.1 - ts.1;
            if (tdx * tdx + tdy * tdy).sqrt() < len_eps {
                continue;
            }

            let p_start_3d = tplane.unproject_3d(ts.0, ts.1);
            let p_end_3d = tplane.unproject_3d(te.0, te.1);
            let s_start_final = plane.project_2d(&p_start_3d);
            let s_end_final = plane.project_2d(&p_end_3d);

            // Re-clip to tool face polygon for the same reason as on the target side.
            let Some((s_start_final, s_end_final)) =
                clip_line_to_polygon_2d_given_points(s_start_final, s_end_final, polygon_2d)
            else {
                continue;
            };

            segments.push((s_start_final, s_end_final));
        }

        let name = face
            .name
            .clone()
            .ok_or_else(|| format!("tool face {} missing name", fi))?;

        if segments.is_empty() {
            tool_fragments.push(FaceFragment {
                source_face_index: fi,
                polygon_3d: polygon_3d.clone(),
                plane: plane.clone(),
                parent_name: name,
                traversal_index: 0,
                is_tool_side: true,
            });
        } else {
            let sub_faces = pslg_subdivide(polygon_2d, &segments, plane);
            for (idx, sub_poly_2d) in sub_faces.into_iter().enumerate() {
                let poly_3d: Vec<Point> = sub_poly_2d
                    .iter()
                    .map(|(u, v)| plane.unproject_3d(*u, *v))
                    .collect();
                tool_fragments.push(FaceFragment {
                    source_face_index: fi,
                    polygon_3d: poly_3d,
                    plane: plane.clone(),
                    parent_name: name.clone(),
                    traversal_index: idx as u32,
                    is_tool_side: true,
                });
            }
        }
    }

    // Handle coplanar shared faces as fragments.
    // For each coplanar pair we produce:
    //   (a) The overlap polygon → classified as SharedSame/OppositeDirection downstream.
    //   (b) The remainder (target face minus overlap) → classified by point-in-polyhedron.
    //       Computed by progressively clipping the remaining polygon OUTSIDE each edge of
    //       the overlap polygon (one non-overlapping strip per overlap edge).
    for pair in &coplanar_pairs {
        let tf = &target.faces[pair.target_face];
        let tname = tf
            .name
            .clone()
            .ok_or_else(|| format!("target face {} missing name", pair.target_face))?;
        let tp = &target_planes[pair.target_face];

        // (a) Overlap polygon — must be CCW in target 2D; sutherland_hodgman_clip preserves
        // subject orientation so this is guaranteed when target_polygons_2d is CCW.
        let overlap = &pair.overlap_poly_2d;
        if overlap.len() >= 3 && signed_area_2d(overlap).abs() > area_eps {
            let poly_3d: Vec<Point> = overlap
                .iter()
                .map(|(u, v)| tp.unproject_3d(*u, *v))
                .collect();
            target_fragments.push(FaceFragment {
                source_face_index: pair.target_face,
                polygon_3d: poly_3d,
                plane: tp.clone(),
                parent_name: tname.clone(),
                traversal_index: 0,
                is_tool_side: false,
            });
        }

        // (b) Remainder strips — walk each edge of the overlap polygon; clip the
        // ever-shrinking "remaining" polygon OUTSIDE that edge to extract a strip.
        let n_overlap = overlap.len();
        let mut remaining = target_polygons_2d[pair.target_face].clone();
        let mut strip_idx: u32 = 1;

        for k in 0..n_overlap {
            if remaining.is_empty() {
                break;
            }
            let ei = overlap[k];
            let ej = overlap[(k + 1) % n_overlap];

            let strip = clip_polygon_halfplane(&remaining, ei, ej, false);
            if strip.len() >= 3 && signed_area_2d(&strip).abs() > area_eps {
                let poly_3d: Vec<Point> =
                    strip.iter().map(|(u, v)| tp.unproject_3d(*u, *v)).collect();
                target_fragments.push(FaceFragment {
                    source_face_index: pair.target_face,
                    polygon_3d: poly_3d,
                    plane: tp.clone(),
                    parent_name: tname.clone(),
                    traversal_index: strip_idx,
                    is_tool_side: false,
                });
                strip_idx += 1;
            }

            remaining = clip_polygon_halfplane(&remaining, ei, ej, true);
        }
    }

    Ok((target_fragments, tool_fragments))
}

fn angle_near(a: Vec3, b: Vec3) -> bool {
    let cross = a.cross(&b);
    let a_norm = a.norm();
    let b_norm = b.norm();
    if a_norm < LENGTH_TOLERANCE || b_norm < LENGTH_TOLERANCE {
        return false;
    }
    let sin_angle = cross.norm() / (a_norm * b_norm);
    if sin_angle < LENGTH_TOLERANCE {
        return true;
    }
    false
}

fn get_all_face_polygons(solid: &crate::brep::topology::Solid) -> Vec<Vec<Point>> {
    solid
        .faces
        .iter()
        .map(|f| get_loop_vertices(solid, f.outer_loop))
        .collect()
}

fn get_loop_vertices(solid: &crate::brep::topology::Solid, loop_idx: usize) -> Vec<Point> {
    let lp = &solid.loops[loop_idx];
    lp.half_edges
        .iter()
        .map(|&he_idx| {
            let he = &solid.half_edges[he_idx];
            solid.vertices[he.start_vertex].point
        })
        .collect()
}

fn intersect_planes(a: &PlaneData, b: &PlaneData) -> Option<((f64, f64), (f64, f64))> {
    let direction = a.normal.cross(&b.normal);
    if direction.norm() < LENGTH_TOLERANCE {
        return None; // Parallel planes
    }

    let n_dot = a.normal.dot(&b.normal);
    let denom = 1.0 - n_dot * n_dot;
    if denom.abs() < LENGTH_TOLERANCE * LENGTH_TOLERANCE {
        return None;
    }

    // Solve n_A·P = c_A, n_B·P = c_B with P = α·n_A + β·n_B.
    // α = (c_A - ndot·c_B)/denom, β = (c_B - ndot·c_A)/denom.
    let c_a = a.origin.coords.dot(&a.normal);
    let c_b = b.origin.coords.dot(&b.normal);
    let origin = Point::from(
        (c_a - n_dot * c_b) / denom * a.normal + (c_b - n_dot * c_a) / denom * b.normal,
    );

    let origin_2d = a.project_2d(&origin);
    let dir_3d = direction.normalize();
    let p2 = origin + dir_3d;
    let p2_2d = a.project_2d(&p2);
    let dir_2d = (p2_2d.0 - origin_2d.0, p2_2d.1 - origin_2d.1);
    let dir_len = (dir_2d.0 * dir_2d.0 + dir_2d.1 * dir_2d.1).sqrt();
    if dir_len < LENGTH_TOLERANCE {
        return None;
    }
    let dir_2d_norm = (dir_2d.0 / dir_len, dir_2d.1 / dir_len);

    Some((origin_2d, dir_2d_norm))
}

fn clip_line_to_polygon_2d(
    line_origin: (f64, f64),
    line_dir: (f64, f64),
    polygon: &[(f64, f64)],
) -> Option<((f64, f64), (f64, f64))> {
    let mut t_min = f64::NEG_INFINITY;
    let mut t_max = f64::INFINITY;
    let n = polygon.len();

    // (-edge.1, edge.0) is the LEFT perpendicular: outward for CW, inward for CCW.
    // For CCW polygons we need the outward (RIGHT) perpendicular instead.
    let sa = signed_area_2d(polygon);
    let flip: f64 = if sa >= 0.0 { -1.0 } else { 1.0 };

    for i in 0..n {
        let j = (i + 1) % n;
        let edge = (polygon[j].0 - polygon[i].0, polygon[j].1 - polygon[i].1);
        let edge_normal = (flip * (-edge.1), flip * edge.0); // outward normal for both CW and CCW

        let denom = line_dir.0 * edge_normal.0 + line_dir.1 * edge_normal.1;
        let num = (polygon[i].0 - line_origin.0) * edge_normal.0
            + (polygon[i].1 - line_origin.1) * edge_normal.1;

        if denom.abs() < LENGTH_TOLERANCE * LENGTH_TOLERANCE {
            // Parallel
            if num < -LENGTH_TOLERANCE {
                return None;
            }
        } else {
            let t = num / denom;
            if denom < 0.0 {
                t_min = t_min.max(t);
            } else {
                t_max = t_max.min(t);
            }
        }
    }

    if t_min > t_max + LENGTH_TOLERANCE {
        return None;
    }

    let len_sq = line_dir.0 * line_dir.0 + line_dir.1 * line_dir.1;
    if len_sq < LENGTH_TOLERANCE * LENGTH_TOLERANCE {
        return None;
    }

    let p0 = (
        line_origin.0 + t_min * line_dir.0,
        line_origin.1 + t_min * line_dir.1,
    );
    let p1 = (
        line_origin.0 + t_max * line_dir.0,
        line_origin.1 + t_max * line_dir.1,
    );

    Some((p0, p1))
}

/// Clip the **segment** [p_start, p_end] to the polygon interior.
/// Unlike `clip_line_to_polygon_2d`, this never extends beyond the input endpoints.
fn clip_line_to_polygon_2d_given_points(
    p_start: (f64, f64),
    p_end: (f64, f64),
    polygon: &[(f64, f64)],
) -> Option<((f64, f64), (f64, f64))> {
    let dx = p_end.0 - p_start.0;
    let dy = p_end.1 - p_start.1;
    let len = (dx * dx + dy * dy).sqrt();
    if len < LENGTH_TOLERANCE {
        return None;
    }
    let dir = (dx / len, dy / len);

    // Cyrus-Beck with t clamped to the segment [0, len].
    let mut t_min = 0.0_f64;
    let mut t_max = len;
    let n = polygon.len();

    let sa = signed_area_2d(polygon);
    let flip: f64 = if sa >= 0.0 { -1.0 } else { 1.0 };

    for i in 0..n {
        let j = (i + 1) % n;
        let edge = (polygon[j].0 - polygon[i].0, polygon[j].1 - polygon[i].1);
        let edge_normal = (flip * (-edge.1), flip * edge.0);

        let denom = dir.0 * edge_normal.0 + dir.1 * edge_normal.1;
        let num =
            (polygon[i].0 - p_start.0) * edge_normal.0 + (polygon[i].1 - p_start.1) * edge_normal.1;

        if denom.abs() < LENGTH_TOLERANCE * LENGTH_TOLERANCE {
            if num < -LENGTH_TOLERANCE {
                return None;
            }
        } else {
            let t = num / denom;
            if denom < 0.0 {
                t_min = t_min.max(t);
            } else {
                t_max = t_max.min(t);
            }
        }
    }

    if t_min > t_max + LENGTH_TOLERANCE {
        return None;
    }

    let len_sq = dir.0 * dir.0 + dir.1 * dir.1;
    if len_sq < LENGTH_TOLERANCE * LENGTH_TOLERANCE {
        return None;
    }

    let p0 = (p_start.0 + t_min * dir.0, p_start.1 + t_min * dir.1);
    let p1 = (p_start.0 + t_max * dir.0, p_start.1 + t_max * dir.1);

    Some((p0, p1))
}

fn signed_area_2d(poly: &[(f64, f64)]) -> f64 {
    let n = poly.len();
    let mut area = 0.0;
    for i in 0..n {
        let j = (i + 1) % n;
        area += poly[i].0 * poly[j].1;
        area -= poly[j].0 * poly[i].1;
    }
    area / 2.0
}

/// Clip a polygon to one side of the directed edge p0→p1.
/// `keep_left = true`  → keep points with cross-product ≥ 0 (left / inside).
/// `keep_left = false` → keep points with cross-product ≤ 0 (right / outside).
fn clip_polygon_halfplane(
    poly: &[(f64, f64)],
    p0: (f64, f64),
    p1: (f64, f64),
    keep_left: bool,
) -> Vec<(f64, f64)> {
    let edge = (p1.0 - p0.0, p1.1 - p0.1);
    let mut output = Vec::new();
    let n = poly.len();

    for k in 0..n {
        let l = (k + 1) % n;
        let cur = poly[k];
        let nxt = poly[l];

        let cross_c = (cur.0 - p0.0) * edge.1 - (cur.1 - p0.1) * edge.0;
        let cross_n = (nxt.0 - p0.0) * edge.1 - (nxt.1 - p0.1) * edge.0;

        let c_in = if keep_left {
            cross_c >= 0.0
        } else {
            cross_c <= 0.0
        };
        let n_in = if keep_left {
            cross_n >= 0.0
        } else {
            cross_n <= 0.0
        };

        if c_in {
            output.push(cur);
            if !n_in {
                if let Some(pt) = line_intersection_2d(cur, nxt, p0, p1) {
                    output.push(pt);
                }
            }
        } else if n_in {
            if let Some(pt) = line_intersection_2d(cur, nxt, p0, p1) {
                output.push(pt);
            }
        }
    }

    output
}

/// Sutherland-Hodgman polygon clipping
fn sutherland_hodgman_clip(subject: &[(f64, f64)], clip: &[(f64, f64)]) -> Vec<(f64, f64)> {
    let mut output = subject.to_vec();
    let n = clip.len();

    for i in 0..n {
        if output.is_empty() {
            return Vec::new();
        }
        let j = (i + 1) % n;
        let edge = (clip[j].0 - clip[i].0, clip[j].1 - clip[i].1);
        let input = std::mem::take(&mut output);

        for k in 0..input.len() {
            let l = (k + 1) % input.len();
            let current = input[k];
            let next = input[l];

            let current_inside =
                (current.0 - clip[i].0) * edge.1 - (current.1 - clip[i].1) * edge.0 >= 0.0;
            let next_inside = (next.0 - clip[i].0) * edge.1 - (next.1 - clip[i].1) * edge.0 >= 0.0;

            if current_inside {
                output.push(current);
                if !next_inside {
                    if let Some(pt) = line_intersection_2d(current, next, clip[i], clip[j]) {
                        output.push(pt);
                    }
                }
            } else if next_inside {
                if let Some(pt) = line_intersection_2d(current, next, clip[i], clip[j]) {
                    output.push(pt);
                }
            }
        }
    }

    output
}

fn line_intersection_2d(
    a1: (f64, f64),
    a2: (f64, f64),
    b1: (f64, f64),
    b2: (f64, f64),
) -> Option<(f64, f64)> {
    let d1 = (a2.0 - a1.0, a2.1 - a1.1);
    let d2 = (b2.0 - b1.0, b2.1 - b1.1);
    let denom = d1.0 * d2.1 - d1.1 * d2.0;
    if denom.abs() < LENGTH_TOLERANCE * LENGTH_TOLERANCE {
        return None;
    }
    let dx = b1.0 - a1.0;
    let dy = b1.1 - a1.1;
    let t = (dx * d2.1 - dy * d2.0) / denom;
    Some((a1.0 + t * d1.0, a1.1 + t * d1.1))
}

/// Build a PSLG from the outer loop and segments, then subdivide into sub-polygons.
fn pslg_subdivide(
    outer_loop: &[(f64, f64)],
    segments: &[Segment2D],
    _plane: &PlaneData,
) -> Vec<Vec<(f64, f64)>> {
    let len_eps = LENGTH_TOLERANCE;
    let area_eps = LENGTH_TOLERANCE * LENGTH_TOLERANCE;
    let outer_signed_area = signed_area_2d(outer_loop);

    // Collect all unique points
    let mut all_points: Vec<(f64, f64)> = outer_loop.to_vec();
    for (s, e) in segments {
        all_points.push(*s);
        all_points.push(*e);
    }

    // Add intersection points between segments
    for i in 0..segments.len() {
        for j in (i + 1)..segments.len() {
            if let Some(pt) = segment_segment_intersect_2d(
                segments[i].0,
                segments[i].1,
                segments[j].0,
                segments[j].1,
            ) {
                all_points.push(pt);
            }
        }
    }

    // Add intersections between segments and outer loop edges
    let n_outer = outer_loop.len();
    for seg in segments {
        for i in 0..n_outer {
            let j = (i + 1) % n_outer;
            if let Some(pt) =
                segment_segment_intersect_2d(seg.0, seg.1, outer_loop[i], outer_loop[j])
            {
                all_points.push(pt);
            }
        }
    }

    // Deduplicate points
    let mut unique_points: Vec<(f64, f64)> = Vec::new();
    for p in &all_points {
        let exists = unique_points
            .iter()
            .any(|u| (u.0 - p.0).abs() < len_eps && (u.1 - p.1).abs() < len_eps);
        if !exists {
            unique_points.push(*p);
        }
    }

    let find_point = |p: (f64, f64), pts: &[(f64, f64)]| -> Option<usize> {
        pts.iter()
            .position(|u| (u.0 - p.0).abs() < len_eps && (u.1 - p.1).abs() < len_eps)
    };

    // Build edges: outer loop edges + segments (split at intersections)
    let mut edges: Vec<(usize, usize)> = Vec::new();

    // Split outer loop edges at all interior points
    for i in 0..n_outer {
        let j = (i + 1) % n_outer;
        let pi = find_point(outer_loop[i], &unique_points).unwrap();
        let pj = find_point(outer_loop[j], &unique_points).unwrap();

        // Find all points along this edge
        let mut edge_points = vec![pi];
        for (k, pk) in unique_points.iter().enumerate() {
            if k == pi || k == pj {
                continue;
            }
            if point_on_segment_2d(*pk, outer_loop[i], outer_loop[j]) {
                edge_points.push(k);
            }
        }
        edge_points.push(pj);

        // Sort by parameter t along the edge
        let d = (
            outer_loop[j].0 - outer_loop[i].0,
            outer_loop[j].1 - outer_loop[i].1,
        );
        let d_len_sq = d.0 * d.0 + d.1 * d.1;
        if d_len_sq > len_eps * len_eps {
            edge_points.sort_by_key(|&k| {
                let pk = unique_points[k];
                let t =
                    ((pk.0 - outer_loop[i].0) * d.0 + (pk.1 - outer_loop[i].1) * d.1) / d_len_sq;
                (t * 1e12).round() as i64
            });
        }
        edge_points.dedup();

        for w in edge_points.windows(2) {
            edges.push((w[0], w[1]));
        }
    }

    // Split segments at all interior points
    for seg in segments {
        let si = find_point(seg.0, &unique_points).unwrap();
        let ei = find_point(seg.1, &unique_points).unwrap();

        let mut seg_points = vec![si];
        for (k, pk) in unique_points.iter().enumerate() {
            if k == si || k == ei {
                continue;
            }
            if point_on_segment_2d(*pk, seg.0, seg.1) {
                seg_points.push(k);
            }
        }
        seg_points.push(ei);

        let d = (seg.1 .0 - seg.0 .0, seg.1 .1 - seg.0 .1);
        let d_len_sq = d.0 * d.0 + d.1 * d.1;
        if d_len_sq > len_eps * len_eps {
            seg_points.sort_by_key(|&k| {
                let pk = unique_points[k];
                let t = ((pk.0 - seg.0 .0) * d.0 + (pk.1 - seg.0 .1) * d.1) / d_len_sq;
                (t * 1e12).round() as i64
            });
        }
        seg_points.dedup();

        for w in seg_points.windows(2) {
            edges.push((w[0], w[1]));
        }
    }

    // Build adjacency and walk sub-faces using DCEL-like traversal
    // Each edge (a, b) generates two half-edges: a→b and b→a
    let mut he_next: std::collections::HashMap<(usize, usize), (usize, usize)> =
        std::collections::HashMap::new();

    // For each vertex, collect all outgoing half-edges (both directions), sorted by angle
    let mut out_edges: std::collections::HashMap<usize, Vec<usize>> =
        std::collections::HashMap::new();
    for (a, b) in &edges {
        out_edges.entry(*a).or_default().push(*b);
        out_edges.entry(*b).or_default().push(*a); // both directions for DCEL correctness
    }

    // Sort outgoing edges by angle for proper face walking
    for (v, neighbors) in out_edges.iter_mut() {
        let pv = unique_points[*v];
        neighbors.sort_by(|a, b| {
            let da = (unique_points[*a].0 - pv.0, unique_points[*a].1 - pv.1);
            let db = (unique_points[*b].0 - pv.0, unique_points[*b].1 - pv.1);
            let angle_a = da.1.atan2(da.0);
            let angle_b = db.1.atan2(db.0);
            angle_a
                .partial_cmp(&angle_b)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
    }

    // Build DCEL next-pointer for every directed half-edge (both a→b and b→a per edge)
    for (a, b) in &edges {
        // Forward a→b: arriving at b, find the CCW-next outgoing edge from b
        if let Some(nbrs) = out_edges.get(b) {
            if let Some(pos) = nbrs.iter().position(|x| *x == *a) {
                let prev_pos = if pos == 0 { nbrs.len() - 1 } else { pos - 1 };
                he_next.insert((*a, *b), (*b, nbrs[prev_pos]));
            }
        }
        // Reverse b→a: arriving at a, find the CCW-next outgoing edge from a
        if let Some(nbrs) = out_edges.get(a) {
            if let Some(pos) = nbrs.iter().position(|x| *x == *b) {
                let prev_pos = if pos == 0 { nbrs.len() - 1 } else { pos - 1 };
                he_next.insert((*b, *a), (*a, nbrs[prev_pos]));
            }
        }
    }

    // Walk faces — iterate all directed half-edges (both directions per edge)
    let all_directed: Vec<(usize, usize)> = {
        let mut v: Vec<(usize, usize)> =
            edges.iter().flat_map(|&(a, b)| [(a, b), (b, a)]).collect();
        v.sort_unstable();
        v.dedup();
        v
    };

    let mut visited: std::collections::HashSet<(usize, usize)> = std::collections::HashSet::new();
    let mut sub_faces: Vec<Vec<(f64, f64)>> = Vec::new();

    for &(a, b) in &all_directed {
        if visited.contains(&(a, b)) {
            continue;
        }

        let mut face_verts = Vec::new();
        let mut cur = (a, b);
        let start = cur;
        loop {
            if visited.contains(&cur) {
                break;
            }
            visited.insert(cur);
            face_verts.push(unique_points[cur.0]);

            match he_next.get(&cur) {
                Some(next) => cur = *next,
                None => break,
            }

            if cur == start {
                break;
            }

            if face_verts.len() > unique_points.len() * 2 {
                break; // Safety: prevent infinite loop
            }
        }

        if face_verts.len() >= 3 && cur == start {
            let area = signed_area_2d(&face_verts);
            if area.abs() > area_eps {
                // Preserve the original outer_loop orientation so that edge
                // traversal directions remain consistent across adjacent faces.
                if (area < 0.0) != (outer_signed_area < 0.0) {
                    face_verts.reverse();
                }
                sub_faces.push(face_verts);
            }
        }
    }

    // The outer boundary face has the largest area; filter it out
    if sub_faces.len() > 1 {
        let outer_area: f64 = outer_loop
            .iter()
            .zip(outer_loop.iter().cycle().skip(1))
            .map(|(a, b)| a.0 * b.1 - b.0 * a.1)
            .sum::<f64>()
            .abs()
            / 2.0;

        sub_faces.retain(|f| {
            let area = signed_area_2d(f).abs();
            area < outer_area - area_eps
        });
    }

    // Sanitize: remove degenerate faces
    sub_faces.retain(|f| {
        if f.len() < 3 {
            return false;
        }
        let area = signed_area_2d(f).abs();
        area > area_eps
    });

    if sub_faces.is_empty() {
        // No subdivision actually happened
        vec![outer_loop.to_vec()]
    } else {
        sub_faces
    }
}

fn point_on_segment_2d(p: (f64, f64), a: (f64, f64), b: (f64, f64)) -> bool {
    let eps = LENGTH_TOLERANCE;
    let d = (b.0 - a.0, b.1 - a.1);
    let dp = (p.0 - a.0, p.1 - a.1);
    let cross = d.0 * dp.1 - d.1 * dp.0;
    if cross.abs() > eps * eps * 100.0 {
        return false;
    }
    let d_len_sq = d.0 * d.0 + d.1 * d.1;
    if d_len_sq < eps * eps {
        return false;
    }
    let t = (dp.0 * d.0 + dp.1 * d.1) / d_len_sq;
    t >= -eps && t <= 1.0 + eps
}

fn segment_segment_intersect_2d(
    a0: (f64, f64),
    a1: (f64, f64),
    b0: (f64, f64),
    b1: (f64, f64),
) -> Option<(f64, f64)> {
    let d1 = (a1.0 - a0.0, a1.1 - a0.1);
    let d2 = (b1.0 - b0.0, b1.1 - b0.1);
    let denom = d1.0 * d2.1 - d1.1 * d2.0;
    let eps = LENGTH_TOLERANCE;
    if denom.abs() < eps * eps {
        return None;
    }
    let dx = b0.0 - a0.0;
    let dy = b0.1 - a0.1;
    let t = (dx * d2.1 - dy * d2.0) / denom;
    let u = (dx * d1.1 - dy * d1.0) / denom;

    let margin = eps * 0.01;
    if t > margin && t < 1.0 - margin && u > margin && u < 1.0 - margin {
        Some((a0.0 + t * d1.0, a0.1 + t * d1.1))
    } else {
        None
    }
}
