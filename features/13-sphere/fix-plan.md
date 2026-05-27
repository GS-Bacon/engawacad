# Codex 最終レビュー指摘修正プラン (F01/F02/F03)

## F01 (High) — tessellation endpoint tolerance: absolute→relative
**File**: `crates/mycad-kernel/src/tessellation/mod.rs`
**Problem**: `tessellate_face_sphere` の canonical 判定でエンドポイント照合に `eps = 1e-9` (絶対値) を使用。
`curve.evaluate(π)` の x 成分は `r * sin(π) ≈ r * 1.2e-16` となるため、r=1e10 では差が ~1.2e-6 になり、
正当な大半径球が `TrimmedFaceUnsupported` に誤判定される。

**Fix**:
- `is_canonical_sphere_face` または `tessellate_face_sphere` の冒頭で `let geom_eps = sph_radius * 1e-9_f64;` を算出
- エンドポイント照合 (line ~384):
  ```rust
  if (p_start - solid.vertices[edge.vertices[0]].point).norm() > geom_eps
      || (p_end - solid.vertices[edge.vertices[1]].point).norm() > geom_eps
  ```
  の `eps` を `geom_eps` に変更する
- seam normal チェック (`(*seam_normal + Vec3::y()).norm() > eps`) と
  span チェック (`(span - PI).abs() > eps`) は絶対誤差 1e-9 のままでよい（単位ベクトル・ラジアン）
- **テスト追加**: `r = 1e6` の球で `tessellate_solid` が成功することを検証する回帰テスト

## F02 (High) — validate_manifold: isolated edge は 0 HE で見逃す
**File**: `crates/mycad-kernel/src/brep/topology.rs`
**Problem**: `validate_manifold` で `edge_he_count` を half_edges から構築後、
`edge_he_count` の entry しかループしない。HalfEdge から参照されない孤立 Edge (0 HE) は
entry が存在せず、`Ok(())` で通過してしまう。

**Fix**: 既存の `for (edge_idx, forwards) in &edge_he_count { ... }` ブロックを削除し、
以下で置き換える:
```rust
for edge_idx in 0..self.edges.len() {
    match edge_he_count.get(&edge_idx) {
        None => return Err("edge has no half-edges"),
        Some(forwards) => {
            if forwards.len() != 2 {
                return Err("edge must have exactly 2 half-edges");
            }
            if forwards[0] == forwards[1] {
                return Err("edge half-edges must have opposite orientation");
            }
        }
    }
}
```
**テスト追加**: 孤立エッジ (HalfEdge が 1 本または 0 本しかない edge) を含む手動構築 Solid が
`validate_manifold().is_err()` になることを確認する負例テスト。

## F03 (Medium) — validate_manifold loop closure: 座標比較 → インデックス比較
**File**: `crates/mycad-kernel/src/brep/topology.rs`
**Problem**: ループ閉性の判定で `vertices[end_v].point` と `vertices[he_next.start_vertex].point` の
座標ノルム差 (`< eps`) で比較している。coincident vertices や浮動小数点誤差で偽陽性になり得る。

**Fix**: 座標比較を頂点インデックス等値比較に変更:
```rust
// 現在:
if (self.vertices[end_v].point - self.vertices[he_next.start_vertex].point).norm() > eps {
    return Err("loop is not closed");
}
// 変更後:
if end_v != he_next.start_vertex {
    return Err("loop is not closed");
}
```
**テスト追加**: 頂点座標が一致するが別インデックスの頂点でループが「見かけ上」閉じているが
実際には断裂している Solid が `validate_manifold().is_err()` になることを確認する負例テスト
（任意; medium のため未実装でも可だが追加推奨）。

## CI 確認
修正後: `cargo xtask ci` が green になること。
特に以下を確認:
- `cargo test -p mycad-kernel --lib -- sphere` 25 テスト all pass
- `cargo test -p mycad-kernel --lib -- validate` 全 pass
- 新規追加: large radius + 孤立エッジ負例テスト
