# Test Spec: #289 (STEP 6.6 GLM-test-implementer 向け)

## 背景

STEP 6 (GLM core 実装) で `tessellation/sketch.rs` の inline test に `T_BOUNDARY_above_tolerance_*` × 4 と `T_BOUNDARY_exact_tolerance_*` × 4 + 反転済み `T_EDGE_length_tolerance_boundary_passes` が追加された。

acceptance test `crates/engawa-kernel/tests/length_tolerance_boundary_acceptance.rs` には STEP 5.5 で配置した `#[ignore = "STEP 6 で実装後に解除"]` 付きの `todo!()` スケルトンが 10 個残っている。これを STEP 6.6 で本実装し、`#[ignore]` を外す。

## 不足テスト (plan 計画分)

acceptance file の 10 個のスケルトン (1 plus boundary 8 plus deg 1) を以下の通り実装する。

### 前提: pub use の追加

`crates/engawa-kernel/src/tessellation/mod.rs` の先頭の `pub use` セクションに以下を追加する:

```rust
pub use sketch::tessellate_sketch_element;
```

これがないと acceptance file から `engawa_kernel::tessellation::tessellate_sketch_element` を呼べない。

### t01_determinism

`tessellate_sketch_element` を同一 SketchElement::Circle で 2 回呼び、得られた `Vec<[f64; 2]>` が完全一致することを確認する。

```rust
let circle = SketchElement::Circle { id: "c0".into(), center: [0.0, 0.0], radius: 1.0 };
let a = tessellate_sketch_element(&circle, 32).unwrap();
let b = tessellate_sketch_element(&circle, 32).unwrap();
assert_eq!(a, b);
```

### t_boundary_above_tolerance_circle / arc / ellipse_major / ellipse_minor

`radius / major / minor = LENGTH_TOLERANCE * 1.000_001` で **pass** すること。inline test (sketch.rs 内) と同じ入力を使い、acceptance file からは `tessellate_sketch_element(&elem, 32).unwrap()` で結果を取り出して点数 (Circle/Ellipse は 32、Arc は 8) を確認する。

### t_boundary_exact_tolerance_circle / arc / ellipse_major / ellipse_minor

`radius / major / minor = LENGTH_TOLERANCE` で **reject** されること。`Err(KernelError::DegenerateSketchElement { .. })` を match で確認する。

### t_deg_below_tolerance

`radius = LENGTH_TOLERANCE / 2.0` で reject されること (既存 inline test `t_edge_tiny_radius_circle_fails` の acceptance 版)。

## 実装差分から追加すべきテスト

なし (inline test と acceptance test で同一観点を 2 系統からカバーする、これ自体が偽陽性ガード)。

## 類似ケース (未カバー)

本 Issue は `LENGTH_TOLERANCE` 境界判定の統一のみが scope。`ANGLE_TOLERANCE` / `EPS_DISCRIMINANT` / `EPS_AXIS_RATIO` の境界判定の統一は別 Issue で扱う (Non-Goals に明記済み)。

## エッジケース・退化入力

inline test 側で既に網羅 (`T_EDGE_negative_zero_radius` / `T_EDGE_negative_radius` / `T_EDGE_min_positive_radius` 等)。acceptance 側では `t_deg_below_tolerance` のみ追加。

## 数値境界

inline test と acceptance test で同じ境界値 (`LENGTH_TOLERANCE`, `LENGTH_TOLERANCE * 1.000_001`) を使う。

## 決定性

`t01_determinism` で `tessellate_sketch_element` の決定性を独立 acceptance file からも確認する。
