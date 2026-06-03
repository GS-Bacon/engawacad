use crate::brep::topology::{IdGenerator, Solid};
use crate::error::KernelError;
use crate::geometry::curve::Curve;
use crate::geometry::math::orthonormal_basis;
use crate::geometry::math::LENGTH_TOLERANCE;
use crate::geometry::surface::Surface;
use crate::geometry::Point;
use mycad_format::EntityRef;

use super::classify::ClassifiedFragment;
use super::types::{BooleanOp, FragmentLabel};

pub fn assemble(
    classified: &[ClassifiedFragment],
    op: BooleanOp,
    id_gen: &mut IdGenerator,
) -> Result<Solid, KernelError> {
    let len_eps = LENGTH_TOLERANCE;
    let area_eps = LENGTH_TOLERANCE * LENGTH_TOLERANCE;

    // Select fragments based on operation
    let mut selected: Vec<&ClassifiedFragment> = Vec::new();

    for cf in classified {
        let is_tool = cf.fragment.is_tool_side;
        let should_select = matches!(
            (op, is_tool, cf.label),
            (BooleanOp::Cut, false, FragmentLabel::OutsideOther)
                | (
                    BooleanOp::Cut,
                    false,
                    FragmentLabel::SharedOppositeDirection
                )
                | (BooleanOp::Cut, true, FragmentLabel::InsideOther)
                | (BooleanOp::Fuse, false, FragmentLabel::OutsideOther)
                | (BooleanOp::Fuse, false, FragmentLabel::SharedSameDirection)
                | (BooleanOp::Fuse, true, FragmentLabel::OutsideOther)
                | (BooleanOp::Intersect, false, FragmentLabel::InsideOther)
                | (
                    BooleanOp::Intersect,
                    false,
                    FragmentLabel::SharedSameDirection
                )
                | (BooleanOp::Intersect, true, FragmentLabel::InsideOther)
        );

        if should_select {
            selected.push(cf);
        }
    }

    if selected.is_empty() {
        return Err(KernelError::EmptyBooleanResult);
    }

    // Merge vertices
    let mut vertices: Vec<(Point, Option<EntityRef>)> = Vec::new();
    let mut vertex_map: Vec<usize> = Vec::new();

    let mut all_points: Vec<(Point, Option<EntityRef>)> = Vec::new();
    for cf in &selected {
        for (vi, p) in cf.fragment.polygon_3d.iter().enumerate() {
            let name = derive_vertex_name(&cf.fragment, vi, op);
            all_points.push((*p, name));
        }
        for inner_poly in &cf.fragment.inner_polygons_3d {
            for p in inner_poly {
                all_points.push((*p, None));
            }
        }
    }

    for (point, name) in &all_points {
        let existing = vertices
            .iter()
            .position(|(vp, _)| (vp - point).norm() < len_eps);
        match existing {
            Some(idx) => vertex_map.push(idx),
            None => {
                let idx = vertices.len();
                vertices.push((*point, name.clone()));
                vertex_map.push(idx);
            }
        }
    }

    let mut solid = Solid::new(id_gen.next());

    for (point, name) in &vertices {
        solid.add_vertex(id_gen.next(), *point, name.clone());
    }

    // Build fragment-local vertex index lists (outer + inner polygons)
    let mut frag_vertex_indices: Vec<Vec<usize>> = Vec::new();
    let mut frag_inner_vertex_indices: Vec<Vec<Vec<usize>>> = Vec::new();
    let mut point_offset = 0;
    for cf in &selected {
        let n = cf.fragment.polygon_3d.len();
        let mut local_vi: Vec<usize> = Vec::new();
        for i in 0..n {
            local_vi.push(vertex_map[point_offset + i]);
        }
        frag_vertex_indices.push(local_vi);
        point_offset += n;
        let mut inner_vi_list: Vec<Vec<usize>> = Vec::new();
        for inner_poly in &cf.fragment.inner_polygons_3d {
            let m = inner_poly.len();
            let mut inner_vi: Vec<usize> = Vec::new();
            for i in 0..m {
                inner_vi.push(vertex_map[point_offset + i]);
            }
            inner_vi_list.push(inner_vi);
            point_offset += m;
        }
        frag_inner_vertex_indices.push(inner_vi_list);
    }

    // Collect all unique undirected edges across all selected fragments
    // Key: normalized (min_vertex, max_vertex), Value: edge index in solid
    let mut edge_map: std::collections::HashMap<[usize; 2], usize> =
        std::collections::HashMap::new();

    // Collect intersection edge candidates for naming: (fragment_idx, edge_local_idx)
    // to look up partner provenance from boundary_partners.
    let mut edge_fragment_partner: std::collections::HashMap<[usize; 2], Vec<(usize, usize)>> =
        std::collections::HashMap::new();

    for (fi, cf) in selected.iter().enumerate() {
        let vis = &frag_vertex_indices[fi];
        let n = vis.len();

        for k in 0..n {
            let i = k;
            let j = (k + 1) % n;
            let vi_start = vis[i];
            let vi_end = vis[j];
            let key = normalize_edge_key(vi_start, vi_end);

            // Track provenance for intersection edge naming
            if cf
                .fragment
                .boundary_partners
                .get(k)
                .is_some_and(|p| p.is_some())
            {
                edge_fragment_partner.entry(key).or_default().push((fi, k));
            }
        }
    }

    // Assign intersection edge names using provenance
    let mut intersection_edge_names: std::collections::HashMap<[usize; 2], EntityRef> =
        std::collections::HashMap::new();
    {
        // Collect candidates: (op, canonical_parents, edge_key, frag_idx, edge_local_idx)
        let mut candidates: Vec<(BooleanOp, Vec<EntityRef>, [usize; 2])> = Vec::new();
        for (&key, entries) in &edge_fragment_partner {
            let mut partners_seen: Vec<EntityRef> = Vec::new();
            for &(fi, k) in entries {
                if let Some(Some(partner)) = selected[fi].fragment.boundary_partners.get(k) {
                    partners_seen.push(partner.clone());
                }
            }
            if !partners_seen.is_empty() {
                let canonical = canonicalize_provenance(&partners_seen, op);
                candidates.push((op, canonical, key));
            }
        }

        let selectors = assign_intersection_edge_selectors(&candidates);
        for (op, canonical, key) in &candidates {
            if let Some(selector) = selectors.get(key) {
                if let Ok(name) = derive_edge_name(canonical, *op, selector) {
                    intersection_edge_names.insert(*key, name);
                }
            }
        }
    }

    for (fi, cf) in selected.iter().enumerate() {
        let vis = &frag_vertex_indices[fi];
        let n = vis.len();

        for k in 0..n {
            let i = k;
            let j = (k + 1) % n;
            let vi_start = vis[i];
            let vi_end = vis[j];
            let key = normalize_edge_key(vi_start, vi_end);
            if let std::collections::hash_map::Entry::Vacant(e) = edge_map.entry(key) {
                let p0 = solid.vertices[vi_start].point;
                let p1 = solid.vertices[vi_end].point;
                let edge_name = intersection_edge_names.get(&key).cloned();
                let (curve, t_range) =
                    circle_curve_for_edge(cf.fragment.boundary_curves.get(k), &p0, &p1);
                let edge_idx =
                    solid.add_edge(id_gen.next(), [vi_start, vi_end], curve, t_range, edge_name);
                e.insert(edge_idx);
            }
        }
    }

    // Build edges for inner polygon boundaries (ring face holes)
    for (fi, cf) in selected.iter().enumerate() {
        for (ii, inner_vis) in frag_inner_vertex_indices[fi].iter().enumerate() {
            let m = inner_vis.len();
            for k in 0..m {
                let vi_start = inner_vis[k];
                let vi_end = inner_vis[(k + 1) % m];
                let key = normalize_edge_key(vi_start, vi_end);
                if let std::collections::hash_map::Entry::Vacant(e) = edge_map.entry(key) {
                    let p0 = solid.vertices[vi_start].point;
                    let p1 = solid.vertices[vi_end].point;
                    let inner_curve_opt = cf
                        .fragment
                        .inner_boundary_curves
                        .get(ii)
                        .and_then(|curves| curves.get(k));
                    let (curve, t_range) = circle_curve_for_edge(inner_curve_opt, &p0, &p1);
                    let edge_idx =
                        solid.add_edge(id_gen.next(), [vi_start, vi_end], curve, t_range, None);
                    e.insert(edge_idx);
                }
            }
        }
    }

    // Build faces: for each edge, track if the forward HE has been created
    // Key: edge index, Value: (forward_he_created, reverse_he_created)
    let mut he_forward_created: std::collections::HashMap<usize, usize> =
        std::collections::HashMap::new();
    let mut he_reverse_created: std::collections::HashMap<usize, usize> =
        std::collections::HashMap::new();

    let mut face_indices = Vec::new();

    for (fi, cf) in selected.iter().enumerate() {
        let frag = &cf.fragment;
        let vis = &frag_vertex_indices[fi];
        let n = vis.len();
        if n < 3 && !matches!(frag.surface, Surface::Sphere { .. }) {
            continue;
        }

        let flip_normals = matches!(
            (op, cf.fragment.is_tool_side, cf.label),
            (BooleanOp::Cut, true, _)
        );

        let mut he_indices = Vec::with_capacity(n);

        for k in 0..n {
            let i = k;
            let j = (k + 1) % n;
            let vi_start = vis[i];
            let vi_end = vis[j];

            let edge_idx = edge_map[&normalize_edge_key(vi_start, vi_end)];
            let edge = &solid.edges[edge_idx];

            // Determine if this traversal goes forward along the edge
            let goes_forward = edge.vertices[0] == vi_start && edge.vertices[1] == vi_end;

            // Check which HE slot to use
            let he_idx = if goes_forward {
                if let Some(&existing) = he_forward_created.get(&edge_idx) {
                    existing
                } else {
                    let idx = solid.add_half_edge(id_gen.next(), vi_start, edge_idx, true);
                    he_forward_created.insert(edge_idx, idx);
                    idx
                }
            } else if let Some(&existing) = he_reverse_created.get(&edge_idx) {
                existing
            } else {
                let idx = solid.add_half_edge(id_gen.next(), vi_start, edge_idx, false);
                he_reverse_created.insert(edge_idx, idx);
                idx
            };

            he_indices.push(he_idx);
        }

        let loop_idx = solid.add_loop(id_gen.next(), he_indices);

        let face_name = derive_face_name(frag, op);

        let mut frag_surface = frag.surface.clone();
        let same_sense = if flip_normals {
            // Flip the surface normal so signed_volume gives negative volume (void shell).
            // reverse_face_orientation will flip it back and toggle same_sense.
            if let Surface::Plane { normal, .. } = &mut frag_surface {
                *normal = -*normal;
            }
            // Curved surfaces (sphere, cylinder) need inward-pointing face normals
            // so they contribute negative signed volume for void regions.
            // Plane normals are already negated above, so same_sense=true is correct for planes.
            !matches!(
                frag_surface,
                Surface::Sphere { .. } | Surface::Cylinder { .. }
            )
        } else {
            true
        };

        let face_idx = solid.add_face(
            id_gen.next(),
            frag_surface,
            loop_idx,
            vec![],
            same_sense,
            face_name,
        );
        face_indices.push(face_idx);

        // Build inner loops for ring fragments (faces with circular holes)
        for inner_vis in &frag_inner_vertex_indices[fi] {
            if inner_vis.len() < 3 {
                continue;
            }
            let m = inner_vis.len();
            let mut inner_he_indices = Vec::with_capacity(m);
            for k in 0..m {
                let vi_start = inner_vis[k];
                let vi_end = inner_vis[(k + 1) % m];
                let key = normalize_edge_key(vi_start, vi_end);
                let edge_idx = edge_map[&key];
                let edge = &solid.edges[edge_idx];
                let goes_forward = edge.vertices[0] == vi_start && edge.vertices[1] == vi_end;
                let he_idx = if goes_forward {
                    if let Some(&existing) = he_forward_created.get(&edge_idx) {
                        existing
                    } else {
                        let idx = solid.add_half_edge(id_gen.next(), vi_start, edge_idx, true);
                        he_forward_created.insert(edge_idx, idx);
                        idx
                    }
                } else if let Some(&existing) = he_reverse_created.get(&edge_idx) {
                    existing
                } else {
                    let idx = solid.add_half_edge(id_gen.next(), vi_start, edge_idx, false);
                    he_reverse_created.insert(edge_idx, idx);
                    idx
                };
                inner_he_indices.push(he_idx);
            }
            let inner_loop_idx = solid.add_loop(id_gen.next(), inner_he_indices);
            solid.faces[face_idx].inner_loops.push(inner_loop_idx);
        }
    }

    // Build shells by connected components
    let shells = find_connected_shells(&solid, &face_indices);
    if shells.is_empty() {
        return Err(KernelError::EmptyBooleanResult);
    }

    // Classify shells by signed volume
    let mut positive_shells = Vec::new();
    let mut negative_shells = Vec::new();

    for shell_faces in &shells {
        let vol = signed_volume(&solid, shell_faces);
        if vol > area_eps {
            positive_shells.push(shell_faces.clone());
        } else if vol < -area_eps {
            negative_shells.push(shell_faces.clone());
        }
    }

    match op {
        BooleanOp::Cut => {
            if positive_shells.len() > 1 {
                return Err(KernelError::MultipleOuterShellsResult { op: "cut" });
            }
        }
        BooleanOp::Fuse => {
            if positive_shells.len() > 1 {
                return Err(KernelError::DisjointFuseResult);
            }
        }
        BooleanOp::Intersect => {
            if positive_shells.len() > 1 {
                return Err(KernelError::MultipleOuterShellsResult { op: "intersect" });
            }
        }
    }

    if positive_shells.is_empty() {
        if !negative_shells.is_empty() {
            return Err(KernelError::DegenerateBooleanContact);
        }
        return Err(KernelError::EmptyBooleanResult);
    }

    // Sort shells for determinism: sort each shell's face list, then sort shells by first face index
    let mut sorted_positive: Vec<Vec<usize>> = positive_shells;
    for shell in &mut sorted_positive {
        shell.sort_unstable();
    }
    sorted_positive.sort_by_key(|s| s[0]);

    let mut sorted_negative: Vec<Vec<usize>> = negative_shells;
    for shell in &mut sorted_negative {
        shell.sort_unstable();
    }
    sorted_negative.sort_by_key(|s| s[0]);

    // Add outer shell first
    solid.add_shell(id_gen.next(), sorted_positive[0].clone(), true);

    // Handle void shells
    for void_shell in &sorted_negative {
        for &fi in void_shell {
            // Self-adjacent periodic sphere faces already have correct same_sense=false
            // from flip_normals — skip reverse_face_orientation to preserve it.
            let is_periodic_sphere = {
                let face = &solid.faces[fi];
                matches!(face.surface, Surface::Sphere { .. })
                    && solid.loops[face.outer_loop].half_edges.len() < 3
            };
            if !is_periodic_sphere {
                reverse_face_orientation(&mut solid, fi, id_gen);
            }
        }
        solid.add_shell(id_gen.next(), void_shell.clone(), true);
    }

    // Shrink: remove orphan half-edges and loops
    shrink_solid(&mut solid);

    // Validate
    solid
        .validate_manifold()
        .map_err(|e| KernelError::BooleanInternal(format!("manifold validation failed: {}", e)))?;

    Ok(solid)
}

