use crate::geometry::curve::Curve;
use crate::geometry::math::{length_near, LENGTH_TOLERANCE};
use crate::geometry::pcurve::Curve2D;
use crate::geometry::surface::Surface;
use crate::geometry::{Point, Vec3};
use mycad_format::EntityRef;

use super::types::BooleanOp;

/// Number of chord segments for discretizing Circle intersection curves.
const ANGULAR_SEGMENTS_DEFAULT: usize = 64;

/// An intersection segment with provenance tracking (per-edge partner).
#[derive(Clone)]
#[allow(dead_code)] // t_range / pcurve fields used by STEP η pcurve attach (not yet wired)
pub(crate) struct IntersectionSegment {
    pub p_start: (f64, f64),
    pub p_end: (f64, f64),
    pub partner: EntityRef,
    /// Analytical source curve (Some(Circle) for circle intersections, None for line).
    pub source_curve_3d: Option<Curve>,
    /// Parameter range on the 3D curve for this chord segment.
    pub curve_3d_t_range: [f64; 2],
    /// 2D curve on face A's UV space for this chord segment.
    pub pcurve_on_a: Option<Curve2D>,
    /// Parameter range on pcurve_on_a for this chord segment.
    pub pcurve_on_a_t_range: [f64; 2],
    /// 2D curve on face B's UV space for this chord segment.
    pub pcurve_on_b: Option<Curve2D>,
    /// Parameter range on pcurve_on_b for this chord segment.
    pub pcurve_on_b_t_range: [f64; 2],
}

pub(crate) struct FaceFragment {
    pub source_face_index: usize,
    pub polygon_3d: Vec<Point>,
    pub inner_polygons_3d: Vec<Vec<Point>>,
    pub surface: Surface,
    pub parent_name: EntityRef,
    pub traversal_index: u32,
    pub is_tool_side: bool,
    /// Per-edge partner provenance. Edge i connects polygon_3d[i] → polygon_3d[(i+1)%n].
    /// Some(partner) means this edge was created by an intersection with the partner face.
    /// None means it originated from the outer loop of the source face.
    pub boundary_partners: Vec<Option<EntityRef>>,
    pub boundary_curves: Vec<Option<Curve>>,
    pub boundary_t_ranges: Vec<[f64; 2]>,
    pub boundary_pcurves_a: Vec<Option<Curve2D>>,
    pub boundary_pcurves_b: Vec<Option<Curve2D>>,
    /// Inner loop boundary provenance (one Vec per inner loop).
    pub inner_boundary_partners: Vec<Vec<Option<EntityRef>>>,
    pub inner_boundary_curves: Vec<Vec<Option<Curve>>>,
    pub inner_boundary_t_ranges: Vec<Vec<[f64; 2]>>,
    pub inner_boundary_pcurves_a: Vec<Vec<Option<Curve2D>>>,
    pub inner_boundary_pcurves_b: Vec<Vec<Option<Curve2D>>>,
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
    // Counts how many target plane faces produced an interior circle from each sphere tool face.
    // Used to (a) detect multi-plane intersection and (b) skip the degenerate pass-through.
    let mut sphere_plane_interior_count: std::collections::HashMap<usize, usize> =
        std::collections::HashMap::new();
    // Tracks sphere tool faces whose caps were already built during cyl×sph processing
    // in the target loop, so the tool loop doesn't push a duplicate pass-through.
    let mut cyl_sph_sphere_processed: std::collections::HashSet<usize> =
        std::collections::HashSet::new();
    // Tracks cylinder target faces that were already split into bands during cyl×sph processing
    // in the target loop, so the tool loop doesn't push duplicate cylinder fragments.
    let mut cyl_sph_cyl_target_processed: std::collections::HashSet<usize> =
        std::collections::HashSet::new();

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

        // Degenerate polygon (circular face with <3 loop vertices) — pass through as-is
        if polygon_2d.len() < 3 {
            let name = face
                .name
                .clone()
                .ok_or_else(|| format!("target face {} missing name", fi))?;
            target_fragments.push(FaceFragment {
                source_face_index: fi,
                polygon_3d: polygon_3d.clone(),
                inner_polygons_3d: vec![],
                surface: surface.clone(),
                parent_name: name,
                traversal_index: 0,
                is_tool_side: false,
                boundary_partners: vec![None; polygon_3d.len()],
                boundary_curves: vec![None; polygon_3d.len()],
                boundary_t_ranges: vec![[0.0, 1.0]; polygon_3d.len()],
                boundary_pcurves_a: vec![None; polygon_3d.len()],
                boundary_pcurves_b: vec![None; polygon_3d.len()],
                inner_boundary_partners: vec![],
                inner_boundary_curves: vec![],
                inner_boundary_t_ranges: vec![],
                inner_boundary_pcurves_a: vec![],
                inner_boundary_pcurves_b: vec![],
            });
            continue;
        }
        let polygon_2d = &target_polygons_2d[fi];
        let polygon_3d = &target_polygons_3d[fi];
        let face = &target.faces[fi];
        let surface = &face.surface;

        let mut segments: Vec<IntersectionSegment> = Vec::new();
        let mut sphere_tool_face_ui: Option<usize> = None;

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

            let is_cyl = matches!(surface, Surface::Cylinder { .. });
            let is_sph = matches!(surface, Surface::Sphere { .. });
            let tool_is_cyl = matches!(tool_surface, Surface::Cylinder { .. });
            let tool_is_sph = matches!(tool_surface, Surface::Sphere { .. });
            let is_cyl_sph_pair = (is_cyl && tool_is_sph) || (is_sph && tool_is_cyl);

            let isect_result =
                crate::geometry::surface_intersect::intersect_surfaces(surface, tool_surface);
            let isect_loops = match isect_result {
                Ok(loops) => loops,
                Err(e) if is_cyl_sph_pair => {
                    return Err(format!("unsupported boolean case: {e}"));
                }
                Err(_) => continue,
            };
            if isect_loops.is_empty() {
                continue;
            }

            if tool_is_sph {
                sphere_tool_face_ui = Some(ui);
            }

