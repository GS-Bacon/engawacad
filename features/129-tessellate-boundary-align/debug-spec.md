# debug-spec: Issue #129 — tessellate_face_uv_grid の n_u 修正

## 確定した根本原因

前回の GLM dispatch（glm_runs=1）で Fix A（u_min を circle edge t_range から取得）と Fix B（earcut 巻き方向）を適用したが、**t05_watertight_fuse は依然 FAIL（384 naked edges）**。

`debug_fuse_topo` テストで詳細なトポロジーを確認した結果：

### フェース構造（boolean(box(10³), cyl(r=2,h=15), Fuse) 結果）
- Face 6: Plane, 64 本の Line HE（fwd=true, edges 12-75）
- Face 8: Cylinder, 130 HE（seam(fwd) + 64 Circle arcs + seam(bwd) + 64 Line HE(fwd=false, edges 75→12)）
- Face 6 と Face 8 は 64 本の Line エッジを**共有**（Face 6 が forward、Face 8 が backward）

Face 8 の 64 Circle arcs の total span: 64 × 0.0982 ≈ 2π → `full_rev_count = 1` ✓（コードは通る）

### 実際のバグ

`tessellate_face_uv_grid`（`mod.rs:495`）:
```rust
let n_u = opts.angular_segments.max(3);  // ← デフォルト 32
```

UV グリッドは `(n_u+1) × (n_v+1)` = `33 × (n_v+1)` 個の頂点を生成。

しかし Face 8 に隣接する Face 6 の境界は 64 本の Line エッジ = **64 頂点**。

`collect_loop_points` で Line エッジは `sample_segment` が `vec![t_start 点]`（1 点のみ）を返すため、64 本の Line = **64 頂点**（u = k×2π/64, k=0..63）。

UV グリッドは `u = u_min + k×2π/32`（k=0..32）= **33 頂点**（0, 2π/32, ..., 2π）。

**64 ≠ 33 → 境界頂点位置が完全不一致 → naked edges 384 本**

### Fix A（前回 GLM が適用）は効果なし

Face 8 の最初の Circle arc が t=[0.0000, 0.0982]（edge 141, fwd=true）のため、Fix A が抽出する `u_min = 0.0`。ハードコードと同値なので変化なし。Fix A は削除しても構わないが、保持しても問題ない（フォールバック 0.0 と等価）。

## 修正方針

**唯一の修正箇所**: `tessellate_face_uv_grid`（`mod.rs:495`）の `n_u` 算出を変更する。

### BEFORE（現状）
```rust
let n_u = opts.angular_segments.max(3);
```

### AFTER（修正後）
```rust
// Use the number of circle arcs in the outer loop as n_u.
// This ensures the UV grid boundary rows have exactly the same number of
// sampling points as adjacent faces tessellated via collect_loop_points.
//
// For a boolean-result cylinder with 64 arcs: n_u=64 → grid has 65 columns
// (u=0, 2π/64, ..., 64×2π/64=2π). Adjacent face boundary: 64 Line edges
// → collect_loop_points returns 64 start-vertex points. After seam welding
// (column 0 == column 64), the boundaries align exactly.
//
// For a primitive cylinder (make_cylinder, 1 full-circle arc): circle_arc_count=1
// which is degenerate; fall back to opts.angular_segments.
let circle_arc_count: usize = outer_loop
    .half_edges
    .iter()
    .filter(|&&he_idx| {
        let he = &solid.half_edges[he_idx];
        matches!(solid.edges[he.edge].curve, Curve::Circle { .. })
    })
    .count();
let n_u = if circle_arc_count > 1 {
    circle_arc_count
} else {
    opts.angular_segments.max(3)
};
```

**注意**: `circle_arc_count > 1`（厳密 > 1）にすること。make_cylinder は全円弧を1本の Circle edge（自己隣接ループ）で表現するため circle_arc_count=1 になる場合があり、この場合は fallback で既存挙動を維持する。

make_cylinder の円筒側面の外ループ:
```
[he_seam_up, he_top_rev, he_seam_dn, he_bot_fwd]
```
Circle arcs: `he_top_rev`（top circle、1 HE）+ `he_bot_fwd`（bottom circle、1 HE）= **2 本**。
→ circle_arc_count=2, n_u=2 は明らかに小さすぎる。

実際に試したところ `make_cylinder` の外ループには seam line × 2 + top circle × 1 + bottom circle × 1 = 4 HE。Circle は 2 本。

**修正**: circle_arc_count を使う条件を `circle_arc_count > 2 × full_rev_count` とする：
- boolean 後の cylinder（各 rev ≥ 4 arcs）: circle_arc_count ≥ 4 per rev → n_u = circle_arc_count / full_rev_count を使う
- make_cylinder 後の cylinder（各 rev = 1 arc）: circle_arc_count = full_rev_count (= 1 or 2) → fallback