/// Build edge curve and t_range from an optional source Curve::Circle.
/// If the source is a circle, computes per-chord arc angles from 3D positions.
/// Falls back to Curve::Line otherwise.
fn circle_curve_for_edge(src: Option<&Option<Curve>>, p0: &Point, p1: &Point) -> (Curve, [f64; 2]) {
    if let Some(Some(Curve::Circle {
        center,
        normal,
        radius,
    })) = src
    {
        let (u_ax, v_ax) = orthonormal_basis(normal);
        let d0 = p0.coords - center.coords;
        let d1 = p1.coords - center.coords;
        let t0 = d0.dot(&v_ax).atan2(d0.dot(&u_ax));
        let mut t1 = d1.dot(&v_ax).atan2(d1.dot(&u_ax));
        // Unwrap t1 so the arc goes in the shorter direction from t0.
        // For a single chord (≤ π arc), this puts t1 just past t0.
        if t1 < t0 {
            t1 += 2.0 * std::f64::consts::PI;
        }
        if t1 - t0 > std::f64::consts::PI {
            t1 -= 2.0 * std::f64::consts::PI; // prefer shorter arc (handles CW boundary)
        }
        (
            Curve::Circle {
                center: *center,
                normal: *normal,
                radius: *radius,
            },
            [t0, t1],
        )
    } else {
        let direction = p1 - p0;
        (
            Curve::Line {
                origin: *p0,
                direction,
            },
            [0.0, 1.0],
        )
    }
}