            // Guard: Plane×Sphere great circle (d ≈ 0) → reject
            if (is_sph && matches!(tool_surface, Surface::Plane { .. }))
                || (matches!(surface, Surface::Plane { .. }) && tool_is_sph)
            {
                if let (
                    Surface::Plane {
                        origin: pl_orig,
                        normal: pl_norm,
                        ..
                    },
                    Surface::Sphere { center: sph_c, .. },
                ) = if matches!(surface, Surface::Plane { .. }) && tool_is_sph {
                    (surface, tool_surface)
                } else {
                    (tool_surface, surface)
                } {
                    let d = (sph_c.coords - pl_orig.coords).dot(&pl_norm.normalize());
                    if d.abs() < LENGTH_TOLERANCE {
                        return Err(
                            "unsupported boolean case: plane through sphere center (great circle)"
                                .to_string(),
                        );
                    }
                }
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
                            source_curve_3d: None,
                            curve_3d_t_range: [0.0, 1.0],
                            pcurve_on_a: None,
                            pcurve_on_a_t_range: [0.0, 1.0],
                            pcurve_on_b: None,
                            pcurve_on_b_t_range: [0.0, 1.0],
                        });
                    }
                    crate::geometry::curve::Curve::Circle { .. } => {
                        // For Plane×Cylinder: skip if the intersection plane is outside
                        // the tool cylinder face's finite height range.
                        if tool_is_cyl {
                            if let Surface::Cylinder {
                                origin: cyl_orig,
                                axis: cyl_ax,
                                ..
                            } = tool_surface
                            {
                                if let Curve2D::Line2D { origin: o, .. } = &iloop.pcurve_on_b {
                                    let ax = cyl_ax.normalize();
                                    let v_cut = o.1;
                                    let tool_poly = &tool_polygons_3d[ui];
                                    let v_min = tool_poly
                                        .iter()
                                        .map(|p| (p.coords - cyl_orig.coords).dot(&ax))
                                        .fold(f64::MAX, f64::min);
                                    let v_max = tool_poly
                                        .iter()
                                        .map(|p| (p.coords - cyl_orig.coords).dot(&ax))
                                        .fold(f64::MIN, f64::max);
                                    if v_cut < v_min - len_eps || v_cut > v_max + len_eps {
                                        continue;
                                    }
                                }
                            }
                        }

                        let n_chords = ANGULAR_SEGMENTS_DEFAULT;
                        let t0 = iloop.t_range[0];
                        let t1 = iloop.t_range[1];
                        let dt = (t1 - t0) / n_chords as f64;
                        let circle_3d = iloop.curve_3d.clone();
                        let pc_a = iloop.pcurve_on_a.clone();
                        let _pc_a_tr = iloop.pcurve_on_a_t_range;
                        let pc_b = iloop.pcurve_on_b.clone();
                        let _pc_b_tr = iloop.pcurve_on_b_t_range;

                        for k in 0..n_chords {
                            let ta = t0 + k as f64 * dt;
                            let tb = t0 + (k + 1) as f64 * dt;
                            let p3a = circle_3d.evaluate(ta);
                            let p3b = circle_3d.evaluate(tb);
                            let s_a = project_to_face_uv(surface, &p3a);
                            let s_b = project_to_face_uv(surface, &p3b);
                            let chord_len =
                                ((s_b.0 - s_a.0).powi(2) + (s_b.1 - s_a.1).powi(2)).sqrt();
                            if chord_len < len_eps {
                                continue;
                            }
                            if !tool_is_cyl && !tool_is_sph {
                                let Some((clipped_s, clipped_e)) =
                                    clip_line_to_polygon_2d_given_points(s_a, s_b, polygon_2d)
                                else {
                                    continue;
                                };
                                let cl = ((clipped_e.0 - clipped_s.0).powi(2)
                                    + (clipped_e.1 - clipped_s.1).powi(2))
                                .sqrt();
                                if cl < len_eps {
                                    continue;
                                }
                                segments.push(IntersectionSegment {
                                    p_start: clipped_s,
                                    p_end: clipped_e,
                                    partner: tool_face_name.clone(),
                                    source_curve_3d: Some(circle_3d.clone()),
                                    curve_3d_t_range: [ta, tb],
                                    pcurve_on_a: Some(pc_a.clone()),
                                    pcurve_on_a_t_range: [ta, tb],
                                    pcurve_on_b: Some(pc_b.clone()),
                                    pcurve_on_b_t_range: [ta, tb],
                                });
                            } else {
                                segments.push(IntersectionSegment {
                                    p_start: s_a,
                                    p_end: s_b,
                                    partner: tool_face_name.clone(),
                                    source_curve_3d: Some(circle_3d.clone()),
                                    curve_3d_t_range: [ta, tb],
                                    pcurve_on_a: Some(pc_a.clone()),
                                    pcurve_on_a_t_range: [ta, tb],
                                    pcurve_on_b: Some(pc_b.clone()),
                                    pcurve_on_b_t_range: [ta, tb],
                                });
                            }
                        }
                    }
                }
            }
        }

        // Guard: Sphere face intersected by multiple distinct plane faces → reject
        if matches!(surface, Surface::Sphere { .. }) && !segments.is_empty() {
            let mut partner_keys: std::collections::HashSet<String> =
                std::collections::HashSet::new();
            for seg in &segments {
                partner_keys.insert(format!("{:?}", seg.partner));
            }
            if partner_keys.len() > 1 {
                return Err("unsupported boolean case: multi-plane sphere intersection".to_string());
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
                inner_polygons_3d: vec![],
                surface: surface.clone(),
                parent_name: name,
                traversal_index: 0,
                is_tool_side: false,
                boundary_partners: vec![None; polygon_3d.len()],
                boundary_curves: vec![None; polygon_3d.len()],
                boundary_t_ranges: vec![[0.0, 1.0]; polygon_3d.len()],
                boundary_pcurves_a: vec![None; polygon_3d.len()],
                boundary_pcurves_b: vec![None; polygon_3d.len()],
                inner_boundary_partners: vec![],
                inner_boundary_curves: vec![],
                inner_boundary_t_ranges: vec![],
                inner_boundary_pcurves_a: vec![],
                inner_boundary_pcurves_b: vec![],
            });
        } else if let Some(plane_data) = plane {
            // Check if circle segments are interior (form a hole) vs boundary-crossing
            let circle_segs: Vec<&IntersectionSegment> = segments
                .iter()
                .filter(|s| s.source_curve_3d.is_some())
                .collect();
            let line_segs: Vec<&IntersectionSegment> = segments
                .iter()
                .filter(|s| s.source_curve_3d.is_none())
                .collect();

            if !circle_segs.is_empty()
                && line_segs.is_empty()
                && segments_are_interior(&circle_segs, polygon_2d)
            {
                // Interior circle: single fragment with hole
                // Obtain provenance (partners/curves/pcurses) from
                // collect_ordered_circle_polygon — we only use it for metadata,
                // not for 3D vertex positions.
                let (
                    inner_poly_2d,
                    inner_partners_proto,
                    inner_curves_proto,
                    _inner_tr_proto,
                    inner_pca_proto,
                    inner_pcb_proto,
                ) = collect_ordered_circle_polygon(&circle_segs, surface);

                // Generate ring inner-loop 3D points using the SAME
                // Circle::evaluate(k*dt) discretisation as the cylinder band
                // boundary, so that vertex-map merging in assemble produces
                // exact matches.  The ring loop is reversed (k=0, k=63, …, k=1)
                // so that each shared edge gets opposite half-edges → manifold OK.
                let n_inner = ANGULAR_SEGMENTS_DEFAULT; // 64
                let dt = 2.0 * std::f64::consts::PI / n_inner as f64;
                let circle_opt = circle_segs.first().and_then(|s| s.source_curve_3d.as_ref());
                let ring_inner_poly_3d: Vec<Point> = if let Some(circle) = circle_opt {
                    let mut pts = Vec::with_capacity(n_inner);
                    pts.push(circle.evaluate(0.0));
                    for k in (1..n_inner).rev() {
                        pts.push(circle.evaluate(k as f64 * dt));
                    }
                    pts
                } else {
                    // Fallback: legacy unproject path (no circle curve available)
                    inner_poly_2d
                        .iter()
                        .map(|(u, v)| unproject_from_face_uv(surface, *u, *v))
                        .collect()
                };

                // Pad provenance arrays to 64 entries (all edges share the same
                // circle segment provenance — same partner, curve, t-range).
                let first_partner = inner_partners_proto.first().and_then(|p| p.clone());
                let first_curve = inner_curves_proto.first().and_then(|c| c.clone());
                let first_pca = inner_pca_proto.first().and_then(|c| c.clone());
                let first_pcb = inner_pcb_proto.first().and_then(|c| c.clone());
                let inner_partners: Vec<Option<EntityRef>> =
                    (0..n_inner).map(|_| first_partner.clone()).collect();
                let inner_curves: Vec<Option<Curve>> =
                    (0..n_inner).map(|_| first_curve.clone()).collect();
                let inner_tr: Vec<[f64; 2]> = (0..n_inner)
                    .map(|_| [0.0, 2.0 * std::f64::consts::PI])
                    .collect();
                let inner_pca: Vec<Option<Curve2D>> =
                    (0..n_inner).map(|_| first_pca.clone()).collect();
                let inner_pcb: Vec<Option<Curve2D>> =
                    (0..n_inner).map(|_| first_pcb.clone()).collect();

                let n_outer = polygon_3d.len();
                target_fragments.push(FaceFragment {
                    source_face_index: fi,
                    polygon_3d: polygon_3d.clone(),
                    inner_polygons_3d: vec![ring_inner_poly_3d],
                    surface: surface.clone(),
                    parent_name: name.clone(),
                    traversal_index: 0,
                    is_tool_side: false,
                    boundary_partners: vec![None; n_outer],
                    boundary_curves: vec![None; n_outer],
                    boundary_t_ranges: vec![[0.0, 1.0]; n_outer],
                    boundary_pcurves_a: vec![None; n_outer],
                    boundary_pcurves_b: vec![None; n_outer],
                    inner_boundary_partners: vec![inner_partners],
                    inner_boundary_curves: vec![inner_curves],
                    inner_boundary_t_ranges: vec![inner_tr],
                    inner_boundary_pcurves_a: vec![inner_pca],
                    inner_boundary_pcurves_b: vec![inner_pcb],
                });
                // Also create a second fragment: the disc (tool side)
                let disc_poly_3d: Vec<Point> = inner_poly_2d
                    .iter()
                    .map(|(u, v)| unproject_from_face_uv(surface, *u, *v))
                    .collect();
                let n_disc = disc_poly_3d.len();
                let disc_curves: Vec<Option<Curve>> = (0..n_disc)
                    .map(|_| circle_segs.first().and_then(|s| s.source_curve_3d.clone()))
                    .collect();
                let disc_tr: Vec<[f64; 2]> = (0..n_disc)
                    .map(|_| [0.0, 2.0 * std::f64::consts::PI])
                    .collect();
                let disc_pca: Vec<Option<Curve2D>> = (0..n_disc)
                    .map(|_| circle_segs.first().and_then(|s| s.pcurve_on_a.clone()))
                    .collect();
                let disc_pcb: Vec<Option<Curve2D>> = (0..n_disc)
                    .map(|_| circle_segs.first().and_then(|s| s.pcurve_on_b.clone()))
                    .collect();
                let disc_partners: Vec<Option<EntityRef>> = if let Some(first) = circle_segs.first()
                {
                    vec![Some(first.partner.clone()); n_disc]
                } else {
                    vec![None; n_disc]
                };
                target_fragments.push(FaceFragment {
                    source_face_index: fi,
                    polygon_3d: disc_poly_3d,
                    inner_polygons_3d: vec![],
                    surface: surface.clone(),
                    parent_name: name.clone(),
                    traversal_index: 1,
                    is_tool_side: false,
                    boundary_partners: disc_partners,
                    boundary_curves: disc_curves,
                    boundary_t_ranges: disc_tr,
                    boundary_pcurves_a: disc_pca,
                    boundary_pcurves_b: disc_pcb,
                    inner_boundary_partners: vec![],
                    inner_boundary_curves: vec![],
                    inner_boundary_t_ranges: vec![],
                    inner_boundary_pcurves_a: vec![],
                    inner_boundary_pcurves_b: vec![],
                });
                // Plane×Sphere: create sphere cap fragment and detect multi-plane
                if let Some(sph_ui) = sphere_tool_face_ui {
                    // Reconstruct inner_poly_3d from UV for sphere cap (independent discretisation)
                    let inner_poly_3d: Vec<Point> = inner_poly_2d
                        .iter()
                        .map(|(u, v)| unproject_from_face_uv(surface, *u, *v))
                        .collect();
                    let count = sphere_plane_interior_count.entry(sph_ui).or_insert(0);
                    *count += 1;
                    if *count > 1 {
                        return Err(
                            "unsupported boolean case: multi-plane sphere intersection".to_string()
                        );
                    }
                    let sph_face = &tool.faces[sph_ui];
                    let sph_name = sph_face
                        .name
                        .clone()
                        .ok_or_else(|| format!("tool sphere face {} missing name", sph_ui))?;
                    let sph_polygon_3d = &tool_polygons_3d[sph_ui];
                    let n_seam = sph_polygon_3d.len();
                    let seam_curves: Vec<Option<Curve>> = {
                        let ol = &tool.loops[sph_face.outer_loop];
                        ol.half_edges
                            .iter()
                            .map(|he_idx| {
                                let he = &tool.half_edges[*he_idx];
                                Some(tool.edges[he.edge].curve.clone())
                            })
                            .collect()
                    };
                    let mut sphere_inner_poly_3d = inner_poly_3d.clone();
                    sphere_inner_poly_3d.reverse();
                    let n_inner = sphere_inner_poly_3d.len();
                    let circle_curve_opt =
                        circle_segs.first().and_then(|s| s.source_curve_3d.clone());
                    let inner_curves: Vec<Option<Curve>> =
                        (0..n_inner).map(|_| circle_curve_opt.clone()).collect();
                    let inner_tr: Vec<[f64; 2]> = (0..n_inner)
                        .map(|_| [0.0, 2.0 * std::f64::consts::PI])
                        .collect();
                    let inner_partners: Vec<Option<EntityRef>> = vec![Some(name.clone()); n_inner];
                    tool_fragments.push(FaceFragment {
                        source_face_index: sph_ui,
                        polygon_3d: sph_polygon_3d.clone(),
                        inner_polygons_3d: vec![sphere_inner_poly_3d],
                        surface: sph_face.surface.clone(),
                        parent_name: sph_name,
                        traversal_index: 0,
                        is_tool_side: true,
                        boundary_partners: vec![None; n_seam],
                        boundary_curves: seam_curves,
                        boundary_t_ranges: vec![[0.0, 1.0]; n_seam],
                        boundary_pcurves_a: vec![None; n_seam],
                        boundary_pcurves_b: vec![None; n_seam],
                        inner_boundary_partners: vec![inner_partners],
                        inner_boundary_curves: vec![inner_curves],
                        inner_boundary_t_ranges: vec![inner_tr],
                        inner_boundary_pcurves_a: vec![vec![None; n_inner]],
                        inner_boundary_pcurves_b: vec![vec![None; n_inner]],
                    });
                }
            } else {
                // Standard PSLG subdivision (line segments or circle segments crossing boundary)
                let sub_faces = pslg_subdivide(polygon_2d, &segments, plane_data);
                for (idx, (sub_poly_2d, edge_partners)) in sub_faces.into_iter().enumerate() {
                    let poly_3d: Vec<Point> = sub_poly_2d
                        .iter()
                        .map(|(u, v)| unproject_from_face_uv(surface, *u, *v))
                        .collect();
                    let n = poly_3d.len();
                    target_fragments.push(FaceFragment {
                        source_face_index: fi,
                        polygon_3d: poly_3d,
                        inner_polygons_3d: vec![],
                        surface: surface.clone(),
                        parent_name: name.clone(),
                        traversal_index: idx as u32,
                        is_tool_side: false,
                        boundary_partners: edge_partners,
                        boundary_curves: vec![None; n],
                        boundary_t_ranges: vec![[0.0, 1.0]; n],
                        boundary_pcurves_a: vec![None; n],
                        boundary_pcurves_b: vec![None; n],
                        inner_boundary_partners: vec![],
                        inner_boundary_curves: vec![],
                        inner_boundary_t_ranges: vec![],
                        inner_boundary_pcurves_a: vec![],
                        inner_boundary_pcurves_b: vec![],
                    });
                }
            }
        } else if matches!(surface, Surface::Cylinder { .. })
            && sphere_tool_face_ui.is_some()
            && segments.iter().any(|s| s.source_curve_3d.is_some())
        {
            // Cylinder target face intersected by a sphere tool face.
            // Collect intersection circles and split into bands.
            let (cyl_origin, cyl_axis_raw, cyl_radius) = match surface {
                Surface::Cylinder {
                    origin,
                    axis,
                    radius,
                } => (*origin, *axis, *radius),
                _ => unreachable!(),
            };
            let cyl_axis = cyl_axis_raw.normalize();

            let v_bot = polygon_3d
                .iter()
                .map(|p| (p.coords - cyl_origin.coords).dot(&cyl_axis))
                .fold(f64::MAX, f64::min);
            let v_top = polygon_3d
                .iter()
                .map(|p| (p.coords - cyl_origin.coords).dot(&cyl_axis))
                .fold(f64::MIN, f64::max);

            // Collect distinct intersection circles (by v height) sorted ascending
            let mut circle_vs: Vec<(f64, Curve, EntityRef)> = Vec::new();
            for seg in &segments {
                if seg.source_curve_3d.is_none() {
                    continue;
                }
                let v = match &seg.pcurve_on_a {
                    Some(Curve2D::Line2D { origin, .. }) => origin.1,
                    _ => continue,
                };
                // Skip circles outside the face height range
                if v < v_bot - len_eps || v > v_top + len_eps {
                    continue;
                }
                // Dedup by v
                if circle_vs.iter().any(|(vv, _, _)| (vv - v).abs() < len_eps) {
                    continue;
                }
                let curve_3d = match &seg.source_curve_3d {
                    Some(c @ Curve::Circle { .. }) => c.clone(),
                    _ => continue,
                };
                let partner = seg.partner.clone();
                circle_vs.push((v, curve_3d, partner));
            }
            circle_vs.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));

            if circle_vs.is_empty() {
                // No valid circles — pass through as-is
                target_fragments.push(FaceFragment {
                    source_face_index: fi,
                    polygon_3d: polygon_3d.clone(),
                    inner_polygons_3d: vec![],
                    surface: surface.clone(),
                    parent_name: name.clone(),
                    traversal_index: 0,
                    is_tool_side: false,
                    boundary_partners: vec![None; polygon_3d.len()],
                    boundary_curves: vec![None; polygon_3d.len()],
                    boundary_t_ranges: vec![[0.0, 1.0]; polygon_3d.len()],
                    boundary_pcurves_a: vec![None; polygon_3d.len()],
                    boundary_pcurves_b: vec![None; polygon_3d.len()],
                    inner_boundary_partners: vec![],
                    inner_boundary_curves: vec![],
                    inner_boundary_t_ranges: vec![],
                    inner_boundary_pcurves_a: vec![],
                    inner_boundary_pcurves_b: vec![],
                });
            } else {
                // Build bands between v_bot, each circle v, and v_top
                let n_seg = ANGULAR_SEGMENTS_DEFAULT;
                let dt = 2.0 * std::f64::consts::PI / n_seg as f64;
                let mut band_idx: u32 = 0;

                let mut boundaries: Vec<f64> = vec![v_bot];
                for (v, _, _) in &circle_vs {
                    boundaries.push(*v);
                }
                boundaries.push(v_top);

                for band in 0..boundaries.len() - 1 {
                    let v_lo = boundaries[band];
                    let v_hi = boundaries[band + 1];
                    if (v_hi - v_lo).abs() < len_eps {
                        continue;
                    }

                    let _is_bottom = band == 0;
                    let _is_top = band == boundaries.len() - 2;

                    let seam_lo = surface.evaluate(0.0, v_lo);
                    let seam_hi = surface.evaluate(0.0, v_hi);

                    let lo_circle_3d = Curve::Circle {
                        center: Point::from(cyl_origin.coords + cyl_axis * v_lo),
                        normal: cyl_axis,
                        radius: cyl_radius,
                    };
                    let hi_circle_3d = Curve::Circle {
                        center: Point::from(cyl_origin.coords + cyl_axis * v_hi),
                        normal: cyl_axis,
                        radius: cyl_radius,
                    };

                    let total_pts = 2 * n_seg + 2;
                    let mut poly: Vec<Point> = Vec::with_capacity(total_pts);
                    poly.push(seam_lo);
                    poly.push(seam_hi);
                    for k in 1..n_seg {
                        poly.push(hi_circle_3d.evaluate(k as f64 * dt));
                    }
                    poly.push(seam_hi);
                    poly.push(seam_lo);
                    for k in 1..n_seg {
                        poly.push(lo_circle_3d.evaluate(k as f64 * dt));
                    }

                    let mut partners: Vec<Option<EntityRef>> = vec![None; total_pts];
                    let mut curves: Vec<Option<Curve>> = vec![None; total_pts];

                    // Upper boundary (first half, indices 1..=n_seg)
                    if band < circle_vs.len() {
                        // Intersection circle — partner + curve
                        let (_, ref circle_c, ref partner_c) = circle_vs[band];
                        for k in 1..=n_seg {
                            partners[k] = Some(partner_c.clone());
                            curves[k] = Some(circle_c.clone());
                        }
                    }
                    // else: top rim — no curve, no partner (matches original)

                    // Lower boundary (second half, indices n_seg+2..total_pts-1)
                    if band > 0 {
                        // Intersection circle — partner + curve
                        let (_, ref circle_c, ref partner_c) = circle_vs[band - 1];
                        for curve in curves.iter_mut().skip(n_seg + 2) {
                            *curve = Some(circle_c.clone());
                        }
                        for partner in partners.iter_mut().skip(n_seg + 2) {
                            *partner = Some(partner_c.clone());
                        }
                    } else {
                        // Bottom rim — curve only (no partner)
                        for curve in curves.iter_mut().skip(n_seg + 2) {
                            *curve = Some(lo_circle_3d.clone());
                        }
                    }

                    target_fragments.push(FaceFragment {
                        source_face_index: fi,
                        polygon_3d: poly,
                        inner_polygons_3d: vec![],
                        surface: surface.clone(),
                        parent_name: name.clone(),
                        traversal_index: band_idx,
                        is_tool_side: false,
                        boundary_partners: partners,
                        boundary_curves: curves,
                        boundary_t_ranges: vec![[0.0, 1.0]; total_pts],
                        boundary_pcurves_a: vec![None; total_pts],
                        boundary_pcurves_b: vec![None; total_pts],
                        inner_boundary_partners: vec![],
                        inner_boundary_curves: vec![],
                        inner_boundary_t_ranges: vec![],
                        inner_boundary_pcurves_a: vec![],
                        inner_boundary_pcurves_b: vec![],
                    });
                    band_idx += 1;
                }

                // Mark this cylinder face as processed
                cyl_sph_cyl_target_processed.insert(fi);

                // Also create sphere cap fragments (tool side)
                if let Some(&sph_ui) = sphere_tool_face_ui.as_ref() {
                    if !cyl_sph_sphere_processed.contains(&sph_ui) {
                        let sph_face = &tool.faces[sph_ui];
                        let sph_name = sph_face
                            .name
                            .clone()
                            .ok_or_else(|| format!("tool sphere face {} missing name", sph_ui))?;
                        let sph_polygon_3d = &tool_polygons_3d[sph_ui];
                        let sph_surface = &sph_face.surface;
                        let _n_seam = sph_polygon_3d.len();

                        // Extract the sphere surface properties
                        let (sph_center, sph_radius) = match sph_surface {
                            Surface::Sphere { center, radius } => (*center, *radius),
                            _ => unreachable!(),
                        };

                        // Build one cap per intersection circle
                        for (cap_idx, (v, circle_c, _partner_c)) in circle_vs.iter().enumerate() {
                            let circle_center_on_axis = cyl_origin.coords + cyl_axis * (*v);
                            let is_top_cap = (*v - sph_center.coords.dot(&cyl_axis)).abs()
                                > len_eps
                                && *v > sph_center.coords.dot(&cyl_axis);

                            // Interior point: pole on the side away from cylinder center
                            let pole_z = if is_top_cap {
                                sph_center.coords + sph_radius * cyl_axis
                            } else {
                                sph_center.coords - sph_radius * cyl_axis
                            };
                            let _interior = Point::from((pole_z + circle_center_on_axis) * 0.5);

                            // Full sphere polygon (seam)
                            let cap_poly_3d: Vec<Point> = sph_polygon_3d.clone();
                            let n_cap = cap_poly_3d.len();

                            // Build inner polygon from the circle
                            let mut inner_poly_3d: Vec<Point> = Vec::with_capacity(n_seg);
                            for k in 0..n_seg {
                                inner_poly_3d.push(circle_c.evaluate(k as f64 * dt));
                            }
                            inner_poly_3d.reverse();
                            let n_inner = inner_poly_3d.len();

                            let inner_curves: Vec<Option<Curve>> =
                                (0..n_inner).map(|_| Some(circle_c.clone())).collect();
                            let inner_tr: Vec<[f64; 2]> = (0..n_inner)
                                .map(|_| [0.0, 2.0 * std::f64::consts::PI])
                                .collect();
                            let inner_partners: Vec<Option<EntityRef>> =
                                (0..n_inner).map(|_| Some(name.clone())).collect();

                            let seam_curves: Vec<Option<Curve>> = {
                                let ol = &tool.loops[sph_face.outer_loop];
                                ol.half_edges
                                    .iter()
                                    .map(|he_idx| {
                                        let he = &tool.half_edges[*he_idx];
                                        Some(tool.edges[he.edge].curve.clone())
                                    })
                                    .collect()
                            };

                            tool_fragments.push(FaceFragment {
                                source_face_index: sph_ui,
                                polygon_3d: cap_poly_3d,
                                inner_polygons_3d: vec![inner_poly_3d],
                                surface: sph_surface.clone(),
                                parent_name: sph_name.clone(),
                                traversal_index: cap_idx as u32,
                                is_tool_side: true,
                                boundary_partners: vec![None; n_cap],
                                boundary_curves: seam_curves,
                                boundary_t_ranges: vec![[0.0, 1.0]; n_cap],
                                boundary_pcurves_a: vec![None; n_cap],
                                boundary_pcurves_b: vec![None; n_cap],
                                inner_boundary_partners: vec![inner_partners],
                                inner_boundary_curves: vec![inner_curves],
                                inner_boundary_t_ranges: vec![inner_tr],
                                inner_boundary_pcurves_a: vec![vec![None; n_inner]],
                                inner_boundary_pcurves_b: vec![vec![None; n_inner]],
                            });
                        }
                        cyl_sph_sphere_processed.insert(sph_ui);
                    }
                }
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

        // Degenerate polygon (circular face with <3 loop vertices) — pass through as-is
        if polygon_2d.len() < 3 {
            // Sphere faces that already received an interior-circle cap in the first loop
            // must not be pushed again as bare pass-through fragments.
            if sphere_plane_interior_count.contains_key(&fi)
                || cyl_sph_sphere_processed.contains(&fi)
            {
                continue;
            }
            let name = face
                .name
                .clone()
                .ok_or_else(|| format!("tool face {} missing name", fi))?;
            tool_fragments.push(FaceFragment {
                source_face_index: fi,
                polygon_3d: polygon_3d.clone(),
                inner_polygons_3d: vec![],
                surface: surface.clone(),
                parent_name: name,
                traversal_index: 0,
                is_tool_side: true,
                boundary_partners: vec![None; polygon_3d.len()],
                boundary_curves: vec![None; polygon_3d.len()],
                boundary_t_ranges: vec![[0.0, 1.0]; polygon_3d.len()],
                boundary_pcurves_a: vec![None; polygon_3d.len()],
                boundary_pcurves_b: vec![None; polygon_3d.len()],
                inner_boundary_partners: vec![],
                inner_boundary_curves: vec![],
                inner_boundary_t_ranges: vec![],
                inner_boundary_pcurves_a: vec![],
                inner_boundary_pcurves_b: vec![],
            });
            continue;
        }

        let mut segments: Vec<IntersectionSegment> = Vec::new();

        for ti in 0..target.faces.len() {
            if coplanar_target_set[ti] {
                continue;
            }
            let target_face_name = target.faces[ti]
                .name
                .clone()
                .ok_or_else(|| format!("target face {} missing name", ti))?;

            let target_surface = &target.faces[ti].surface;
            let is_cyl = matches!(surface, Surface::Cylinder { .. });
            let is_sph = matches!(surface, Surface::Sphere { .. });
            let target_is_cyl = matches!(target_surface, Surface::Cylinder { .. });
            let target_is_sph = matches!(target_surface, Surface::Sphere { .. });
            let is_cyl_sph_pair = (is_cyl && target_is_sph) || (is_sph && target_is_cyl);

            let isect_result =
                crate::geometry::surface_intersect::intersect_surfaces(surface, target_surface);
            let isect_loops = match isect_result {
                Ok(loops) => loops,
                Err(e) if is_cyl_sph_pair => {
                    return Err(format!("unsupported boolean case: {e}"));
                }
                Err(_) => continue,
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
                            source_curve_3d: None,
                            curve_3d_t_range: [0.0, 1.0],
                            pcurve_on_a: None,
                            pcurve_on_a_t_range: [0.0, 1.0],
                            pcurve_on_b: None,
                            pcurve_on_b_t_range: [0.0, 1.0],
                        });
                    }
                    crate::geometry::curve::Curve::Circle { .. } => {
                        let n_chords = ANGULAR_SEGMENTS_DEFAULT;
                        let t0 = iloop.t_range[0];
                        let t1 = iloop.t_range[1];
                        let dt = (t1 - t0) / n_chords as f64;
                        let circle_3d = iloop.curve_3d.clone();
                        let pc_a = iloop.pcurve_on_a.clone();
                        let _pc_a_tr = iloop.pcurve_on_a_t_range;
                        let pc_b = iloop.pcurve_on_b.clone();
                        let _pc_b_tr = iloop.pcurve_on_b_t_range;

                        for k in 0..n_chords {
                            let ta = t0 + k as f64 * dt;
                            let tb = t0 + (k + 1) as f64 * dt;
                            let p3a = circle_3d.evaluate(ta);
                            let p3b = circle_3d.evaluate(tb);
                            let s_a = project_to_face_uv(surface, &p3a);
                            let s_b = project_to_face_uv(surface, &p3b);
                            let chord_len =
                                ((s_b.0 - s_a.0).powi(2) + (s_b.1 - s_a.1).powi(2)).sqrt();
                            if chord_len < len_eps {
                                continue;
                            }
                            // For Cylinder / Sphere tool faces, the UV polygon is self-intersecting
                            // due to the seam (u = atan2 jumps at π). Skip the UV clip
                            // check; the Cylinder/Sphere branch reconstructs fragments directly
                            // in 3D using the circle curve.
                            if !is_cyl && !is_sph {
                                let Some((clipped_s, clipped_e)) =
                                    clip_line_to_polygon_2d_given_points(s_a, s_b, polygon_2d)
                                else {
                                    continue;
                                };
                                let cl = ((clipped_e.0 - clipped_s.0).powi(2)
                                    + (clipped_e.1 - clipped_s.1).powi(2))
                                .sqrt();
                                if cl < len_eps {
                                    continue;
                                }
                                segments.push(IntersectionSegment {
                                    p_start: clipped_s,
                                    p_end: clipped_e,
                                    partner: target_face_name.clone(),
                                    source_curve_3d: Some(circle_3d.clone()),
                                    curve_3d_t_range: [ta, tb],
                                    pcurve_on_a: Some(pc_a.clone()),
                                    pcurve_on_a_t_range: [ta, tb],
                                    pcurve_on_b: Some(pc_b.clone()),
                                    pcurve_on_b_t_range: [ta, tb],
                                });
                            } else {
                                segments.push(IntersectionSegment {
                                    p_start: s_a,
                                    p_end: s_b,
                                    partner: target_face_name.clone(),
                                    source_curve_3d: Some(circle_3d.clone()),
                                    curve_3d_t_range: [ta, tb],
                                    pcurve_on_a: Some(pc_a.clone()),
                                    pcurve_on_a_t_range: [ta, tb],
                                    pcurve_on_b: Some(pc_b.clone()),
                                    pcurve_on_b_t_range: [ta, tb],
                                });
                            }
                        }
                        let _ = (_pc_a_tr, _pc_b_tr);
                    }
                }
            }
        }

        let name = face
            .name
            .clone()
            .ok_or_else(|| format!("tool face {} missing name", fi))?;

        if matches!(surface, Surface::Cylinder { .. }) {}
        if segments.is_empty() {
            tool_fragments.push(FaceFragment {
                source_face_index: fi,
                polygon_3d: polygon_3d.clone(),
                inner_polygons_3d: vec![],
                surface: surface.clone(),
                parent_name: name,
                traversal_index: 0,
                is_tool_side: true,
                boundary_partners: vec![None; polygon_3d.len()],
                boundary_curves: vec![None; polygon_3d.len()],
                boundary_t_ranges: vec![[0.0, 1.0]; polygon_3d.len()],
                boundary_pcurves_a: vec![None; polygon_3d.len()],
                boundary_pcurves_b: vec![None; polygon_3d.len()],
                inner_boundary_partners: vec![],
                inner_boundary_curves: vec![],
                inner_boundary_t_ranges: vec![],
                inner_boundary_pcurves_a: vec![],
                inner_boundary_pcurves_b: vec![],
            });
        } else if let Some(plane_data) = PlaneData::from_surface(surface) {
            let sub_faces = pslg_subdivide(polygon_2d, &segments, &plane_data);
            for (idx, (sub_poly_2d, edge_partners)) in sub_faces.into_iter().enumerate() {
                let poly_3d: Vec<Point> = sub_poly_2d
                    .iter()
                    .map(|(u, v)| unproject_from_face_uv(surface, *u, *v))
                    .collect();
                let n = poly_3d.len();
                tool_fragments.push(FaceFragment {
                    source_face_index: fi,
                    polygon_3d: poly_3d,
                    inner_polygons_3d: vec![],
                    surface: surface.clone(),
                    parent_name: name.clone(),
                    traversal_index: idx as u32,
                    is_tool_side: true,
                    boundary_partners: edge_partners,
                    boundary_curves: vec![None; n],
                    boundary_t_ranges: vec![[0.0, 1.0]; n],
                    boundary_pcurves_a: vec![None; n],
                    boundary_pcurves_b: vec![None; n],
                    inner_boundary_partners: vec![],
                    inner_boundary_curves: vec![],
                    inner_boundary_t_ranges: vec![],
                    inner_boundary_pcurves_a: vec![],
                    inner_boundary_pcurves_b: vec![],
                });
            }
        } else if matches!(surface, Surface::Cylinder { .. }) {
            // Cylinder lat-face trimmed by intersection circle(s).
            // UV space for a periodic surface is non-simple after seam-crossing — build
            // the trimmed fragments directly in 3D instead of going through PSLG.
            let (cyl_origin, cyl_axis_raw, cyl_radius) = match surface {
                Surface::Cylinder {
                    origin,
                    axis,
                    radius,
                } => (*origin, *axis, *radius),
                _ => unreachable!(),
            };
            let cyl_axis_norm = cyl_axis_raw.normalize();

            let v_bot = polygon_3d
                .iter()
                .map(|p| (p.coords - cyl_origin.coords).dot(&cyl_axis_norm))
                .fold(f64::MAX, f64::min);
            let v_top = polygon_3d
                .iter()
                .map(|p| (p.coords - cyl_origin.coords).dot(&cyl_axis_norm))
                .fold(f64::MIN, f64::max);

            // Collect distinct intersection circles (by v height) sorted ascending
            let mut circle_vs: Vec<(f64, Curve, EntityRef)> = Vec::new();
            for seg in &segments {
                if seg.source_curve_3d.is_none() {
                    continue;
                }
                let v = match &seg.pcurve_on_b {
                    Some(Curve2D::Line2D { origin, .. }) => origin.1,
                    _ => continue,
                };
                if v < v_bot - len_eps || v > v_top + len_eps {
                    continue;
                }
                if circle_vs.iter().any(|(vv, _, _)| (vv - v).abs() < len_eps) {
                    continue;
                }
                let curve_3d = match &seg.source_curve_3d {
                    Some(c @ Curve::Circle { .. }) => c.clone(),
                    _ => continue,
                };
                circle_vs.push((v, curve_3d, seg.partner.clone()));
            }
            circle_vs.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));

            if circle_vs.is_empty() {
                continue;
            }

            let n_seg = ANGULAR_SEGMENTS_DEFAULT;
            let dt = 2.0 * std::f64::consts::PI / n_seg as f64;
            let mut band_idx: u32 = 0;

            let mut boundaries: Vec<f64> = vec![v_bot];
            for (v, _, _) in &circle_vs {
                boundaries.push(*v);
            }
            boundaries.push(v_top);

            for band in 0..boundaries.len() - 1 {
                let v_lo = boundaries[band];
                let v_hi = boundaries[band + 1];
                if (v_hi - v_lo).abs() < len_eps {
                    continue;
                }

                let _is_bottom = band == 0;
                let _is_top = band == boundaries.len() - 2;

                // Build evaluation circles for each boundary.
                // For intersection boundaries (those with a circle_vs entry),
                // use the intersection circle directly so that vertices match
                // the ring fragment on the plane face.  For cap boundaries
                // (top/bottom of cylinder), use the locally-constructed circle.
                let lo_circle_3d = Curve::Circle {
                    center: Point::from(cyl_origin.coords + cyl_axis_norm * v_lo),
                    normal: cyl_axis_norm,
                    radius: cyl_radius,
                };
                let hi_circle_3d = Curve::Circle {
                    center: Point::from(cyl_origin.coords + cyl_axis_norm * v_hi),
                    normal: cyl_axis_norm,
                    radius: cyl_radius,
                };

                // Pick the circle to evaluate for each boundary.
                // Intersection boundaries must use the intersection circle
                // (source_curve_3d) so that vertex positions are bit-exact
                // matches with the ring fragment's inner loop on the partner face.
                let lo_eval: &Curve = if band > 0 {
                    &circle_vs[band - 1].1
                } else {
                    &lo_circle_3d
                };
                let hi_eval: &Curve = if band < circle_vs.len() {
                    &circle_vs[band].1
                } else {
                    &hi_circle_3d
                };

                // Seam points must also come from the evaluation circle so the
                // polygon closes consistently (seam = circle.evaluate(0)).
                let seam_lo = lo_eval.evaluate(0.0);
                let seam_hi = hi_eval.evaluate(0.0);

                // Lower half of band polygon: seam_lo → seam_hi → hi_circle_k1..k63
                // Upper half: seam_hi → seam_lo → lo_circle_k1..k63
                let total_pts = 2 * n_seg + 2;
                let mut poly: Vec<Point> = Vec::with_capacity(total_pts);
                poly.push(seam_lo);
                poly.push(seam_hi);
                // Top rim (no intersection circle above): reverse hi_circle to match
                // original winding — consistent with cap face boundary direction.
                if band >= circle_vs.len() {
                    for k in (1..n_seg).rev() {
                        poly.push(hi_eval.evaluate(k as f64 * dt));
                    }
                } else {
                    for k in 1..n_seg {
                        poly.push(hi_eval.evaluate(k as f64 * dt));
                    }
                }
                poly.push(seam_hi);
                poly.push(seam_lo);
                for k in 1..n_seg {
                    poly.push(lo_eval.evaluate(k as f64 * dt));
                }

                let mut partners: Vec<Option<EntityRef>> = vec![None; total_pts];
                let mut curves: Vec<Option<Curve>> = vec![None; total_pts];

                // Upper boundary (first half, indices 1..=n_seg)
                if band < circle_vs.len() {
                    let (_, ref circle_c, ref partner_c) = circle_vs[band];
                    for k in 1..=n_seg {
                        partners[k] = Some(partner_c.clone());
                        curves[k] = Some(circle_c.clone());
                    }
                }

                // Lower boundary (second half, indices n_seg+2..total_pts-1)
                if band > 0 {
                    let (_, ref circle_c, ref partner_c) = circle_vs[band - 1];
                    for curve in curves.iter_mut().skip(n_seg + 2) {
                        *curve = Some(circle_c.clone());
                    }
                    for partner in partners.iter_mut().skip(n_seg + 2) {
                        *partner = Some(partner_c.clone());
                    }
                } else {
                    for curve in curves.iter_mut().skip(n_seg + 2) {
                        *curve = Some(lo_circle_3d.clone());
                    }
                }

                tool_fragments.push(FaceFragment {
                    source_face_index: fi,
                    polygon_3d: poly,
                    inner_polygons_3d: vec![],
                    surface: surface.clone(),
                    parent_name: name.clone(),
                    traversal_index: band_idx,
                    is_tool_side: true,
                    boundary_partners: partners,
                    boundary_curves: curves,
                    boundary_t_ranges: vec![[0.0, 1.0]; total_pts],
                    boundary_pcurves_a: vec![None; total_pts],
                    boundary_pcurves_b: vec![None; total_pts],
                    inner_boundary_partners: vec![],
                    inner_boundary_curves: vec![],
                    inner_boundary_t_ranges: vec![],
                    inner_boundary_pcurves_a: vec![],
                    inner_boundary_pcurves_b: vec![],
                });
                band_idx += 1;
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
            let n = poly_3d.len();
            target_fragments.push(FaceFragment {
                source_face_index: pair.target_face,
                polygon_3d: poly_3d.clone(),
                inner_polygons_3d: vec![],
                surface: t_surface.clone(),
                parent_name: tname.clone(),
                traversal_index: 0,
                is_tool_side: false,
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
                let n = poly_3d.len();
                target_fragments.push(FaceFragment {
                    source_face_index: pair.target_face,
                    polygon_3d: poly_3d.clone(),
                    inner_polygons_3d: vec![],
                    surface: t_surface.clone(),
                    parent_name: tname.clone(),
                    traversal_index: strip_idx,
                    is_tool_side: false,
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
                });
                strip_idx += 1;
            }

            remaining = clip_polygon_halfplane(&remaining, ei, ej, true);
        }
    }

    Ok((target_fragments, tool_fragments))
}

