//! Acceptance test skeleton for #296 (Sketch Fillet)
//! テスト計画: features/296-phase10-sketch-fillet-engawa/plan.md 「## テスト計画（ID 付き）」参照
//! STEP 6 で GLM が #[ignore] を解除し実装する。

use engawa_build::build_bodies_from_features;
use engawa_format::{Feature, SketchElement};
use engawa_kernel::brep::topology::IdGenerator;

#[test]
#[ignore = "STEP 6 で実装後に解除"]
fn t01_determinism_and_derived_arc_id() {
    todo!()
}

#[test]
#[ignore = "STEP 6 で実装後に解除"]
fn t01b_input_order_invariance() {
    todo!()
}

#[test]
#[ignore = "STEP 6 で実装後に解除"]
fn t01c_arc_id_collision_rejected() {
    todo!()
}

#[test]
#[ignore = "STEP 6 で実装後に解除"]
fn t01d_build_determinism_with_id_generator() {
    todo!()
}

/// T02: 90°コーナーの解析解検証は kernel level (compute_fillet 単体テスト) で行う。
/// See `crates/engawa-kernel/src/geometry/sketch_fillet.rs::tests` (STEP 6 で GLM が追加)。
#[test]
fn t02_normal_90deg_covered_at_kernel_level() {}

/// T03: 60°コーナーの解析解検証は kernel level で行う。
/// See `crates/engawa-kernel/src/geometry/sketch_fillet.rs::tests` (STEP 6 で GLM が追加)。
#[test]
fn t03_normal_60deg_covered_at_kernel_level() {}

#[test]
#[ignore = "STEP 6 で実装後に解除"]
fn t04_build_rectangle_corner_fillet() {
    todo!()
}

/// T05: Feature::SketchFillet の YAML roundtrip は engawa-format 側の inline test で行う。
/// See `crates/engawa-format/src/feature.rs::tests` (STEP 6 で GLM が追加)。
#[test]
fn t05_roundtrip_covered_at_format_level() {}

#[test]
#[ignore = "STEP 6 で実装後に解除"]
fn t06_extrude_uses_filleted_profile() {
    todo!()
}

#[test]
#[ignore = "STEP 6 で実装後に解除"]
fn t07_closed_loop_wraparound_corner() {
    todo!()
}

#[test]
#[ignore = "STEP 6 で実装後に解除"]
fn t08_cw_profile_negative_sweep() {
    todo!()
}

#[test]
#[ignore = "STEP 6 で実装後に解除"]
fn t09_profile_chain_continuity() {
    todo!()
}

#[test]
#[ignore = "STEP 6 で実装後に解除"]
fn t10_crud_gate_rejects_rename_breaking_fillet() {
    todo!()
}

#[test]
#[ignore = "STEP 6 で実装後に解除"]
fn t11_crud_gate_rejects_insert_with_missing_element() {
    todo!()
}

#[test]
#[ignore = "STEP 6 で実装後に解除"]
fn t12_crud_gate_rejects_reorder_breaking_adjacency() {
    todo!()
}

#[test]
#[ignore = "STEP 6 で実装後に解除"]
fn t_deg_fillet_too_large() {
    todo!()
}

#[test]
#[ignore = "STEP 6 で実装後に解除"]
fn t_deg_corner_angle_flat() {
    todo!()
}

#[test]
#[ignore = "STEP 6 で実装後に解除"]
fn t_deg_corner_angle_zero() {
    todo!()
}

#[test]
#[ignore = "STEP 6 で実装後に解除"]
fn t_deg_no_shared_corner() {
    todo!()
}

#[test]
#[ignore = "STEP 6 で実装後に解除"]
fn t_deg_non_line_element_rejected() {
    todo!()
}

#[test]
#[ignore = "STEP 6 で実装後に解除"]
fn t_deg_same_element_rejected() {
    todo!()
}

#[test]
#[ignore = "STEP 6 で実装後に解除"]
fn t_deg_not_adjacent_rejected() {
    todo!()
}

#[test]
#[ignore = "STEP 6 で実装後に解除"]
fn t_deg_elem_not_found() {
    todo!()
}

#[test]
#[ignore = "STEP 6 で実装後に解除"]
fn t_deg_negative_radius() {
    todo!()
}

#[test]
#[ignore = "STEP 6 で実装後に解除"]
fn t_deg_nan_radius() {
    todo!()
}

#[test]
#[ignore = "STEP 6 で実装後に解除"]
fn t_deg_sketch_ref_not_found() {
    todo!()
}

// 未使用 import 抑制 (STEP 6 で実装が入れば自然に使われる)
#[allow(dead_code)]
fn _unused_import_anchor() {
    let _ = build_bodies_from_features
        as fn(
            &[Feature],
            &[engawa_format::RefPlane],
            &mut IdGenerator,
        ) -> Result<engawa_build::BuiltBodies, engawa_kernel::error::KernelError>;
    let _: Option<SketchElement> = None;
}
