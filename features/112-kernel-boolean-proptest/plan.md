## In-Scope / Out-of-Scope
| In-Scope | Out-of-Scope |
|----------|--------------|
| proptest を workspace deps に追加 | proptest の mycad-build への追加 |
| mycad-kernel/tests/boolean_proptest.rs: Boolean Cut の property test | GUI/API layer のプロパティテスト |
| validate_manifold() が常に Ok または明確な Err を返す検証 | 既存 example-based テストの削除・変更 |
| 頂点数変化 + signed_volume 減少の不変条件検証 | fuse/intersect のプロパティテスト |

## Non-Goals
- フォーマット変換 (proptest と serde の組み合わせ)
- GUI/Playwright テスト
- proptest の advanced shrinkage 戦略（デフォルト shrink で十分）
- fuse/intersect の property test（今回は Cut のみ）

## 実装対象
Issue: #112
影響ファイル:
- `Cargo.toml` (root workspace) — `proptest = "1"` を `[workspace.dependencies]` に追加
- `crates/mycad-kernel/Cargo.toml` — `[dev-dependencies]` に `proptest = { workspace = true }` を追加
- `crates/mycad-kernel/tests/boolean_proptest.rs` (新規) — property test 本体

## 設計方針

### proptest ストラテジー
ターゲット: `make_cuboid(10.0, 20.0, 30.0)` — 原点中心, X∈[-5,5]

```rust
use proptest::prelude::*;

proptest! {
    #![proptest_config(ProptestConfig::with_cases(32))]
    #[test]
    fn prop_cut_never_produces_degenerate_manifold(
        x_offset in -3.0_f64..3.0_f64,
        tool_w in 1.0_f64..4.0_f64,
        tool_h in 1.0_f64..8.0_f64,
        tool_d in 1.0_f64..10.0_f64,
    ) {
        let mut gen = IdGenerator::new(99);
        let target = make_cuboid(10.0, 20.0, 30.0, &mut gen).unwrap();
        let mut tool = make_cuboid(tool_w, tool_h, tool_d, &mut gen).unwrap();
        tool.translate(Vec3::new(x_offset, 0.0, 0.0));
        let result = boolean(&target, &tool, BooleanOp::Cut, &mut gen);
        match result {
            Ok(solid) => prop_assert!(
                solid.validate_manifold().is_ok(),
                "degenerate B-rep: {:?}", solid.validate_manifold()
            ),
            Err(_) => {} // clean rejection is acceptable
        }
    }
}
```

### 内部 cut 体積減少テスト
```rust
proptest! {
    #![proptest_config(ProptestConfig::with_cases(32))]
    #[test]
    fn prop_internal_cut_reduces_volume(
        x_offset in -2.0_f64..2.0_f64,
    ) {
        let mut gen = IdGenerator::new(42);
        let target = make_cuboid(10.0, 20.0, 30.0, &mut gen).unwrap();
        let mut tool = make_cuboid(2.0, 2.0, 2.0, &mut gen).unwrap();
        tool.translate(Vec3::new(x_offset, 0.0, 0.0));
        let result = boolean(&target, &tool, BooleanOp::Cut, &mut gen);
        if let Ok(cut) = result {
            let mesh_t = tessellate_solid(&target).unwrap();
            let mesh_c = tessellate_solid(&cut).unwrap();
            let vol_t = mesh_signed_volume(&mesh_t).abs();
            let vol_c = mesh_signed_volume(&mesh_c).abs();
            prop_assert!(
                vol_c < vol_t,
                "volume must decrease: before={vol_t}, after={vol_c}"
            );
        }
    }
}

fn mesh_signed_volume(mesh: &TriangleMesh) -> f64 {
    mesh.indices.chunks(3).map(|tri| {
        let p0 = mesh.positions[tri[0] as usize];
        let p1 = mesh.positions[tri[1] as usize];
        let p2 = mesh.positions[tri[2] as usize];
        (p0[0]*(p1[1]*p2[2]-p1[2]*p2[1])
        +p0[1]*(p1[2]*p2[0]-p1[0]*p2[2])
        +p0[2]*(p1[0]*p2[1]-p1[1]*p2[0])) / 6.0
    }).sum()
}
```

### 決定性テスト（example-based）
proptest とは別に、固定 seed で同じ結果になることを検証。

## テスト計画（ID 付き）
| ID | 種別 | 内容 | 期待結果 |
|----|------|------|----------|
| T01 | property | ランダム cut → validate_manifold() は Ok or clear Err | prop_assert! pass |
| T01_boundary | 境界 | tool が coplanar 位置（x_offset → target face と接触）→ Ok or Err (not panic) | prop_assert! pass |
| T02 | property | 内部完全埋没 cut で体積減少 | vol_after < vol_before |
| T02_degen | 退化 | tool w/h/d = 1.0 の最小ケース → Ok or Err | prop_assert! pass |
| T03 | 決定性 | IdGenerator(99) で固定入力 → 2 回同一結果 | assert_eq! |

## 幾何的不変条件チェックリスト
- [x] validate_manifold() が代替 Euler-Poincaré チェック: 内部で verify
- [x] signed_volume の符号: abs() で対処（同一点列 winding 依存）
- N/A: flip_normals/same_sense（Boolean は既存ロジックで処理済み）
