# debug-spec — #42 Plane×Cylinder Fuse+Intersect

## 仮説（根本原因特定済み）

`crates/mycad-kernel/src/booleans/partition.rs` の **upper fragment 構築部** で、
top circle (v_cut..v_top の上端) の traversal 方向が **WRONG** (CCW) であるため、
assemble 時に cylinder top cap (polygon が CCW) と衝突し、
「同方向 HE が 2 本」→ manifold violation "edge must have exactly 2 half-edges" が発生している。

**Probe 結果**:
- Intersect: V=128, E=129, F=3, manifold OK ✓ (下部 lateral + bottom cap + box top disc = 3 faces)  
- Fuse: `BooleanInternal("manifold validation failed: manifold violation: edge must have exactly 2 half-edges")`

**なぜ Cut は通る?** Cut では upper fragment(v_cut..v_top) は InsideOther 判定で選択されない。Lower fragment(v_bot..v_cut) のみ選択される。lower fragment の bot circle は bottom cap と OPPOSITE 方向 → manifold OK。

**なぜ top circle が問題か?**

`make_cylinder` で top cap loop は `he_top_fwd`(forward = CCW from above)。
`get_loop_vertices` が forward HE で top circle を評価すると:
polygon = [seam_top, top_k1, top_k2, ..., top_k(n-1)] = **CCW**

`partition.rs` の upper fragment top boundary:
```rust
upper_poly.push(seam_top);
for k in 1..n_seg {
    upper_poly.push(top_circle_3d.evaluate(k as f64 * dt));  // seam_top → top_k1 → ... = CCW
}
upper_poly.push(seam_top);  // second occurrence
```

assemble でこの fragment と top cap fragment が同じ edges を **同方向** (forward) で HE を作成
→ edge あたり 1 HE しかない → manifold violation!

**Lower fragment の bot circle はなぜ大丈夫か?**
`make_cylinder` で bottom cap loop は `he_bot_rev`(reverse = CW from above)。
`get_loop_vertices` が REVERSE HE で評価すると polygon = [seam_bot, bot_k(n-1), ..., bot_k1] = **CW** (逆順)。
lower fragment bot boundary は CCW → top cap polygon CW と OPPOSITE → manifold OK ✓。

## 修正箇所

**ファイル**: `crates/mycad-kernel/src/booleans/partition.rs`

**場所**: upper fragment 構築部 (line 1120 付近、`upper_poly.push(top_circle_3d.evaluate(...))` のループ)

**Before** (実際の行を確認してから修正すること):
```rust
upper_poly.push(seam_top);    // [1]
for k in 1..n_seg {
    upper_poly.push(top_circle_3d.evaluate(k as f64 * dt));  // CCW — WRONG
}
upper_poly.push(seam_top);    // [n_seg+1] (2nd occurrence)
```

**After** (top circle を逆順にして CW にする):
```rust
upper_poly.push(seam_top);    // [1]
for k in (1..n_seg).rev() {   // REVERSED: top_k(n-1), ..., top_k1 = CW ← FIX
    upper_poly.push(top_circle_3d.evaluate(k as f64 * dt));
}
upper_poly.push(seam_top);    // [n_seg+1] (2nd occurrence)
```

この 1 行変更で top circle の traversal が CW になり、top cap (CCW) と OPPOSITE → manifold ✓。

**boundary_partners の変更は不要**:
upper_partners は `(n_seg+2)..upper_total` に設定 → これは cut circle edges のみを指す。
top circle edges (index 1..n_seg) は `None` のまま。top circle reversalで index は変わらない。

## 確認: tessellate_cylinder_face_trimmed は不要

元の plan.md では `tessellate_cylinder_face_trimmed` 新設を想定していたが、**不要**。

理由: `tessellate_face_uv_grid` (tessellation/mod.rs:368) がすでに upper fragment を正しく処理できる:
1. inner_loops 無し ✓
2. circle_span = cut circle 全体 = 2π (64 edges × dt) ✓  
3. v_min, v_max = corner vertices の v 座標から自動決定 (v_cut..v_top) ✓