/// Check if all circle chord endpoints lie strictly inside the outer polygon (not on boundary).
fn segments_are_interior(
    circle_segs: &[&IntersectionSegment],
    outer_loop_2d: &[(f64, f64)],
) -> bool {
    let eps = LENGTH_TOLERANCE * 10.0;
    circle_segs.iter().all(|seg| {
        !point_on_polygon_boundary(seg.p_start, outer_loop_2d, eps)
            && !point_on_polygon_boundary(seg.p_end, outer_loop_2d, eps)
    })
}

/// Check if a 2D point lies on the boundary of a polygon.
fn point_on_polygon_boundary(p: (f64, f64), polygon: &[(f64, f64)], eps: f64) -> bool {
    let n = polygon.len();
    for i in 0..n {
        let j = (i + 1) % n;
        if point_on_segment_2d(p, polygon[i], polygon[j])
            && dist_to_segment_2d(p, polygon[i], polygon[j]) < eps
        {
            return true;
        }
    }
    false
}

/// Distance from point p to the line segment a-b.
fn dist_to_segment_2d(p: (f64, f64), a: (f64, f64), b: (f64, f64)) -> f64 {
    let dx = b.0 - a.0;
    let dy = b.1 - a.1;
    let len_sq = dx * dx + dy * dy;
    if len_sq < LENGTH_TOLERANCE * LENGTH_TOLERANCE {
        return ((p.0 - a.0).powi(2) + (p.1 - a.1).powi(2)).sqrt();
    }
    let t = ((p.0 - a.0) * dx + (p.1 - a.1) * dy) / len_sq;
    let t = t.clamp(0.0, 1.0);
    let proj_x = a.0 + t * dx;
    let proj_y = a.1 + t * dy;
    ((p.0 - proj_x).powi(2) + (p.1 - proj_y).powi(2)).sqrt()
}