fn normalize_edge_key(v0: usize, v1: usize) -> [usize; 2] {
    if v0 < v1 {
        [v0, v1]
    } else {
        [v1, v0]
    }
}

fn reverse_face_orientation(solid: &mut Solid, face_idx: usize, id_gen: &mut IdGenerator) {
    let face = &solid.faces[face_idx];
    let loop_idx = face.outer_loop;
    let lp = &solid.loops[loop_idx];

    let mut vertex_seq: Vec<usize> = Vec::new();
    for &he_idx in &lp.half_edges {
        let he = &solid.half_edges[he_idx];
        vertex_seq.push(he.start_vertex);
    }

    vertex_seq.reverse();

    // Build new half-edges using the shared edge approach
    let mut new_he_indices = Vec::with_capacity(vertex_seq.len());
    for k in 0..vertex_seq.len() {
        let vi_start = vertex_seq[k];
        let vi_end = vertex_seq[(k + 1) % vertex_seq.len()];

        // Find the existing edge for this vertex pair
        let key = normalize_edge_key(vi_start, vi_end);
        let edge_idx = find_edge_by_vertices(&solid.edges, &key);

        let goes_forward = solid.edges[edge_idx].vertices[0] == vi_start
            && solid.edges[edge_idx].vertices[1] == vi_end;

        let he_idx = solid.add_half_edge(id_gen.next(), vi_start, edge_idx, goes_forward);
        new_he_indices.push(he_idx);
    }

    solid.loops[loop_idx].half_edges = new_he_indices;
    solid.faces[face_idx].same_sense = !solid.faces[face_idx].same_sense;
}