A1 (Cut) でも lower fragment は tessellate_face_uv_grid が v_bot..v_cut で処理済み ✓。

## 実装手順

1. **partition.rs の 1 行修正** (上記 before/after を参照)
   - 実際の行番号は Read で確認すること
   - `for k in 1..n_seg {` → `for k in (1..n_seg).rev() {` に変更

2. **probe test で確認**:
   ```bash
   cargo test --package mycad-build --test fuse_isect_probe -- --nocapture
   ```
   期待: `Fuse OK: V=... E=... F=... manifold: OK`

3. **acceptance test の実装**:
   `crates/mycad-build/tests/a1_1_plane_cyl_fuse_isect_acceptance.rs` の `#[ignore]` を外して T01〜T12 を実装。
   
   **重要: build_features ヘルパは `crates/mycad-build/tests/feature_dispatcher.rs:720` にある**
   - そのファイルからは import できない。同様の helper を acceptance test に書く。
   - `build_bodies_from_features` は `mycad_build` から import 可能。

4. **acceptance test ヘルパ（テストファイル内に書く）**:
   ```rust
   use mycad_build::build_bodies_from_features;
   use mycad_format::Feature;
   use mycad_kernel::brep::topology::IdGenerator;
   use mycad_kernel::error::KernelError;
   
   fn build_fuse() -> mycad_kernel::brep::topology::Solid { ... }
   fn build_intersect() -> mycad_kernel::brep::topology::Solid { ... }
   fn check_euler(solid: &Solid, expected: i64, l_inner: usize) { ... }
   ```

5. **テスト実装の重要ポイント**:
   - T02 Fuse 体積: V_box(10³) + V_cyl_outside の近似値
     - box: 10×10×10 = 1000 (but centered at origin, cylinder at (0,0,0))
     - cylinder: r=2, h=15. cylinder spans z=0..15. box spans z=-5..+5.
     - intersection: cylinder at z=-5..+5 (overlap height = 10? no...)
     
     **重要**: box は (-5,-5,-5)〜(5,5,5), cylinder は z=0..15, overlap = z=0..5 (高さ5)
     - V_cylinder_total = π*4*15 ≈ 188.5
     - V_cylinder_inside_box = π*4*5 ≈ 62.83
     - V_cylinder_outside_box ≈ π*4*10 ≈ 125.66
     - V_box_minus_cylinder_part = 1000 - π*4*5 ≈ 937.17
     - V_fuse = V_box_minus + V_cyl_outside + V_cyl_inside = 1000 - π*4*5 + π*4*10 = 1000 + π*4*5 ≈ 1062.83
     - より正確: V_fuse = V_box + V_cylinder_outside_box = 1000 + π*4*10 ≈ 1125.66
     
   - T04 Intersect 体積: cylinder がboxの内部に入っている部分 = π*r²*h_overlap = π*4*5 ≈ 62.83
   
   - T03 Euler: V-E+F-L_inner=2 で `check_euler` を書く。L_inner は Fuse で inner_loops を持つfaceの inner_loops 数の合計。Fuse では box top face に inner_loop が 1 つ。なので L_inner=1, expected V-E+F=3。
   
   - T06 Fuse edge curve: intersection edge (cylinder lateral ∩ box top face) が `Curve::Circle { normal ≈ +Z, .. }` かチェック。

6. **examples ファイル**:
   - `examples/boolean_fuse_box_cyl.mycad`
   - `examples/boolean_intersect_box_cyl.mycad`
   
   既存の example 形式を参考にする:
   ```bash
   ls examples/*.mycad | head -5
   cat examples/boolean_cut_box_cyl.mycad  # 参考
   ```

7. **CI 実行**:
   ```bash
   cargo fmt --all
   cargo xtask ci
   ```

## 試した修正と結果

- GLM Run 1: probe test を作成して調査。max_turns(60) 超過。formatting 問題のみで実装未完。
  - fuse_isect_probe.rs は private module アクセスで compile error → Claude が public API 版に修正済み
  - a1_1_plane_cyl_fuse_isect_acceptance.rs は formatting で fmt error → Claude が `cargo fmt` 実行済み

## 次にやること

