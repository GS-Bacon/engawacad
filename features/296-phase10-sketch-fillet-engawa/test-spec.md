# test-spec: #296 Sketch Fillet

STEP 6-A (GLM core impl) の実装差分を確認済み。plan.md 通りに実装されている
(`compute_fillet` / `find_adjacent_pair` / `apply_sketch_fillet_build` の3関数構成、
error kind 文字列、`sketch_fillet_arc_id_collision` ガード、feature_crud.rs の
element-level gate (`refs_resolve_in_state` / `check_refs_resolve_before` /
`SketchElementNotResolved`) すべて plan.md 記載通り)。

`crates/engawa-build/tests/sketch_fillet_acceptance.rs` の acceptance skeleton
(STEP 5.5 作成) には plan.md のテスト計画 ID 表に対応する 23 個の `#[ignore]`
stub が既にある。本 STEP 6.6 では **これら全 ID の実装** が主タスク。

## 不足テスト（plan 計画分）

以下 23 関数の `#[ignore]` を外し、`todo!()` を実装に置き換える
(関数名と ID の対応は `sketch_fillet_acceptance.rs` のコメント参照。
各 ID の期待挙動は plan.md 「## テスト計画（ID 付き）」表を参照):

- `t01_determinism_and_derived_arc_id` (T01)
- `t01b_input_order_invariance` (T01b)
- `t01c_arc_id_collision_rejected` (T01c)
- `t01d_build_determinism_with_id_generator` (T01d)
- `t04_build_rectangle_corner_fillet` (T04)
- `t06_extrude_uses_filleted_profile` (T06)
- `t07_closed_loop_wraparound_corner` (T07)
- `t08_cw_profile_negative_sweep` (T08)
- `t09_profile_chain_continuity` (T09)
- `t10_crud_gate_rejects_rename_breaking_fillet` (T10)
- `t11_crud_gate_rejects_insert_with_missing_element` (T11)
- `t12_crud_gate_rejects_reorder_breaking_adjacency` (T12)
- `t_deg_fillet_too_large`
- `t_deg_corner_angle_flat`
- `t_deg_corner_angle_zero`
- `t_deg_no_shared_corner`
- `t_deg_non_line_element_rejected`
- `t_deg_same_element_rejected`
- `t_deg_not_adjacent_rejected`
- `t_deg_elem_not_found`
- `t_deg_negative_radius`
- `t_deg_nan_radius`
- `t_deg_sketch_ref_not_found`

T02/T03 (kernel 90°/60° 解析解) と T05 (roundtrip) は STEP 6-A で GLM が
`crates/engawa-kernel/src/geometry/sketch_fillet.rs::tests` と
`crates/engawa-format/src/feature.rs::tests` に既に実装済み
(acceptance ファイル側は doc-pointer の空 pass のままでよい)。

## 実装差分から追加すべきテスト

- feature_crud.rs の `SketchElementNotResolved` エラー variant のフィールド
  (`feature_id` / `sketch_ref` / `elem1_id` / `elem2_id` / `reason`) が
  T11 で実際に検証されること (単に `is_err()` だけでなく `reason` の値まで assert)。
- `set_feature_suppressed` に `Feature::SketchFillet` arm が追加されている
  (feature_crud.rs:975)。suppressed フラグの toggle が正しく効くことを
  T10-T12 のいずれかで suppressed=false 前提のケースとして暗黙に確認できるが、
  明示的な suppressed テストは plan.md に無い (Non-Goals ではないが本 Issue の
  必須要件でもないため追加不要、次 Issue 以降で気になれば対応)。

## エッジケース・退化入力

plan.md の T_DEG_* で網羅済み。追加で GLM が実装中に気づいた分岐があれば
このファイルではなく `debug-spec.md` (STEP 6-C 相当) または
`claude-self-review.md` (STEP 6.7) で記録すること。

## 数値境界

- `radius <= LENGTH_TOLERANCE` (1e-9) が `InvalidParameter{kind:"radius"}` の
  境界。T_DEG_negative_radius は `radius=-1.0` (明確に負) をテストするのみで
  境界値ちょうど (`radius = LENGTH_TOLERANCE`) のテストは plan.md 未記載。
  厳密性を高めるなら追加してよいが必須ではない (Non-Goal ではないが低優先度)。
- corner 角度 `theta` の境界 (`ANGLE_TOLERANCE` および `π - ANGLE_TOLERANCE`
  ちょうど) も T_DEG_corner_angle_flat/zero は「ほぼ」180°/0° (境界の内側)
  でテストする想定。ちょうど境界値のテストは同様に追加不要。

## 決定性

T01 (kernel 純関数の2回実行一致 + 派生 ID 文字列一致) と T01d (IdGenerator
込みの build 全体2回一致) の2段構えで、`built_sketch_profiles` の状態遷移と
下流 `make_extrusion` の EntityId 発番の両方をカバーする。T01b は入力順序
不変性 (elem1_id/elem2_id の入れ替え) を担当し、3者で決定性要件を分担する。