/// Collect ordered circle polygon vertices from chord segments.
/// Returns (polygon_2d, partners, curves, t_ranges, pcurves_a, pcurves_b).
#[allow(clippy::type_complexity)]
fn collect_ordered_circle_polygon(
    circle_segs: &[&IntersectionSegment],
    _surface: &Surface,
) -> (
    Vec<(f64, f64)>,
    Vec<Option<EntityRef>>,
    Vec<Option<Curve>>,
    Vec<[f64; 2]>,
    Vec<Option<Curve2D>>,
    Vec<Option<Curve2D>>,
) {
    // Collect unique endpoints
    let eps = LENGTH_TOLERANCE * 10.0;
    let mut points: Vec<(f64, f64)> = Vec::new();
    for seg in circle_segs {
        for pt in &[seg.p_start, seg.p_end] {
            let exists = points
                .iter()
                .any(|p| (p.0 - pt.0).abs() < eps && (p.1 - pt.1).abs() < eps);
            if !exists {
                points.push(*pt);
            }
        }
    }

    // Compute centroid for atan2 sorting
    let cx: f64 = points.iter().map(|p| p.0).sum::<f64>() / points.len() as f64;
    let cy: f64 = points.iter().map(|p| p.1).sum::<f64>() / points.len() as f64;

    // Sort by atan2 from centroid (deterministic CCW ordering)
    points.sort_by(|a, b| {
        let angle_a = (a.1 - cy).atan2(a.0 - cx);
        let angle_b = (b.1 - cy).atan2(b.0 - cx);
        angle_a
            .partial_cmp(&angle_b)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    let n = points.len();

    // Build provenance per edge by matching chord segments
    let first_partner = circle_segs.first().map(|s| s.partner.clone());
    let first_curve = circle_segs.first().and_then(|s| s.source_curve_3d.clone());
    let first_pca = circle_segs.first().and_then(|s| s.pcurve_on_a.clone());
    let first_pcb = circle_segs.first().and_then(|s| s.pcurve_on_b.clone());

    let partners: Vec<Option<EntityRef>> = (0..n).map(|_| first_partner.clone()).collect();
    let curves: Vec<Option<Curve>> = (0..n).map(|_| first_curve.clone()).collect();
    let t_ranges: Vec<[f64; 2]> = (0..n).map(|_| [0.0, 2.0 * std::f64::consts::PI]).collect();
    let pcurves_a: Vec<Option<Curve2D>> = (0..n).map(|_| first_pca.clone()).collect();
    let pcurves_b: Vec<Option<Curve2D>> = (0..n).map(|_| first_pcb.clone()).collect();

    (points, partners, curves, t_ranges, pcurves_a, pcurves_b)
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
    // Self-adjacent seam loops (e.g. full sphere: 2 HEs on the SAME edge) must NOT
    // be densified — the folded polygon (up and back down the same arc) has zero
    // area and breaks signed_volume computation. signed_volume handles verts.len() < 3.
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
            for k in 1..ANGULAR_SEGMENTS_DEFAULT {
                let t = ta + (tb - ta) * k as f64 / ANGULAR_SEGMENTS_DEFAULT as f64;
                pts.push(edge.curve.evaluate(t));
            }
        }
    }
    pts
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
        let sphere = make_sphere(3.0, Point::origin(), &mut gen).unwrap();

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
        let sphere = make_sphere(3.0, Point::origin(), &mut gen).unwrap();

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
        let mut cylinder = make_cylinder(2.0, 4.0, Point::origin(), &mut gen).unwrap();
        // Sphere R=1 at z=2: sits inside the cylinder (cyl R=2), d=2 to each cap > R=1
        // so sphere does not intersect the cylinder cap planes, only the lateral face is relevant.
        let mut sphere = make_sphere(1.0, Point::origin(), &mut gen).unwrap();
        for v in &mut sphere.vertices {
            v.point.coords.z += 2.0;
        }
        // Also offset sphere surface center
        for f in &mut sphere.faces {
            if let Surface::Sphere { center, .. } = &mut f.surface {
                center.coords.z += 2.0;
            }
        }
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

    /// T08 — Circle PSLG interior detection: box×cylinder partition produces
    /// a FaceFragment with inner_polygons_3d on the plane face.
    #[test]
    fn t16_circle_pslg_interior_detection() {
        let mut gen = IdGenerator::new(0);
        let box_solid = make_cuboid(10.0, 10.0, 10.0, &mut gen).unwrap();
        let cylinder = make_cylinder(2.0, 6.0, Point::origin(), &mut gen).unwrap();

        let (target_frags, _tool_frags) = partition_faces(&box_solid, &cylinder, BooleanOp::Cut)
            .expect("partition should succeed");

        let has_inner = target_frags.iter().any(|frag| {
            matches!(frag.surface, Surface::Plane { .. }) && !frag.inner_polygons_3d.is_empty()
        });
        assert!(
            has_inner,
            "A1 partition: at least one Plane fragment should have inner_polygons_3d (circular hole)"
        );
    }

    /// Tx3 — segments_are_interior: circle chords inside a square are detected as interior.
    #[test]
    fn tx3_segments_are_interior_unit() {
        // 4×4 square centered at origin
        let outer: Vec<(f64, f64)> = vec![(-2.0, -2.0), (2.0, -2.0), (2.0, 2.0), (-2.0, 2.0)];

        // Simulate circle chord endpoints at radius 1.0 inside the square
        let n = 64;
        let segs: Vec<IntersectionSegment> = (0..n)
            .map(|k| {
                let ta = k as f64 * 2.0 * std::f64::consts::PI / n as f64;
                let tb = (k + 1) as f64 * 2.0 * std::f64::consts::PI / n as f64;
                IntersectionSegment {
                    p_start: (ta.cos(), ta.sin()),
                    p_end: (tb.cos(), tb.sin()),
                    partner: EntityRef::try_named("cyl", EntityKind::Face, "lateral").unwrap(),
                    source_curve_3d: None,
                    curve_3d_t_range: [ta, tb],
                    pcurve_on_a: None,
                    pcurve_on_a_t_range: [0.0, 0.0],
                    pcurve_on_b: None,
                    pcurve_on_b_t_range: [0.0, 0.0],
                }
            })
            .collect();

        let refs: Vec<&IntersectionSegment> = segs.iter().collect();
        assert!(
            segments_are_interior(&refs, &outer),
            "circle r=1 inside 4×4 square should be interior"
        );
    }
}
