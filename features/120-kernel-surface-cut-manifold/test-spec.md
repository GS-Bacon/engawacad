# Test Spec — Issue #120: kernel-surface-cut-manifold

## 不足テスト（plan 計画分）

計画全 T ID は acceptance.rs と boolean_proptest.rs で実装済み:

| ID | テスト関数 | 場所 | 実装状況 |
|----|-----------|------|---------|
| T01_determinism | `t01_determinism_surface_cut` | `kernel_surface_cut_manifold_acceptance.rs` | ✓ 実装済み |
| T02_surface_cut_manifold | `t02_prop_surface_cut_reduces_volume` | `boolean_proptest.rs`（#[ignore] 解除済み）| ✓ 実装済み |
| T02_volume_reduced | `t02_volume_reduced_after_surface_cut` | `kernel_surface_cut_manifold_acceptance.rs` | ✓ 実装済み |
| T02_boundary_large_offset | `t02_boundary_large_offset_manifold` | `kernel_surface_cut_manifold_acceptance.rs` | ✓ 実装済み |
| T02_degen_flush | `t02_degen_flush_no_panic` | `kernel_surface_cut_manifold_acceptance.rs` | ✓ 実装済み |

plan の全テスト ID は実装済み。追加すべき不足テストなし。

## 実装差分から追加すべきテスト

実装差分で判明した2つの変更パス:

### 1. ring/disc branch（chain_segments_into_polygon）
- x_offset∈[4.1,5.4] の proptest でカバー済み（T02_surface_cut_manifold）
- **追加すべき**: ring fragment の `inner_polygons_3d` が 1 件存在することの unit test（partitionレベル）

### 2. pslg_subdivide の外側フェースフィルタ修正（相対誤差化）
- `dbg_surface_cut_partition_trace` に `tool face[1]` サブフェース数=2 のアサートを GLM が追加済み（partition.rs テスト内）
- この回帰テストは `#[ignore]` 付きではなく CI に含まれるべき

## エッジケース・退化入力

以下を GLM テスト実装に追加する:

### E01: proptest の失敗値（x_offset=4.97809298669049）の固定値テスト
- 回帰テストとして x_offset を固定値でテスト
- proptest は min-case を永続化しないため、固定値テストで防衛する

### E02: chain_segments_into_polygon が None を返す場合のフォールバック
- セグメントが閉ループを形成しない場合（端点が一致しない）→ pslg_subdivide へフォールバック
- boolean() 全体が Err にならないこと（クラッシュしないこと）

### E03: validate_manifold の直接呼び出し
- T02_volume_reduced はすでに `cut.validate_manifold().is_ok()` をアサートしている ✓
- proptest T02 は validate_manifold を通じた Ok チェックを含む（boolean() 内部） ✓

## 数値境界

| 境界条件 | x_offset 値 | カバー状況 |
|---------|------------|----------|
| tool 右面が target 右面にほぼ flush | 4.0 | T02_degen_flush ✓ |
| surface cut 典型値 | 4.5 | T02_volume_reduced ✓ |
| 浮動小数点バグが顕在化した値 | 4.97809298669049 | proptest min-case（固定値テスト追加推奨） |
| tool が大きく貫通 | 5.3 | T02_boundary_large_offset ✓ |

## 決定性

T01_determinism で IdGenerator(42) seed を使い 2 回実行して vertex数・edge数・face数と mesh が一致することを確認済み。

## GLM テスト実装指示

以下の2テストを `crates/mycad-kernel/tests/kernel_surface_cut_manifold_acceptance.rs` に追加すること:

```rust
/// T02_regression_fixed_offset: proptest が縮小した失敗値 x_offset=4.97809298669049 の固定値回帰テスト
#[test]
fn t02_regression_fixed_x_offset() {
    let mut gen = IdGenerator::new(42);
    let target = make_cuboid(10.0, 20.0, 30.0, &mut gen).unwrap();
    let mut tool = make_cuboid(2.0, 2.0, 2.0, &mut gen).unwrap();
    tool.translate(Vec3::new(4.97809298669049, 0.0, 0.0));
    let result = boolean(&target, &tool, BooleanOp::Cut, &mut gen);
    assert!(result.is_ok(), "fixed-offset surface cut must succeed: {result:?}");
    let cut = result.unwrap();
    assert!(
        cut.validate_manifold().is_ok(),
        "fixed-offset surface cut manifold invalid: {:?}",
        cut.validate_manifold()
    );
}

/// T03_determinism_fixed_seed: seed=100 でも決定的であることを確認（seed=42 以外）
#[test]
fn t03_determinism_fixed_seed() {
    let build = |seed: u64| {
        let mut gen = IdGenerator::new(seed);
        let target = make_cuboid(10.0, 20.0, 30.0, &mut gen).unwrap();
        let mut tool = make_cuboid(2.0, 2.0, 2.0, &mut gen).unwrap();
        tool.translate(Vec3::new(4.97809298669049, 0.0, 0.0));
        boolean(&target, &tool, BooleanOp::Cut, &mut gen).unwrap()
    };
    let a = build(100);
    let b = build(100);
    assert_eq!(a.vertices.len(), b.vertices.len());
    assert_eq!(a.edges.len(), b.edges.len());
    assert_eq!(a.faces.len(), b.faces.len());
}
```