fn find_edge_by_vertices(edges: &[crate::brep::topology::Edge], key: &[usize; 2]) -> usize {
    for (i, e) in edges.iter().enumerate() {
        let ek = normalize_edge_key(e.vertices[0], e.vertices[1]);
        if ek == *key {
            return i;
        }
    }
    unreachable!("edge for vertex pair must exist")
}

fn find_connected_shells(solid: &Solid, face_indices: &[usize]) -> Vec<Vec<usize>> {
    let n = face_indices.len();
    if n == 0 {
        return Vec::new();
    }

    let mut face_edges: std::collections::HashMap<usize, Vec<[usize; 2]>> =
        std::collections::HashMap::new();

    for &fi in face_indices {
        let face = &solid.faces[fi];
        let mut edges = Vec::new();
        // Include outer loop and all inner loops so ring faces connect
        // to adjacent faces through inner-loop (hole) edges.
        let all_loops = std::iter::once(face.outer_loop).chain(face.inner_loops.iter().copied());
        for lp_idx in all_loops {
            let lp = &solid.loops[lp_idx];
            for &he_idx in &lp.half_edges {
                let he = &solid.half_edges[he_idx];
                let edge = &solid.edges[he.edge];
                let v0 = edge.vertices[0].min(edge.vertices[1]);
                let v1 = edge.vertices[0].max(edge.vertices[1]);
                edges.push([v0, v1]);
            }
        }
        face_edges.insert(fi, edges);
    }

    let mut parent: Vec<usize> = (0..n).collect();

    fn find(parent: &mut [usize], i: usize) -> usize {
        if parent[i] != i {
            parent[i] = find(parent, parent[i]);
        }
        parent[i]
    }

    for i in 0..n {
        for j in (i + 1)..n {
            let fi = face_indices[i];
            let fj = face_indices[j];
            let edges_i = &face_edges[&fi];
            let edges_j = &face_edges[&fj];

            let adjacent = edges_i.iter().any(|ei| edges_j.iter().any(|ej| ei == ej));
            if adjacent {
                let pi = find(&mut parent, i);
                let pj = find(&mut parent, j);
                if pi != pj {
                    parent[pi] = pj;
                }
            }
        }
    }

    let mut groups: std::collections::BTreeMap<usize, Vec<usize>> =
        std::collections::BTreeMap::new();
    for (i, &fi) in face_indices.iter().enumerate().take(n) {
        let root = find(&mut parent, i);
        groups.entry(root).or_default().push(fi);
    }

    groups.into_values().collect()
}