```rust
// Number of circle arcs per full revolution.
// For a boolean-result cylinder with 64 arcs per rev: arcs_per_rev = 64 → use as n_u.
// For make_cylinder (1 arc per rev): arcs_per_rev = 1 → fall back to opts.angular_segments.
let circle_arc_count: usize = outer_loop
    .half_edges
    .iter()
    .filter(|&&he_idx| {
        let he = &solid.half_edges[he_idx];
        matches!(solid.edges[he.edge].curve, Curve::Circle { .. })
    })
    .count();
let arcs_per_rev = circle_arc_count / full_rev_count as usize;  // integer division
let n_u = if arcs_per_rev > 1 {
    arcs_per_rev
} else {
    opts.angular_segments.max(3)
};
```

例:
- boolean(box, cyl, Fuse): circle_arc_count=64, full_rev_count=1, arcs_per_rev=64, n_u=64 ✓
- make_cylinder: circle_arc_count=2, full_rev_count=1 (lateral has 1 top + 1 bottom arc in separate revs... actually full_rev_count=1), arcs_per_rev=2 > 1 → n_u=2 → TOO SMALL!

実は make_cylinder の lateral face:
- outer loop = [he_seam_up, he_top_rev, he_seam_dn, he_bot_fwd]
- total_circle_span = span(he_top_rev) + span(he_bot_fwd) = 2π + 2π = 4π
- full_rev_count = round(4π / 2π) = 2
- circle_arc_count = 2 (top + bottom circle, each 1 arc)
- arcs_per_rev = 2 / 2 = 1 → n_u = opts.angular_segments.max(3) = 32 ✓

make_cylinder の場合はフォールバックになる。

- boolean(box, cyl, Fuse): face 8 の circle_arc_count=64, full_rev_count=1, arcs_per_rev=64, n_u=64 ✓

**確認: face 9 の full_rev_count**:
Face 9 outer loop (130 HE): HE 0 が seam、その後 64 Line HE + (reverse seam?) + 64 Circle arcs の構造と推定。
face 9 の total_circle_span も 64 × 0.0982 ≈ 2π → full_rev_count=1。
arcs_per_rev = 64 / 1 = 64 → n_u = 64 ✓

## 実装タスク

### 1. mod.rs の修正（GLM担当）

`crates/mycad-kernel/src/tessellation/mod.rs` の `tessellate_face_uv_grid` 関数内:

**行 ~495 付近**（`let n_u = opts.angular_segments.max(3);` を置換）:

```rust
let circle_arc_count: usize = outer_loop
    .half_edges
    .iter()
    .filter(|&&he_idx| {
        let he = &solid.half_edges[he_idx];
        matches!(solid.edges[he.edge].curve, Curve::Circle { .. })
    })
    .count();
// arcs_per_rev > 1: boolean result with many arc segments per circle → use as grid resolution.
// arcs_per_rev <= 1: primitive cylinder (1 arc per full circle) → use opts.angular_segments.
let arcs_per_rev = if full_rev_count > 0 {
    circle_arc_count / full_rev_count as usize
} else {
    0
};
let n_u = if arcs_per_rev > 1 {
    arcs_per_rev
} else {
    opts.angular_segments.max(3)
};
```

Fix A（u_min の取得）は保持して構わない（同じ結果を返すため無害）。
Fix B（earcut winding flip）も保持する。

### 2. debug_fuse_topo テスト削除（GLM またはここで対応可能）

`crates/mycad-kernel/tests/boundary_align_acceptance.rs` から `debug_fuse_topo` 関数（`panic!("debug output")` を含むテスト）を削除する。

### 3. t05_watertight_fuse の #[ignore] 解除確認

`tessellation_cap_acceptance.rs` の `t05_watertight_fuse` から `#[ignore]` が既に削除されていることを確認する（GLM run 1 で実施済み）。

## 試した修正と結果

- **Fix A（u_min 取得）**: 適用済みだが効果なし。Face 8 の最初の circle arc が t=[0.0, 0.0982] のため u_min=0.0（変化なし）。
- **Fix B（earcut winding flip）**: 適用済み。視覚バグ修正だが naked edge とは無関係。
- **Fix C（n_u = arcs_per_rev）**: 未適用。これが本当の修正。

## 次にやること

1. `mod.rs` の Fix C を適用する
2. `cargo test -p mycad-kernel t05_watertight_fuse` が GREEN になることを確認する
3. `cargo xtask ci` が green になることを確認する
4. `debug_fuse_topo` テストを削除する（panic!("debug output") が残っているため CI が FAIL するため）
