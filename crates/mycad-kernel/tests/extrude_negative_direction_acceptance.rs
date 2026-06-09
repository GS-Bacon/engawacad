/// Acceptance tests for Issue #110: negative-offset face extrusion extends outward.
///
/// All tests are initially `#[ignore]`-tagged; STEP 6 (GLM) removes the tag
/// once the corresponding implementation is in place.

#[test]
#[ignore = "STEP 6 で実装後に解除"]
fn t01_kernel_neg_depth_determinism() {
    // 負の depth を持つ make_extrusion を同 seed で 2 回実行し、
    // 全頂点 ID・座標が一致することを確認（決定性保証）
    todo!()
}

#[test]
#[ignore = "STEP 6 で実装後に解除"]
fn t02_kernel_neg_depth_manifold() {
    // 負の depth での押し出し solid が:
    // - validate_manifold() を通る
    // - V - E + F == 2 (Euler-Poincaré) を満たす
    // - top 頂点が -plane.normal 側に配置されている
    todo!()
}

#[test]
#[ignore = "STEP 6 で実装後に解除"]
fn t_boundary_zero_depth_rejected() {
    // depth = 0.0 (abs <= length_eps) を make_extrusion に渡す → Err(InvalidParameter)
    todo!()
}

#[test]
#[ignore = "STEP 6 で実装後に解除"]
fn t_degen_nonfinite_depth_rejected() {
    // depth = f64::NAN, f64::INFINITY, f64::NEG_INFINITY → Err(InvalidParameter)
    todo!()
}
