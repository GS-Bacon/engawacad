# test-spec: #295 Sketch Offset

## 実装済カバレッジ (plan.md T ID との対応)

| T ID | 場所 | 状態 |
|------|-----|------|
| T01 | `crates/engawa-kernel/src/geometry/sketch_offset.rs::tests::t01_offset_deterministic` + `crates/engawa-build/tests/sketch_offset_acceptance.rs::t01_determinism_offset` | ✅ |
| T01a | `crates/engawa-kernel/src/geometry/sketch_offset.rs::tests::t01a_selection_order_invariant` + `crates/engawa-build/tests/sketch_offset_acceptance.rs::t01a_selection_order_independence` | ✅ |
| T02 | `crates/engawa-kernel/src/geometry/sketch_offset.rs::tests::t02_line_offset` + `crates/engawa-build/tests/sketch_offset_acceptance.rs::t02_offset_line_positive_distance` | ✅ |
| T03 | `crates/engawa-kernel/src/geometry/sketch_offset.rs::tests::t03_circle_offset` + `crates/engawa-build/tests/sketch_offset_acceptance.rs::t03_offset_circle_positive_distance` | ✅ |
| T04 | `crates/engawa-kernel/src/geometry/sketch_offset.rs::tests::t04_arc_offset` + `crates/engawa-build/tests/sketch_offset_acceptance.rs::t04_offset_arc_negative_distance` | ✅ |
| T05 (roundtrip) | `crates/engawa-format/src/feature.rs::tests::t_sketch_offset_roundtrip` | ✅ |
| T06 (Extrude 統合) | `crates/engawa-build/tests/sketch_offset_acceptance.rs::t06_extrude_uses_offset_profile` | ✅ |
| T07 | `crates/engawa-build/tests/sketch_offset_acceptance.rs::t07_selection_partial` | ✅ |
| T08 | `crates/engawa-build/tests/sketch_offset_acceptance.rs::t08_selection_empty_means_all` | ✅ |
| T_DEG_zero_distance | `crates/engawa-kernel/src/geometry/sketch_offset.rs::tests::t_deg_zero_distance` + acceptance | ✅ |
| T_DEG_zero_distance_boundary | `crates/engawa-kernel/src/geometry/sketch_offset.rs::tests::t_deg_zero_distance_boundary` + acceptance | ✅ |
| T_DEG_offset_collapse_circle | `crates/engawa-kernel/src/geometry/sketch_offset.rs::tests::t_deg_offset_collapse_circle` | ✅ |
| T_DEG_offset_collapse_line | `crates/engawa-kernel/src/geometry/sketch_offset.rs::tests::t_deg_offset_collapse_line` | ✅ |
| T_DEG_ellipse_reject | `crates/engawa-kernel/src/geometry/sketch_offset.rs::tests::t_deg_ellipse_reject` + acceptance | ✅ |
| T_DEG_nan_distance | `crates/engawa-kernel/src/geometry/sketch_offset.rs::tests::t_deg_nan_distance` + acceptance | ✅ |
| T_DEG_sketch_ref_not_found | acceptance | ✅ |

## 実装差分から追加すべきテスト

- **t_deg_conic_reject** (kernel unit test): plan では T_DEG_ellipse_reject と統合していたが、`SketchElement::Conic` variant のカバーが独立にあった方が明示的。GLM が既に追加済 (`sketch_offset.rs::t_deg_conic_reject`) — 承認。
- **t_deg_inf_distance / t_deg_neg_inf_distance** (kernel unit test): NaN 以外の非有限値も plan の "NaN/Inf 拒否" 規定に含まれる。GLM が追加済 — 承認。
- **t_negative_line_offset** (kernel unit test): 負 distance の Line offset (符号の対称性検証)。GLM が追加済 — 承認。
- **t_diagonal_line_offset** (kernel unit test): 斜め Line の offset (perpendicular 計算検証)。GLM が追加済 — 承認。
- **t_determinism_100_runs** (kernel unit test): 100 回 loop で決定性確認。GLM が追加済 — 承認。

いずれも plan.md のスコープ内 (Line/Circle/Arc の offset + 退化 + 決定性) で、plan の T ID を細分化しただけ。plan 更新不要。

## 類似ケース (未カバー)

Sketch Offset は本 Issue 単独で完結し (Trim / Extend / Fillet は別 Issue)、他 feature からの類似バグ持ち込みは想定しない。

## 期待値乖離

なし。plan の期待値 (Line +1.0 offset → y+1, Circle radius +2 → +2, etc.) と実装 assertion が一致。`check-spec-divergence.ts` は git diff main..HEAD が空扱いで警告なし (ブランチ base 検出の限界、実際の内容は上表で照合済)。