fn signed_volume(solid: &Solid, face_indices: &[usize]) -> f64 {
    let mut volume = 0.0;
    let area_eps = 1e-14;

    for &fi in face_indices {
        let face = &solid.faces[fi];

        // Get the effective outward normal considering same_sense.
        let face_normal = face
            .surface
            .normal_at_point(&face.surface.evaluate(0.0, 0.0));
        let effective_normal = if face.same_sense {
            face_normal
        } else {
            -face_normal
        };

        let lp = &solid.loops[face.outer_loop];

        let verts: Vec<Point> = lp
            .half_edges
            .iter()
            .map(|&he_idx| {
                let he = &solid.half_edges[he_idx];
                solid.vertices[he.start_vertex].point
            })
            .collect();

        if verts.len() < 3 {
            // Self-adjacent periodic sphere face: use analytical volume
            if let Surface::Sphere { radius, .. } = &face.surface {
                let sphere_vol = (4.0 / 3.0) * std::f64::consts::PI * radius * radius * radius;
                let sign = if face.same_sense { 1.0 } else { -1.0 };
                volume += sign * sphere_vol;
            }
            continue;
        }

        match &face.surface {
            Surface::Plane { .. } => {
                let v0 = verts[0];
                for i in 1..verts.len() - 1 {
                    let v1 = verts[i];
                    let v2 = verts[i + 1];

                    let a = v1 - v0;
                    let b = v2 - v0;
                    let cross = a.cross(&b);
                    let cross_norm = cross.norm();
                    if cross_norm < area_eps {
                        continue;
                    }
                    let sign = if effective_normal.dot(&(cross / cross_norm)) > 0.0 {
                        1.0_f64
                    } else {
                        -1.0_f64
                    };
                    volume += sign * v0.coords.dot(&cross) / 6.0;
                }
            }
            _ => {
                // Curved face: use fan triangulation from loop vertices.
                // For volume estimation this is sufficient — the signed tetrahedral
                // volume formula works regardless of surface curvature.
                let v0 = verts[0];
                for i in 1..verts.len() - 1 {
                    let v1 = verts[i];
                    let v2 = verts[i + 1];

                    let a = v1 - v0;
                    let b = v2 - v0;
                    let cross = a.cross(&b);
                    let cross_norm = cross.norm();
                    if cross_norm < area_eps {
                        continue;
                    }
                    let sign = if effective_normal.dot(&(cross / cross_norm)) > 0.0 {
                        1.0_f64
                    } else {
                        -1.0_f64
                    };
                    volume += sign * v0.coords.dot(&cross) / 6.0;
                }
            }
        }
    }

    volume
}

