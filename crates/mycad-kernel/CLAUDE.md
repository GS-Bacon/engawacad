# mycad-kernel — B-rep 幾何カーネル

## Module Dependency Graph

```
primitives → brep, geometry
tessellation → brep, geometry
brep → geometry
geometry → (nalgebra)
```

## Key Invariants

- すべてのEdgeは正確に2つのHalfEdgeを持つ
- 閉じたShellはwatertight（水密）
- HalfEdge/Edgeのvertexインデックスは親Solidのvertex配列を参照
- Loopは閉じたHalfEdge列（最後のHalfEdgeの終点 = 最初のHalfEdgeの始点）
- EntityIDは `IdGenerator` で決定的に生成される

## How to Add a New Primitive

1. `src/primitives/` に新ファイルを作成（`cuboid.rs` をテンプレートとして使用）
2. 関数シグネチャ: `pub fn make_xxx(params..., id_gen: &mut IdGenerator) -> Solid`
3. `primitives/mod.rs` に `mod` と `pub use` を追加
4. 決定性テストを必ず書く（同じ `IdGenerator` 初期値で2回生成し、結果が同一であることを確認）

## How to Add a New Surface Type

1. `geometry/surface.rs` の `Surface` enumに新しいバリアントを追加
2. `evaluate(u, v) -> Point` を実装
3. `normal_at(u, v) -> Vec3` を実装
4. 共有数学関数は `geometry/math.rs` に配置

## How to Add a New Curve Type

1. `geometry/curve.rs` の `Curve` enumに新しいバリアントを追加
2. `evaluate(t) -> Point` を実装
3. 共有数学関数は `geometry/math.rs` に配置
