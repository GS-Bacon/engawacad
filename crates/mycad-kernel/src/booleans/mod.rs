pub use self::types::BooleanOp;

mod assemble;
mod classify;
mod partition;
mod types;

use crate::brep::topology::{IdGenerator, Solid};
use crate::error::KernelError;
use crate::geometry::math::LENGTH_TOLERANCE;
use crate::geometry::surface::Surface;

/// Perform a boolean operation between two planar solids.
pub fn boolean_planar(
    target: &Solid,
    tool: &Solid,
    op: BooleanOp,
    id_gen: &mut IdGenerator,
) -> Result<Solid, KernelError> {
    // Validate inputs in fixed order (Codex R03)
    validate_boolean_input(target, "target")?;
    validate_boolean_input(tool, "tool")?;

    // Phase A: Partition
    let (target_fragments, tool_fragments) =
        partition::partition_faces(target, tool, op).map_err(KernelError::BooleanInternal)?;

    if target_fragments.is_empty() && tool_fragments.is_empty() {
        return Err(KernelError::EmptyBooleanResult);
    }

    // Phase B: Classify
    let classified =
        classify::classify_fragments(&target_fragments, &tool_fragments, target, tool, op, &[])
            .map_err(KernelError::BooleanInternal)?;

    // Phase C: Assemble
    let result = assemble::assemble(&classified, op, id_gen)?;

    Ok(result)
}

fn validate_boolean_input(solid: &Solid, _label: &str) -> Result<(), KernelError> {
    // 1. validate_manifold
    solid
        .validate_manifold()
        .map_err(|_| KernelError::OpenBooleanInput)?;

    // 2. All faces must be planar
    for face in &solid.faces {
        if !matches!(face.surface, Surface::Plane { .. }) {
            return Err(KernelError::NonPlanarBooleanInput {
                kind: "non-planar surface",
            });
        }
    }

    // 3. All faces must have empty inner_loops
    for face in &solid.faces {
        if !face.inner_loops.is_empty() {
            return Err(KernelError::UnsupportedBooleanInput {
                reason: "input face has inner loops",
            });
        }
    }

    // 4. All outer loops must be convex
    let area_eps = LENGTH_TOLERANCE * LENGTH_TOLERANCE;
    for face in &solid.faces {
        let lp = &solid.loops[face.outer_loop];
        let points: Vec<_> = lp
            .half_edges
            .iter()
            .map(|&he_idx| solid.vertices[solid.half_edges[he_idx].start_vertex].point)
            .collect();
        if !is_convex_polygon_3d(&points, area_eps) {
            return Err(KernelError::UnsupportedBooleanInput {
                reason: "input face has non-convex outer loop",
            });
        }
    }

    // 5. All faces/edges/vertices must have names
    for face in &solid.faces {
        if face.name.is_none() {
            return Err(KernelError::MissingEntityName);
        }
    }
    for edge in &solid.edges {
        if edge.name.is_none() {
            return Err(KernelError::MissingEntityName);
        }
    }
    for vertex in &solid.vertices {
        if vertex.name.is_none() {
            return Err(KernelError::MissingEntityName);
        }
    }

    Ok(())
}

fn is_convex_polygon_3d(points: &[crate::geometry::Point], area_eps: f64) -> bool {
    let n = points.len();
    if n < 3 {
        return false;
    }

    // Project to 2D using the dominant axis
    // Compute normal via Newell's method
    let mut normal = crate::geometry::Vec3::new(0.0, 0.0, 0.0);
    for i in 0..n {
        let j = (i + 1) % n;
        normal.x += (points[i].y - points[j].y) * (points[i].z + points[j].z);
        normal.y += (points[i].z - points[j].z) * (points[i].x + points[j].x);
        normal.z += (points[i].x - points[j].x) * (points[i].y + points[j].y);
    }

    let (u_idx, v_idx) = if normal.x.abs() >= normal.y.abs() && normal.x.abs() >= normal.z.abs() {
        (1, 2)
    } else if normal.y.abs() >= normal.z.abs() {
        (0, 2)
    } else {
        (0, 1)
    };

    // Check convexity in 2D
    let mut first_sign: Option<f64> = None;
    for i in 0..n {
        let prev = (i + n - 1) % n;
        let next = (i + 1) % n;
        let d1 = (
            points[i].coords[u_idx] - points[prev].coords[u_idx],
            points[i].coords[v_idx] - points[prev].coords[v_idx],
        );
        let d2 = (
            points[next].coords[u_idx] - points[i].coords[u_idx],
            points[next].coords[v_idx] - points[i].coords[v_idx],
        );
        let cross = d1.0 * d2.1 - d1.1 * d2.0;
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