fn shrink_solid(solid: &mut Solid) {
    let mut used_loops: std::collections::HashSet<usize> = std::collections::HashSet::new();
    let mut used_half_edges: std::collections::HashSet<usize> = std::collections::HashSet::new();

    for face in &solid.faces {
        used_loops.insert(face.outer_loop);
        for &il in &face.inner_loops {
            used_loops.insert(il);
        }
    }

    for &li in &used_loops {
        let lp = &solid.loops[li];
        for &he in &lp.half_edges {
            used_half_edges.insert(he);
        }
    }

    // Collect used edges
    let mut used_edges: std::collections::HashSet<usize> = std::collections::HashSet::new();
    for &he_idx in &used_half_edges {
        let he = &solid.half_edges[he_idx];
        used_edges.insert(he.edge);
    }

    // Remove orphan elements: build new arrays and remap indices
    // Sort by ID for determinism
    let mut he_old_to_new: Vec<Option<usize>> = vec![None; solid.half_edges.len()];
    let mut new_half_edges = Vec::new();
    for (old_idx, he) in solid.half_edges.iter().enumerate() {
        if used_half_edges.contains(&old_idx) {
            let new_idx = new_half_edges.len();
            he_old_to_new[old_idx] = Some(new_idx);
            new_half_edges.push(he.clone());
        }
    }
    solid.half_edges = new_half_edges;

    // Remap loop half-edge indices
    for lp in &mut solid.loops {
        let mut new_hes = Vec::new();
        for &old_he in &lp.half_edges {
            if let Some(new_idx) = he_old_to_new[old_he] {
                new_hes.push(new_idx);
            }
        }
        lp.half_edges = new_hes;
    }

    // Remove unused edges
    let mut edge_old_to_new: Vec<Option<usize>> = vec![None; solid.edges.len()];
    let mut new_edges = Vec::new();
    let mut edge_ids: Vec<(usize, usize)> = Vec::new();
    for (old_idx, edge) in solid.edges.iter().enumerate() {
        if used_edges.contains(&old_idx) {
            edge_ids.push((old_idx, edge.id as usize));
        }
    }
    // Sort by ID for determinism
    edge_ids.sort_by_key(|(_, id)| *id);
    for (old_idx, _) in &edge_ids {
        let new_idx = new_edges.len();
        edge_old_to_new[*old_idx] = Some(new_idx);
        new_edges.push(solid.edges[*old_idx].clone());
    }
    solid.edges = new_edges;

    // Remap half-edge edge references
    for he in &mut solid.half_edges {
        if let Some(new_idx) = edge_old_to_new[he.edge] {
            he.edge = new_idx;
        }
    }

    // Remove unused loops
    let mut loop_old_to_new: Vec<Option<usize>> = vec![None; solid.loops.len()];
    let mut new_loops = Vec::new();
    for (old_idx, lp) in solid.loops.iter().enumerate() {
        if used_loops.contains(&old_idx) && !lp.half_edges.is_empty() {
            let new_idx = new_loops.len();
            loop_old_to_new[old_idx] = Some(new_idx);
            new_loops.push(lp.clone());
        }
    }
    solid.loops = new_loops;

    // Remap face loop references
    for face in &mut solid.faces {
        if let Some(new_idx) = loop_old_to_new[face.outer_loop] {
            face.outer_loop = new_idx;
        }
        let mut new_inner = Vec::new();
        for &old_il in &face.inner_loops {
            if let Some(new_idx) = loop_old_to_new[old_il] {
                new_inner.push(new_idx);
            }
        }
        face.inner_loops = new_inner;
    }
}

