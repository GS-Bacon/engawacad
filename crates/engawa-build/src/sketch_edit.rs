use engawa_format::{Feature, SketchElement};
use engawa_kernel::error::KernelError;
use engawa_kernel::geometry::{
    sketch_chamfer, sketch_fillet, sketch_mirror, sketch_offset, sketch_pattern,
};

/// Apply one active sketch edit to the profile produced by its predecessors.
pub(super) fn apply(
    feature: &Feature,
    profile: &[SketchElement],
) -> Result<Vec<SketchElement>, KernelError> {
    match feature {
        Feature::SketchOffset {
            selection,
            distance,
            ..
        } => sketch_offset::apply_sketch_offset_build(profile, selection, *distance),
        Feature::SketchFillet {
            elem1_id,
            elem2_id,
            radius,
            ..
        } => sketch_fillet::apply_sketch_fillet_build(profile, elem1_id, elem2_id, *radius),
        Feature::SketchChamfer {
            elem1_id,
            elem2_id,
            length,
            ..
        } => sketch_chamfer::apply_sketch_chamfer_build(profile, elem1_id, elem2_id, *length),
        Feature::SketchMirror {
            id_suffix,
            axis_p1,
            axis_p2,
            selection,
            ..
        } => sketch_mirror::apply_sketch_mirror_with_suffix(
            profile,
            *axis_p1,
            *axis_p2,
            selection,
            id_suffix.as_deref().unwrap_or("mirror"),
        ),
        Feature::SketchPatternLinear {
            selection,
            count,
            direction,
            distance,
            ..
        } => sketch_pattern::apply_sketch_pattern_linear(
            profile, selection, *count, *direction, *distance,
        ),
        Feature::SketchPatternCircular {
            selection,
            center,
            count,
            total_angle,
            ..
        } => sketch_pattern::apply_sketch_pattern_circular(
            profile,
            selection,
            *center,
            *count,
            *total_angle,
        ),
        _ => unreachable!("expected a sketch edit"),
    }
}
