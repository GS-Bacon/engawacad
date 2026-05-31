use crate::geometry::math::{length_near, LENGTH_TOLERANCE};
use crate::geometry::surface::Surface;
use crate::geometry::{Point, Vec3};
use mycad_format::EntityRef;

use super::types::BooleanOp;

/// An intersection segment with provenance tracking (per-edge partner).
#[derive(Clone)]
pub struct IntersectionSegment {
    pub p_start: (f64, f64),
    pub p_end: (f64, f64),
    pub partner: EntityRef,
}

pub struct FaceFragment {
    pub source_face_index: usize,
    pub polygon_3d: Vec<Point>,
    pub surface: Surface,
    pub parent_name: EntityRef,
    pub traversal_index: u32,
    pub is_tool_side: bool,
    /// Per-edge partner provenance. Edge i connects polygon_3d[i] → polygon_3d[(i+1)%n].
    /// Some(partner) means this edge was created by an intersection with the partner face.
    /// None means it originated from the outer loop of the source face.
    pub boundary_partners: Vec<Option<EntityRef>>,
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

/// Project a 3D point to 2D coordinates in the surface's parameter space.
pub fn project_to_face_uv(surface: &Surface, p: &Point) -> (f64, f64) {
    match surface {
        Surface::Plane {
            origin,
            u_axis,
            v_axis,
            ..
        } => {
            let d = p - *origin;
            (d.dot(u_axis), d.dot(v_axis))
        }
        _ => surface.uv_of(p),
    }
}

/// Reconstruct a 3D point from 2D coordinates in the surface's parameter space.
pub fn unproject_from_face_uv(surface: &Surface, u: f64, v: f64) -> Point {
    match surface {
        Surface::Plane {
            origin,
            u_axis,
            v_axis,
            ..
        } => *origin + u * u_axis + v * v_axis,
        _ => surface.evaluate(u, v),
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

    // Collect plane data for all faces (None for non-planar surfaces)
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

    // Get outer loop polygons for all faces (in 3D and 2D)
    let target_polygons_3d = get_all_face_polygons(target);
    let tool_polygons_3d = get_all_face_polygons(tool);

    let target_polygons_2d: Vec<Vec<(f64, f64)>> = target
        .faces
        .iter()
        .map(|f| {
            let verts = get_loop_vertices(target, f.outer_loop);
            verts
                .iter()
                .map(|p| project_to_face_uv(&f.surface, p))
                .collect()
        })
        .collect();

    let tool_polygons_2d: Vec<Vec<(f64, f64)>> = tool
        .faces
        .iter()
        .map(|f| {
            let verts = get_loop_vertices(tool, f.outer_loop);
            verts
                .iter()
                .map(|p| project_to_face_uv(&f.surface, p))
                .collect()
        })
        .collect();

    // Identify coplanar pairs
    let mut coplanar_pairs: Vec<CoplanarPair> = Vec::new();
    let mut coplanar_target_set: Vec<bool> = vec![false; target.faces.len()];
    let mut coplanar_tool_set: Vec<bool> = vec![false; tool.faces.len()];

    for ti in 0..target.faces.len() {
        for ui in 0..tool.faces.len() {
            let Some(tp) = &target_planes[ti] else {
                continue;
            };
            let Some(up) = &tool_planes[ui] else { continue };

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
        if coplanar_target_set[fi] {
            continue;
        }
        let plane = &target_planes[fi];
        let polygon_2d = &target_polygons_2d[fi];
        let polygon_3d = &target_polygons_3d[fi];
        let face = &target.faces[fi];
        let surface = &face.surface;

        let mut segments: Vec<IntersectionSegment> = Vec::new();

        for ui in 0..tool.faces.len() {
            if coplanar_tool_set[ui] {
                continue;
            }
            let _uplane = &tool_planes[ui];
            let tool_poly_2d = &tool_polygons_2d[ui];
            let tool_surface = &tool.faces[ui].surface;
            let tool_face_name = tool.faces[ui]
                .name
                .clone()
                .ok_or_else(|| format!("tool face {} missing name", ui))?;

            // Skip cylinder×sphere face pairs — deferred to #39
            let is_cyl = matches!(surface, Surface::Cylinder { .. });
            let is_sph = matches!(surface, Surface::Sphere { .. });
            let tool_is_cyl = matches!(tool_surface, Surface::Cylinder { .. });
            let tool_is_sph = matches!(tool_surface, Surface::Sphere { .. });
            if (is_cyl && tool_is_sph) || (is_sph && tool_is_cyl) {
                continue;
            }

            // Intersect surfaces via the unified dispatcher
            let isect_result =
                crate::geometry::surface_intersect::intersect_surfaces(surface, tool_surface);
            let Ok(isect_loops) = isect_result else {
                continue;
            };
            if isect_loops.is_empty() {
                continue;
            }

            // Convert each intersection loop to line segments for the PSLG
            for iloop in &isect_loops {
                match &iloop.curve_3d {
                    crate::geometry::curve::Curve::Line { .. } => {
                        // Use pcurve_on_a to get 2D coordinates in target face's UV space
                        let line_2d = &iloop.pcurve_on_a;
                        let t_range = iloop.pcurve_on_a_t_range;
                        let p0 = line_2d.evaluate(t_range[0]);
                        let p1 = line_2d.evaluate(t_range[1]);

                        // Clip line to target polygon
                        let dir_2d = (p1.0 - p0.0, p1.1 - p0.1);
                        let dir_len = (dir_2d.0 * dir_2d.0 + dir_2d.1 * dir_2d.1).sqrt();
                        if dir_len < len_eps {
                            continue;
                        }
                        let dir_norm = (dir_2d.0 / dir_len, dir_2d.1 / dir_len);
                        let seg_opt = clip_line_to_polygon_2d(p0, dir_norm, polygon_2d);
                        let Some((s_start, s_end)) = seg_opt else {
                            continue;
                        };

                        let dx = s_end.0 - s_start.0;
                        let dy = s_end.1 - s_start.1;
                        if (dx * dx + dy * dy).sqrt() < len_eps {
                            continue;
                        }

                        // Clip to tool polygon in tool 2D
                        let tool_proj_start = project_to_face_uv(
                            tool_surface,
                            &unproject_from_face_uv(surface, s_start.0, s_start.1),
                        );
                        let tool_proj_end = project_to_face_uv(
                            tool_surface,
                            &unproject_from_face_uv(surface, s_end.0, s_end.1),
                        );

                        let tool_seg = clip_line_to_polygon_2d_given_points(
                            tool_proj_start,
                            tool_proj_end,
                            tool_poly_2d,
                        );
                        let Some((ts, te)) = tool_seg else { continue };

                        let tdx = te.0 - ts.0;
                        let tdy = te.1 - ts.1;
                        if (tdx * tdx + tdy * tdy).sqrt() < len_eps {
                            continue;
                        }

                        // Map back to target 2D
                        let p_start_3d = unproject_from_face_uv(tool_surface, ts.0, ts.1);
                        let p_end_3d = unproject_from_face_uv(tool_surface, te.0, te.1);
                        let s_start_final = project_to_face_uv(surface, &p_start_3d);
                        let s_end_final = project_to_face_uv(surface, &p_end_3d);

                        let Some((s_start_final, s_end_final)) =
                            clip_line_to_polygon_2d_given_points(
                                s_start_final,
                                s_end_final,
                                polygon_2d,
                            )
                        else {
                            continue;
                        };

                        segments.push(IntersectionSegment {
                            p_start: s_start_final,
                            p_end: s_end_final,
                            partner: tool_face_name.clone(),
                        });
                    }
                    crate::geometry::curve::Curve::Circle { .. } => {
                        // Circle intersections handled in later steps
                        continue;
                    }
                }
            }
        }

        // Build PSLG and subdivide
        let name = face
            .name
            .clone()
            .ok_or_else(|| format!("target face {} missing name", fi))?;

        if segments.is_empty() {
            target_fragments.push(FaceFragment {
                source_face_index: fi,
                polygon_3d: polygon_3d.clone(),
                surface: surface.clone(),
                parent_name: name,
                traversal_index: 0,
                is_tool_side: false,
                boundary_partners: vec![None; polygon_3d.len()],
            });
        } else if let Some(plane_data) = plane {
            let sub_faces = pslg_subdivide(polygon_2d, &segments, plane_data);
            for (idx, (sub_poly_2d, edge_partners)) in sub_faces.into_iter().enumerate() {
                let poly_3d: Vec<Point> = sub_poly_2d
                    .iter()
                    .map(|(u, v)| unproject_from_face_uv(surface, *u, *v))
                    .collect();
                target_fragments.push(FaceFragment {
                    source_face_index: fi,
                    polygon_3d: poly_3d,
                    surface: surface.clone(),
                    parent_name: name.clone(),
                    traversal_index: idx as u32,
                    is_tool_side: false,
                    boundary_partners: edge_partners,
                });
            }
        }
    }

    // Process tool faces (symmetric)
    for fi in 0..tool.faces.len() {
        if coplanar_tool_set[fi] {
            continue;
        }
        let polygon_2d = &tool_polygons_2d[fi];
        let polygon_3d = &tool_polygons_3d[fi];
        let face = &tool.faces[fi];
        let surface = &face.surface;

        let mut segments: Vec<IntersectionSegment> = Vec::new();

        for ti in 0..target.faces.len() {
            if coplanar_target_set[ti] {
                continue;
            }
            let target_face_name = target.faces[ti]
                .name
                .clone()
                .ok_or_else(|| format!("target face {} missing name", ti))?;

            // Skip cylinder×sphere face pairs — deferred to #39
            let target_surface = &target.faces[ti].surface;
            let is_cyl = matches!(surface, Surface::Cylinder { .. });
            let is_sph = matches!(surface, Surface::Sphere { .. });
            let target_is_cyl = matches!(target_surface, Surface::Cylinder { .. });
            let target_is_sph = matches!(target_surface, Surface::Sphere { .. });
            if (is_cyl && target_is_sph) || (is_sph && target_is_cyl) {
                continue;
            }

            let isect_result =
                crate::geometry::surface_intersect::intersect_surfaces(surface, target_surface);
            let Ok(isect_loops) = isect_result else {
                continue;
            };
            if isect_loops.is_empty() {
                continue;
            }

            for iloop in &isect_loops {
                match &iloop.curve_3d {
                    crate::geometry::curve::Curve::Line { .. } => {
                        let line_2d = &iloop.pcurve_on_a;
                        let t_range = iloop.pcurve_on_a_t_range;
                        let p0 = line_2d.evaluate(t_range[0]);
                        let p1 = line_2d.evaluate(t_range[1]);

                        let dir_2d = (p1.0 - p0.0, p1.1 - p0.1);
                        let dir_len = (dir_2d.0 * dir_2d.0 + dir_2d.1 * dir_2d.1).sqrt();
                        if dir_len < len_eps {
                            continue;
                        }
                        let dir_norm = (dir_2d.0 / dir_len, dir_2d.1 / dir_len);
                        let seg_opt = clip_line_to_polygon_2d(p0, dir_norm, polygon_2d);
                        let Some((s_start, s_end)) = seg_opt else {
                            continue;
                        };

                        let dx = s_end.0 - s_start.0;
                        let dy = s_end.1 - s_start.1;
                        if (dx * dx + dy * dy).sqrt() < len_eps {
                            continue;
                        }

                        let target_surface = &target.faces[ti].surface;
                        let target_poly_2d = &target_polygons_2d[ti];

                        let tool_proj_start = project_to_face_uv(
                            target_surface,
                            &unproject_from_face_uv(surface, s_start.0, s_start.1),
                        );
                        let tool_proj_end = project_to_face_uv(
                            target_surface,
                            &unproject_from_face_uv(surface, s_end.0, s_end.1),
                        );

                        let target_seg = clip_line_to_polygon_2d_given_points(
                            tool_proj_start,
                            tool_proj_end,
                            target_poly_2d,
                        );
                        let Some((ts, te)) = target_seg else { continue };

                        let tdx = te.0 - ts.0;
                        let tdy = te.1 - ts.1;
                        if (tdx * tdx + tdy * tdy).sqrt() < len_eps {
                            continue;
                        }

                        let p_start_3d = unproject_from_face_uv(target_surface, ts.0, ts.1);
                        let p_end_3d = unproject_from_face_uv(target_surface, te.0, te.1);
                        let s_start_final = project_to_face_uv(surface, &p_start_3d);
                        let s_end_final = project_to_face_uv(surface, &p_end_3d);

                        let Some((s_start_final, s_end_final)) =
                            clip_line_to_polygon_2d_given_points(
                                s_start_final,
                                s_end_final,
                                polygon_2d,
                            )
                        else {
                            continue;
                        };

                        segments.push(IntersectionSegment {
                            p_start: s_start_final,
                            p_end: s_end_final,
                            partner: target_face_name.clone(),
                        });
                    }
                    crate::geometry::curve::Curve::Circle { .. } => {
                        continue;
                    }
                }
            }
        }

        let name = face
            .name
            .clone()
            .ok_or_else(|| format!("tool face {} missing name", fi))?;

        if segments.is_empty() {
            tool_fragments.push(FaceFragment {
                source_face_index: fi,
                polygon_3d: polygon_3d.clone(),
                surface: surface.clone(),
                parent_name: name,
                traversal_index: 0,
                is_tool_side: true,
                boundary_partners: vec![None; polygon_3d.len()],
            });
        } else if let Some(plane_data) = PlaneData::from_surface(surface) {
            let sub_faces = pslg_subdivide(polygon_2d, &segments, &plane_data);
            for (idx, (sub_poly_2d, edge_partners)) in sub_faces.into_iter().enumerate() {
                let poly_3d: Vec<Point> = sub_poly_2d
                    .iter()
                    .map(|(u, v)| unproject_from_face_uv(surface, *u, *v))
                    .collect();
                tool_fragments.push(FaceFragment {
                    source_face_index: fi,
                    polygon_3d: poly_3d,
                    surface: surface.clone(),
                    parent_name: name.clone(),
                    traversal_index: idx as u32,
                    is_tool_side: true,
                    boundary_partners: edge_partners,
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
        let Some(tp) = &target_planes[pair.target_face] else {
            continue;
        };
        let t_surface = &tf.surface;

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
                polygon_3d: poly_3d.clone(),
                surface: t_surface.clone(),
                parent_name: tname.clone(),
                traversal_index: 0,
                is_tool_side: false,
                boundary_partners: vec![None; poly_3d.len()],
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
                    polygon_3d: poly_3d.clone(),
                    surface: t_surface.clone(),
                    parent_name: tname.clone(),
                    traversal_index: strip_idx,
                    is_tool_side: false,
                    boundary_partners: vec![None; poly_3d.len()],
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

/// Per-edge partner provenance for pslg_subdivide output.
type EdgePartners = Vec<Option<EntityRef>>;

/// Sub-polygon result from PSLG subdivision.
type SubFaceResult = (Vec<(f64, f64)>, EdgePartners);

/// Build a PSLG from the outer loop and segments, then subdivide into sub-polygons.
/// Returns (polygon_2d, per_edge_partners) pairs. Edge i of polygon goes from vertex[i] to vertex[(i+1)%n].
/// partner is Some(partner_face_name) if the edge originated from an intersection segment.
fn pslg_subdivide(
    outer_loop: &[(f64, f64)],
    segments: &[IntersectionSegment],
    _plane: &PlaneData,
) -> Vec<SubFaceResult> {
    let len_eps = LENGTH_TOLERANCE;
    let area_eps = LENGTH_TOLERANCE * LENGTH_TOLERANCE;
    let outer_signed_area = signed_area_2d(outer_loop);

    // Collect all unique points
    let mut all_points: Vec<(f64, f64)> = outer_loop.to_vec();
    for seg in segments {
        all_points.push(seg.p_start);
        all_points.push(seg.p_end);
    }

    // Add intersection points between segments
    for i in 0..segments.len() {
        for j in (i + 1)..segments.len() {
            if let Some(pt) = segment_segment_intersect_2d(
                segments[i].p_start,
                segments[i].p_end,
                segments[j].p_start,
                segments[j].p_end,
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
                segment_segment_intersect_2d(seg.p_start, seg.p_end, outer_loop[i], outer_loop[j])
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
    // Also track per-edge partner provenance for intersection segment edges.
    let mut edges: Vec<(usize, usize)> = Vec::new();
    let mut edge_partner: std::collections::HashMap<[usize; 2], EntityRef> =
        std::collections::HashMap::new();
    let mut segment_edge_set: std::collections::HashSet<[usize; 2]> =
        std::collections::HashSet::new();

    // Split outer loop edges at all interior points
    for i in 0..n_outer {
        let j = (i + 1) % n_outer;
        let pi = find_point(outer_loop[i], &unique_points).unwrap();
        let pj = find_point(outer_loop[j], &unique_points).unwrap();

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

    // Split segments at all interior points — tag with partner provenance
    for seg in segments {
        let si = find_point(seg.p_start, &unique_points).unwrap();
        let ei = find_point(seg.p_end, &unique_points).unwrap();

        let mut seg_points = vec![si];
        for (k, pk) in unique_points.iter().enumerate() {
            if k == si || k == ei {
                continue;
            }
            if point_on_segment_2d(*pk, seg.p_start, seg.p_end) {
                seg_points.push(k);
            }
        }
        seg_points.push(ei);

        let d = (seg.p_end.0 - seg.p_start.0, seg.p_end.1 - seg.p_start.1);
        let d_len_sq = d.0 * d.0 + d.1 * d.1;
        if d_len_sq > len_eps * len_eps {
            seg_points.sort_by_key(|&k| {
                let pk = unique_points[k];
                let t = ((pk.0 - seg.p_start.0) * d.0 + (pk.1 - seg.p_start.1) * d.1) / d_len_sq;
                (t * 1e12).round() as i64
            });
        }
        seg_points.dedup();

        for w in seg_points.windows(2) {
            let key = if w[0] < w[1] {
                [w[0], w[1]]
            } else {
                [w[1], w[0]]
            };
            segment_edge_set.insert(key);
            edge_partner.insert(key, seg.partner.clone());
            edges.push((w[0], w[1]));
        }
    }

    // Build adjacency and walk sub-faces using DCEL-like traversal
    let mut he_next: std::collections::HashMap<(usize, usize), (usize, usize)> =
        std::collections::HashMap::new();

    let mut out_edges: std::collections::HashMap<usize, Vec<usize>> =
        std::collections::HashMap::new();
    for (a, b) in &edges {
        out_edges.entry(*a).or_default().push(*b);
        out_edges.entry(*b).or_default().push(*a);
    }

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

    for (a, b) in &edges {
        if let Some(nbrs) = out_edges.get(b) {
            if let Some(pos) = nbrs.iter().position(|x| *x == *a) {
                let prev_pos = if pos == 0 { nbrs.len() - 1 } else { pos - 1 };
                he_next.insert((*a, *b), (*b, nbrs[prev_pos]));
            }
        }
        if let Some(nbrs) = out_edges.get(a) {
            if let Some(pos) = nbrs.iter().position(|x| *x == *b) {
                let prev_pos = if pos == 0 { nbrs.len() - 1 } else { pos - 1 };
                he_next.insert((*b, *a), (*a, nbrs[prev_pos]));
            }
        }
    }

    let all_directed: Vec<(usize, usize)> = {
        let mut v: Vec<(usize, usize)> =
            edges.iter().flat_map(|&(a, b)| [(a, b), (b, a)]).collect();
        v.sort_unstable();
        v.dedup();
        v
    };

    let mut visited: std::collections::HashSet<(usize, usize)> = std::collections::HashSet::new();
    type FaceRaw = (Vec<(f64, f64)>, Vec<(usize, usize)>);
    let mut sub_faces_raw: Vec<FaceRaw> = Vec::new();

    for &(a, b) in &all_directed {
        if visited.contains(&(a, b)) {
            continue;
        }

        let mut face_verts = Vec::new();
        let mut face_dir_edges = Vec::new();
        let mut cur = (a, b);
        let start = cur;
        loop {
            if visited.contains(&cur) {
                break;
            }
            visited.insert(cur);
            face_verts.push(unique_points[cur.0]);
            face_dir_edges.push(cur);

            match he_next.get(&cur) {
                Some(next) => cur = *next,
                None => break,
            }

            if cur == start {
                break;
            }

            if face_verts.len() > unique_points.len() * 2 {
                break;
            }
        }

        if face_verts.len() >= 3 && cur == start {
            let area = signed_area_2d(&face_verts);
            if area.abs() > area_eps {
                if (area < 0.0) != (outer_signed_area < 0.0) {
                    face_verts.reverse();
                    face_dir_edges.reverse();
                }
                sub_faces_raw.push((face_verts, face_dir_edges));
            }
        }
    }

    // The outer boundary face has the largest area; filter it out
    if sub_faces_raw.len() > 1 {
        let outer_area: f64 = outer_loop
            .iter()
            .zip(outer_loop.iter().cycle().skip(1))
            .map(|(a, b)| a.0 * b.1 - b.0 * a.1)
            .sum::<f64>()
            .abs()
            / 2.0;

        sub_faces_raw.retain(|(f, _)| {
            let area = signed_area_2d(f).abs();
            area < outer_area - area_eps
        });
    }

    sub_faces_raw.retain(|(f, _)| {
        if f.len() < 3 {
            return false;
        }
        let area = signed_area_2d(f).abs();
        area > area_eps
    });

    if sub_faces_raw.is_empty() {
        let n = outer_loop.len();
        vec![(outer_loop.to_vec(), vec![None; n])]
    } else {
        sub_faces_raw
            .into_iter()
            .map(|(verts, dir_edges)| {
                let partners: EdgePartners = dir_edges
                    .iter()
                    .map(|&(a, b)| {
                        let key = if a < b { [a, b] } else { [b, a] };
                        edge_partner.get(&key).cloned()
                    })
                    .collect();
                (verts, partners)
            })
            .collect()
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::brep::topology::IdGenerator;
    use crate::brep::topology::Solid;
    use crate::geometry::surface::Surface;
    use crate::primitives::{make_cuboid, make_cylinder, make_sphere};
    use mycad_format::{EntityKind, EntityRef};

    fn ensure_names(solid: &mut Solid, feature_id: &str) {
        for (i, v) in solid.vertices.iter_mut().enumerate() {
            if v.name.is_none() {
                v.name =
                    EntityRef::try_named(feature_id, EntityKind::Vertex, &format!("v{i}")).ok();
            }
        }
        for (i, e) in solid.edges.iter_mut().enumerate() {
            if e.name.is_none() {
                e.name = EntityRef::try_named(feature_id, EntityKind::Edge, &format!("e{i}")).ok();
            }
        }
        for (i, f) in solid.faces.iter_mut().enumerate() {
            if f.name.is_none() {
                f.name = EntityRef::try_named(feature_id, EntityKind::Face, &format!("f{i}")).ok();
            }
        }
    }

    #[test]
    fn t14_sphere_face_partition_no_panic() {
        let mut gen = IdGenerator::new(0);
        let box_solid = make_cuboid(10.0, 10.0, 10.0, &mut gen).unwrap();
        let sphere = make_sphere(3.0, &mut gen).unwrap();

        let result = partition_faces(&box_solid, &sphere, BooleanOp::Cut);
        assert!(
            result.is_ok(),
            "partition with sphere face should not panic"
        );
        let (target_frags, tool_frags) = result.unwrap();
        assert!(!target_frags.is_empty(), "should have target fragments");
        assert!(!tool_frags.is_empty(), "should have tool fragments");
    }

    /// TX2 — box+sphere partition produces exact fragment counts (no intersection)
    #[test]
    fn tx2_box_sphere_partition_fragment_counts() {
        let mut gen = IdGenerator::new(0);
        let box_solid = make_cuboid(10.0, 10.0, 10.0, &mut gen).unwrap();
        let sphere = make_sphere(3.0, &mut gen).unwrap();

        let result = partition_faces(&box_solid, &sphere, BooleanOp::Cut);
        assert!(result.is_ok(), "partition should succeed");
        let (target_frags, tool_frags) = result.unwrap();

        // Sphere fully inside box → no intersections → whole-face fragments
        assert_eq!(
            target_frags.len(),
            6,
            "box should produce 6 fragments (one per face)"
        );
        assert_eq!(
            tool_frags.len(),
            1,
            "sphere should produce 1 fragment (whole sphere face)"
        );
    }

    #[test]
    fn t15_cyl_sph_face_pair_skipped() {
        let mut gen = IdGenerator::new(0);
        let mut cylinder = make_cylinder(2.0, 4.0, &mut gen).unwrap();
        let sphere = make_sphere(3.0, &mut gen).unwrap();
        ensure_names(&mut cylinder, "cyl");

        let result = partition_faces(&cylinder, &sphere, BooleanOp::Cut);
        if let Err(e) = &result {
            panic!("partition with cylinder+sphere failed: {e}");
        }
        let (target_frags, tool_frags) = result.unwrap();
        assert!(!target_frags.is_empty());
        assert!(!tool_frags.is_empty());

        for frag in &target_frags {
            if matches!(frag.surface, Surface::Cylinder { .. }) {
                assert!(
                    frag.boundary_partners.iter().all(|p| p.is_none()),
                    "cylinder face should have no intersection segments with sphere"
                );
            }
        }
        for frag in &tool_frags {
            if matches!(frag.surface, Surface::Sphere { .. }) {
                assert!(
                    frag.boundary_partners.iter().all(|p| p.is_none()),
                    "sphere face should have no intersection segments with cylinder"
                );
            }
        }
    }
}