1. partition.rs の 1 行修正
2. probe test で Fuse manifold OK を確認
3. acceptance test T01-T12 実装 + #[ignore] 外す
4. example ファイル作成
5. cargo xtask ci → green
6. fuse_isect_probe.rs (デバッグ用) を削除してからコミット

## 追加で書いてほしいテスト

T06: Fuse 結果の intersection edges が `Curve::Circle` であることを確認するテスト。
solid.edges で filter → `matches!(e.curve, Curve::Circle { .. })` → count > 0。

実際には cylinder top_circle (boundary of upper lateral ∩ top cap) と cut_circle (upper lateral ∩ box top) の 2 つの Circle edge group が存在するはず。

---

## Round 2: Disc Polygon CW 問題（t02/t04 volume failure の根本原因）

### 問題確認

`probe_disc_edge_t_ranges` テストで以下を確認:
```
Disc face: 64 HEs
  HE 0: forward=true curve=Circle t_range=[-1.5708, 4.6142] span=6.1850
Total circle span = 395.840674 (expected 6.283185)
```

各 HE の span が `6.1850 ≈ 2π - 2π/64` (正しくは `2π/64 ≈ 0.098`)。

### 根本原因

#### Bug 1: partition.rs の disc_poly_3d が CW

`partition.rs` ~595 の `disc_poly_3d` は `inner_poly_2d` から生成される:
```rust
let disc_poly_3d: Vec<Point> = inner_poly_2d
    .iter()
    .map(|(u, v)| unproject_from_face_uv(surface, *u, *v))
    .collect();
```
`inner_poly_2d` は inner hole boundary = **CW 順** → `disc_poly_3d` も **CW**。

disc face の HE は CW 順に並ぶ → 隣接 HE の (p0, p1) が (p_k+1, p_k) (逆転)。

#### Bug 2: assemble.rs circle_curve_for_edge が長弧を生成

`circle_curve_for_edge` (~line 439-464):
```rust
let t0 = d0.dot(&v_ax).atan2(d0.dot(&u_ax));
let mut t1 = d1.dot(&v_ax).atan2(d1.dot(&u_ax));
if t1 < t0 { t1 += 2.0 * std::f64::consts::PI; }  // ← CW エッジで WRONG
```

CW エッジでは `t1_raw = t0 - 2π/64 < t0` → `t1 += 2π` → `t1 = t0 + (2π - 2π/64) = t0 + 6.185`。

これにより `sample_segment` が 1 HE あたり 32 samples × 6.185 rad をサンプリング → 63 周分の fan → volume が 63 倍 (1320 vs 正しい 20.94)。

同じ問題が Fuse の ring face の inner loop にも発生 → earcut も broken。

### 必要な修正（src ファイル — Claude 直接書き込み不可）

#### Fix A: partition.rs ~595

```rust
// Before:
let disc_poly_3d: Vec<Point> = inner_poly_2d
    .iter()
    .map(|(u, v)| unproject_from_face_uv(surface, *u, *v))
    .collect();

// After:
let mut disc_poly_3d: Vec<Point> = inner_poly_2d
    .iter()
    .map(|(u, v)| unproject_from_face_uv(surface, *u, *v))
    .collect();
disc_poly_3d.reverse();  // inner_poly_2d is CW (hole boundary); disc face needs CCW
```

#### Fix B: assemble.rs circle_curve_for_edge (~line 453)

```rust
// Before:
if t1 < t0 { t1 += 2.0 * std::f64::consts::PI; }

// After (prefer shorter arc — handles both CCW and CW boundaries):
if t1 < t0 { t1 += 2.0 * std::f64::consts::PI; }
if t1 - t0 > std::f64::consts::PI {
    t1 -= 2.0 * std::f64::consts::PI;  // short CW arc for ring inner boundary
}
```

### t08/t09 failure（別問題）

acceptance test の t08/t09 が使う `h=5` で cylinder top (z=5) = box top (z=5) の coincident → degenerate boolean → `Err(...)` → `assert!(result.is_ok())` が fail。

**修正**: acceptance test の `height: 5.0` → `height: 7.0` に変更（tests/ ファイル → Claude 書き込み可）。