fn derive_vertex_name(
    frag: &super::partition::FaceFragment,
    vi: usize,
    op: BooleanOp,
) -> Option<EntityRef> {
    let op_str = match op {
        BooleanOp::Cut => "cut_isect_vertex",
        BooleanOp::Fuse => "fuse_isect_vertex",
        BooleanOp::Intersect => "intersect_isect_vertex",
    };
    let selector = format!("v{:04}", vi);
    EntityRef::try_derived(
        mycad_format::EntityKind::Vertex,
        op_str,
        vec![frag.parent_name.clone()],
        &selector,
    )
    .ok()
}

fn derive_face_name(frag: &super::partition::FaceFragment, op: BooleanOp) -> Option<EntityRef> {
    let op_str = op.op_str();
    let selector = format!("frag{:04}", frag.traversal_index);
    EntityRef::try_derived(
        mycad_format::EntityKind::Face,
        op_str,
        vec![frag.parent_name.clone()],
        &selector,
    )
    .ok()
}

/// Derive a stable name for an intersection edge from its provenance (2 parent faces).
/// Returns `Err(MissingIntersectionProvenance)` if parents are empty or insufficient.
pub fn derive_edge_name(
    parents: &[EntityRef],
    op: BooleanOp,
    selector: &str,
) -> Result<EntityRef, KernelError> {
    if parents.is_empty() {
        return Err(KernelError::MissingIntersectionProvenance {
            op: op.isect_op_str().to_string(),
        });
    }
    let canonical = canonicalize_provenance(parents, op);
    EntityRef::try_derived(
        mycad_format::EntityKind::Edge,
        op.isect_op_str(),
        canonical,
        selector,
    )
    .map_err(|_| KernelError::MissingIntersectionProvenance {
        op: op.isect_op_str().to_string(),
    })
}

/// Canonicalize provenance: for commutative ops (Fuse, Intersect), sort parents by canonical_name.
/// For non-commutative ops (Cut), preserve order.
pub fn canonicalize_provenance(parents: &[EntityRef], op: BooleanOp) -> Vec<EntityRef> {
    match op {
        BooleanOp::Cut => parents.to_vec(),
        BooleanOp::Fuse | BooleanOp::Intersect => {
            let mut sorted = parents.to_vec();
            sorted.sort_by_key(|a| a.canonical_name());
            sorted
        }
    }
}

