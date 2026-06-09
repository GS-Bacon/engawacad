## In-Scope / Out-of-Scope

| In-Scope | Out-of-Scope |
|----------|--------------|
| `topology.rs` `validate_manifold()` のコプラナー面チェックを改善 | tessellation・boolean アルゴリズムの変更 |
| 隣接面（shared vertex）を誤検出しないようにする | classify.rs の変更 |
| inner loop を考慮した overlap 判定 | SAT (Separating Axis Theorem) の完全実装 |

## Non-Goals

- surface cut (#120) の修正
- AABB 関数の削除

## 問題

`validate_manifold()` L398-405 の coplanar duplicate face チェックが `aabb_overlap()` のみで重複を判定している。
実ポリゴン同士が非重複でも投影 AABB が重なる正当な face 組を `ManifoldViolation` と誤判定する可能性がある。
また `inner_loops` を考慮していないため穴付き face でも偽陽性になりうる。

## 修正方針

`crates/mycad-kernel/src/brep/topology.rs` の L398-405:

**Before:**
```rust
// 2D AABB overlap check
let verts_a = self.loop_vertex_points(face_a.outer_loop);
let verts_b = self.loop_vertex_points(face_b.outer_loop);
if aabb_overlap(&verts_a, &verts_b, normal_a, len_eps) {
    return Err(KernelError::ManifoldViolation {
        reason: "overlapping coplanar faces detected",
    });
}
```

**After:**
```rust
// Skip face pairs that share at least one vertex (adjacent faces are not duplicates)
let verts_a = self.loop_vertex_points(face_a.outer_loop);
let verts_b = self.loop_vertex_points(face_b.outer_loop);
let share_vertex = verts_a.iter().any(|pa| {
    verts_b.iter().any(|pb| (pa - pb).norm() < len_eps)
});
if share_vertex {
    continue;
}
// 2D AABB overlap as a coarse filter, then check inner loops to reduce false positives
if aabb_overlap(&verts_a, &verts_b, normal_a, len_eps) {
    // Also verify no inner loop covers the overlap region
    let inner_loops_a: Vec<Vec<Point>> = face_a.inner_loops
        .iter()
        .map(|&li| self.loop_vertex_points(li))
        .collect();
    // If the overlap is fully inside any inner loop of face_a, it is a hole → not a dup
    let covered_by_hole = inner_loops_a.iter().any(|hole| {
        aabb_overlap(&verts_b, hole, normal_a, len_eps)
    });
    if !covered_by_hole {
        return Err(KernelError::ManifoldViolation {
            reason: "overlapping coplanar faces detected",
        });
    }
}
```

**注意**: `Face` 構造体に `inner_loops` フィールドがない場合は `Vec::new()` として扱い、動作は変わらない（フィールド名は実コードを grep して確認すること）。

## テスト計画（ID 付き）

| ID | 種別 | 内容 | 期待結果 |
|----|------|------|----------|
| T_adj_coplanar | 正常系 | 隣接コプラナー面を持つ solid が validate_manifold() を通る | Ok(()) |
| T_degen_true_overlap | 退化 | 真の重複面を持つ solid が ManifoldViolation を返す | Err |
