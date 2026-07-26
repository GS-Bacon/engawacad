## 不足テスト（plan 計画分）

STEP 6-A (core impl) で以下は既に実装・pass 済み（`crates/engawa-kernel/src/geometry/sketch_mirror.rs` inline unit tests + `crates/engawa-build/tests/sketch_mirror_acceptance.rs` の T01-T05）:

- T01 決定性、T02 (Line)、T03 (Circle)、T04 (Arc, STEP 3.5 修正後の式で `start=0,end=-π/2` を検証済み)、T05 (empty selection = all)
- kernel unit tests ではさらに T_DEG 系 8 本 + `t_boundary_arc_endpoints_on_axis` + `t_determinism_100_runs` も pass 済み

**`crates/engawa-build/tests/sketch_mirror_acceptance.rs` に残っている `#[ignore] todo!()` スタブ**（このファイルの kernel unit test と重複する内容だが、integration test 層でも固定する）を以下の方針で実装すること:

| 関数名 | 実装方針 |
|---|---|
| `t_deg_mirror_on_axis_line` | kernel `t_deg_mirror_on_axis_line` と同じ入力 (axis=x軸, line `(0,0)-(5,0)`) を `apply_sketch_mirror` に通し `DegenerateSketchElement{reason:"mirror_axis_coincident"}` を assert |
| `t_deg_mirror_on_axis_arc` | kernel `t_deg_mirror_on_axis_arc` と同じ入力 (`start=-π/4,end=π/4`, axis=x軸) で同エラーを assert |
| `t_boundary_arc_endpoints_on_axis` | kernel 版と同じ入力 (`start=0,end=π`) で **Err にならず** `start=0,end=-π` の新規 Arc が追加されることを assert |
| `t_deg_axis_degenerate_same_point` | `axis_p1==axis_p2` → `InvalidParameter{kind:"sketch_mirror_axis_degenerate"}` |
| `t_deg_axis_degenerate_nan` | axis 成分に `f64::NAN` → 同上エラー |
| `t_deg_axis_degenerate_inf` | axis 成分に `f64::INFINITY` → 同上エラー |
| `t_deg_unknown_element` | 存在しない selection id → `InvalidParameter{kind:"sketch_mirror_unknown_element_id"}` |
| `t_deg_unsupported_type_ellipse` | Ellipse を selection → `UnsupportedFeature{kind:"sketch_mirror_of_ellipse_or_conic"}` |
| `t_deg_unsupported_type_conic` | Conic を selection → 同上エラー |
| `t_deg_duplicate_id` | 派生 id が既存 profile と衝突 → `DegenerateSketchElement{reason:"mirror_duplicate_id"}` |
| `t_deg_sketch_ref_not_found` | **kernel 関数ではなく `build_bodies_from_features` 経由**（`Feature::SketchMirror{sketch:"missing_sketch",..}` を含む features を build）で `KernelError::SketchNotFound{sketch}` を assert。既存 `sketch_chamfer_acceptance.rs::t_deg_sketch_ref_not_found` と同型 |

## 実装差分から追加すべきテスト

- なし。実装は plan.md の設計どおりで、plan にない分岐は確認できなかった（`is_axis_coincident` の Line 判定に "順序入れ替わり" OR 節が実装されているが、これは plan §on-axis 自己一致判定に明記済みの想定内挙動）。

## エッジケース・退化入力

- 上記の不足テスト一覧が全て。追加のエッジケースは不要（Line/Circle/Arc の 3 型 × 正常・退化を plan で網羅済み）。

## 数値境界

- STEP 3.5 で修正した Arc 反射式（`new_end = new_start - sweep`）は kernel unit test で既に検証済み（T04, T_boundary_arc_endpoints_on_axis, T_DEG_mirror_on_axis_arc）。追加の数値境界テストは不要。

## 決定性

- T01・`t_determinism_100_runs`（100 回呼び出しの一致）で担保済み。追加不要。

## Codex R03 (YAML roundtrip) — 新規実装が必要

`crates/engawa-format/src/feature.rs` の `#[cfg(test)] mod tests` に `SketchMirror` の roundtrip テストを追加すること（既存 `SketchChamfer`/`SketchOffset` の roundtrip テストと同型、`test_feature_serialization` 等を参照）:

- `T_SERDE_roundtrip`: `selection: vec![]` かつ `suppressed: false` で `serde_yaml::to_string` → `type: sketch_mirror` タグを含み `selection`/`suppressed` フィールドが省略されること、roundtrip で `Feature::id()` が一致すること
- `T_SERDE_selection_present`: `selection: vec!["l1".to_string()]` で roundtrip すると `selection` フィールドが出力されること
- `T_SERDE_suppressed_true`: `suppressed: true` で roundtrip すると `suppressed: true` が出力されること