/// Assign deterministic selectors to intersection edges within the same (op, canonical_parents) group.
/// Edges are sorted by edge_key and assigned e0000, e0001, ...
pub fn assign_intersection_edge_selectors(
    candidates: &[(BooleanOp, Vec<EntityRef>, [usize; 2])],
) -> std::collections::HashMap<[usize; 2], String> {
    use std::collections::BTreeMap;

    let mut groups: BTreeMap<String, Vec<[usize; 2]>> = BTreeMap::new();
    for (op, parents, key) in candidates {
        let canonical = canonicalize_provenance(parents, *op);
        let group_key = format!(
            "{}:{}",
            op.isect_op_str(),
            canonical
                .iter()
                .map(|p| p.canonical_name())
                .collect::<Vec<_>>()
                .join(",")
        );
        groups.entry(group_key).or_default().push(*key);
    }

    let mut result = std::collections::HashMap::new();
    for (_, mut edges) in groups {
        edges.sort();
        for (ordinal, key) in edges.into_iter().enumerate() {
            result.insert(key, format!("e{:04}", ordinal));
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    fn named_face(id: &str, role: &str) -> EntityRef {
        EntityRef::try_named(id, mycad_format::EntityKind::Face, role).unwrap()
    }

    // T15: derive_edge_name produces correct canonical_name
    #[test]
    fn t15_derive_edge_name_cut() {
        let fa = named_face("box1", "top");
        let fb = named_face("box2", "front");
        let result = derive_edge_name(&[fa.clone(), fb.clone()], BooleanOp::Cut, "e0001");
        assert!(result.is_ok());
        let name = result.unwrap();
        let canonical = name.canonical_name();
        // Cut is non-commutative: order preserved [fa, fb]
        assert!(canonical.contains("cut_isect_edge"));
        assert!(canonical.contains("e0001"));
    }

    // T15b: derive_edge_name with empty parents returns MissingIntersectionProvenance
    #[test]
    fn t15b_derive_edge_name_empty_parents() {
        let result = derive_edge_name(&[], BooleanOp::Cut, "e0001");
        assert!(matches!(
            result,
            Err(KernelError::MissingIntersectionProvenance { .. })
        ));
    }

    // T16: commutative op sorts parents — [B, A] same as [A, B]
    #[test]
    fn t16_commutative_sort_deterministic() {
        let fa = named_face("a", "top");
        let fb = named_face("b", "front");
        let r1 = derive_edge_name(&[fa.clone(), fb.clone()], BooleanOp::Fuse, "e0001").unwrap();
        let r2 = derive_edge_name(&[fb.clone(), fa.clone()], BooleanOp::Fuse, "e0001").unwrap();
        assert_eq!(r1.canonical_name(), r2.canonical_name());
    }

    // T17: non-commutative op preserves order — [B, A] ≠ [A, B]
    #[test]
    fn t17_non_commutative_preserves_order() {
        let fa = named_face("a", "top");
        let fb = named_face("b", "front");
        let r1 = derive_edge_name(&[fa.clone(), fb.clone()], BooleanOp::Cut, "e0001").unwrap();
        let r2 = derive_edge_name(&[fb.clone(), fa.clone()], BooleanOp::Cut, "e0001").unwrap();
        assert_ne!(r1.canonical_name(), r2.canonical_name());
    }

    // T17b: assign_intersection_edge_selectors produces unique selectors
    #[test]
    fn t17b_assign_selectors_unique() {
        let fa = named_face("a", "top");
        let fb = named_face("b", "front");
        let parents = vec![fa, fb];
        let candidates = vec![
            (BooleanOp::Cut, parents.clone(), [0usize, 5]),
            (BooleanOp::Cut, parents.clone(), [0usize, 3]),
            (BooleanOp::Cut, parents.clone(), [0usize, 1]),
        ];
        let selectors = assign_intersection_edge_selectors(&candidates);
        assert_eq!(selectors.len(), 3);
        // Sorted by key ascending: [0,1], [0,3], [0,5]
        assert_eq!(selectors[&[0, 1]], "e0000");
        assert_eq!(selectors[&[0, 3]], "e0001");
        assert_eq!(selectors[&[0, 5]], "e0002");
    }
}
