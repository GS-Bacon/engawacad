## Non-Goals

- **Plane×Sphere の Fuse / Intersect** — A2 は Cut のみ。Fuse/Intersect は次 Issue (A2.1 相当)。Cut の partition 経路は op と独立なので Fuse/Intersect で再利用しやすい構造を残すが、テスト・例は Cut のみ。
- **Cylinder×Sphere** — `partition.rs:288-295` の skip は触らない (A4 で対応)。
- **大円ケース (d=0)** — plane が sphere 中心を通る配置は球を 2 半球に分けるため、結果が複数 Solid になる可能性。A2 では `KernelError::UnsupportedBooleanCase` で reject。
- **接触 (tangent, |d|=R) / disjoint (|d|>R)** — `intersect_plane_sphere` が `Ok(vec![])` を返す既存挙動を維持し、Boolean は no-op として通す。A1 と同じ扱い。
- **複数 plane で球を切るケース** — sphere が box の 2 面以上を貫通するケースは `KernelError::UnsupportedBooleanCase`。
- **非 Z-aligned plane** — `intersect_plane_sphere` の MVP 制約 (normal は ±Z) のまま。XY 平面以外は `UnsupportedSurfaceIntersection`。
- **`surface_intersect.rs` の pcurve_on_sph 汎化** — pcurve_on_sph (Line2D 緯度線) は格納するが実装では 3D circle 直接利用で seam ラップを回避 (A1 cylinder と同戦略)。

## 実装対象

<!-- Issue: #41 -->
<!-- 影響クレート: mycad-kernel, mycad-build (テスト) -->

- `crates/mycad-kernel/src/booleans/partition.rs:386-465` — Plane×Sphere skip 削除 + sphere UV クリップ回避
- `crates/mycad-kernel/src/booleans/partition.rs:759-829` — target ループ側で同様
- `crates/mycad-kernel/src/booleans/partition.rs` 末尾 — 多重 plane 交差ガード追加
- `crates/mycad-kernel/src/geometry/surface_intersect.rs` — 変更なし (大円ケースは partition 層で reject、geometry API は純粋関数に保つ)
- `crates/mycad-kernel/src/booleans/assemble.rs:346-460` — 球側 inner_loop 組み立て確認 (新規変更なしで動く想定; 動かなければ Cylinder 経路に倣って追加分岐)
- `crates/mycad-kernel/src/tessellation/mod.rs` — full sphere 検出を `inner_loops.is_empty()` で狭める + `tessellate_sphere_face_trimmed` 新設 (制限 v-range UV グリッド、earcut 不使用)
- `crates/mycad-kernel/src/error.rs` — `UnsupportedBooleanCase` variant 追加 (未存在なら)
- `examples/boolean_cut_sphere_dimple.mycad` — A2 acceptance 用 example
- `crates/mycad-build/tests/feature_dispatcher.rs` — A2 統合テスト T01-T13 追加
- `crates/mycad-build/tests/surface_boolean_a2_acceptance.rs` — `#[ignore]` スケルトン (STEP 5.5 で Claude が配置)

### 既存関数の編集 — before / after

#### partition.rs:386-390 (tool ループ Plane×Sphere skip 削除)

**before**:
```rust
crate::geometry::curve::Curve::Circle { .. } => {
    // Skip Plane×Sphere (deferred to #40)
    if is_sph || tool_is_sph {
        continue;
    }

    // For Plane×Cylinder: skip if the intersection plane is outside ...
    if tool_is_cyl {
```

**after**:
```rust
crate::geometry::curve::Curve::Circle { .. } => {
    // For Plane×Cylinder: skip if the intersection plane is outside ...
    if tool_is_cyl {
```

#### partition.rs:442-465 (tool ループ sphere UV クリップ回避)

tool ループ側に現状 `!is_cyl` UV クリップ分岐が無い箇所 (全 segment が clipped_s/clipped_e 前提) がある場合は以下を追加:

**before**:
```rust
let Some((clipped_s, clipped_e)) =
    clip_line_to_polygon_2d_given_points(s_a, s_b, polygon_2d)
else {
    continue;
};
let cl = ((clipped_e.0 - clipped_s.0).powi(2) + ...).sqrt();
if cl < len_eps { continue; }
segments.push(IntersectionSegment { p_start: clipped_s, p_end: clipped_e, ... });
```

**after**:
```rust
if !tool_is_cyl && !tool_is_sph {
    let Some((clipped_s, clipped_e)) =
        clip_line_to_polygon_2d_given_points(s_a, s_b, polygon_2d)
    else { continue; };
    let cl = (...).sqrt();
    if cl < len_eps { continue; }
    segments.push(IntersectionSegment { p_start: clipped_s, p_end: clipped_e, ... });
} else {
    // Cylinder / Sphere: seam を持つので UV クリップせず 3D 直接構築
    segments.push(IntersectionSegment { p_start: s_a, p_end: s_b, ... });
}
```

#### partition.rs:759-763 (target ループ Plane×Sphere skip 削除)

**before**:
```rust
crate::geometry::curve::Curve::Circle { .. } => {
    // Skip Plane×Sphere (deferred to #40)
    if is_sph || target_is_sph {
        continue;
    }

    let n_chords = ANGULAR_SEGMENTS_DEFAULT;
```

**after**:
```rust
crate::geometry::curve::Curve::Circle { .. } => {
    let n_chords = ANGULAR_SEGMENTS_DEFAULT;
```

#### partition.rs:791 (target ループ sphere UV クリップ回避)

**before**:
```rust
if !is_cyl {
    let Some((clipped_s, clipped_e)) = ... else { continue; };
    ...
    segments.push(...);
} else {
    segments.push(IntersectionSegment { p_start: s_a, p_end: s_b, ... });
}
```

**after**:
```rust
if !is_cyl && !is_sph {
    let Some((clipped_s, clipped_e)) = ... else { continue; };
    ...
    segments.push(...);
} else {
    // Cylinder / Sphere は seam を持つので 3D 直接構築
    segments.push(IntersectionSegment { p_start: s_a, p_end: s_b, ... });
}
```

#### surface_intersect.rs (大円ケース: 変更なし)

`intersect_plane_sphere` は純粋幾何 API として保持し、大円ケース (d=0) でも大円 `Curve::Circle` を返すまま。`UnsupportedBooleanCase` は Boolean 層の責任であり geometry 層に漏らさない。

#### partition.rs — 大円検出ガード (新規追加)

`partition_faces` 内で `intersect_plane_sphere` を呼んだ直後に `d.abs() < LENGTH_TOLERANCE` を検査して Err を返す:

```rust
// partition_faces 内、intersect_plane_sphere 結果処理前
let loops = intersect_plane_sphere(plane_surface, sphere_surface)?;
if !loops.is_empty() {
    let d = (sph_center - plane_origin).dot(&plane_normal);
    if d.abs() < LENGTH_TOLERANCE {
        return Err(KernelError::UnsupportedBooleanCase {
            reason: "plane through sphere center (great circle)",
        });
    }
}
```

実コード挿入位置は実装時に確認。

#### tessellation/mod.rs (full sphere 検出狭め)

**before** (概略 — 実コード行は実装時確認):
```rust
if /* Surface::Sphere */ && is_self_adjacent_periodic {
    return tessellate_full_sphere(...);
}
```

**after**:
```rust
if /* Surface::Sphere */ && is_self_adjacent_periodic && face_inner_loops.is_empty() {
    return tessellate_full_sphere(...);
}
// inner_loop がある球面は tessellate_sphere_face_trimmed へ fall through
```

## 設計方針

### 決定性要件

- 入力 IdGenerator を固定すれば 2 回 build で結果完全一致 (T01)
- partition.rs の chord 角度サンプリングは `ANGULAR_SEGMENTS_DEFAULT = 32` (固定定数)
- earcut の triangle 順序は inner_loop の hole 開始 index 順に決まる (決定的)
- tessellation の sphere UV サンプリング解像度も定数、random seed なし

### B-rep トポロジー妥当性

`box - sphere` (sphere の一部が box 内に food い込む、sphere の大半が box 外) の結果:
- box 上面 (Plane face): outer loop 4HE (矩形) + inner loop 1HE-circle (Curve::Circle) → **穴あき Plane**、genus-0 Euler
- sphere 残存面 (Sphere face self-adjacent): outer loop (seam 経路) + inner loop 1HE-circle (緯度線) → **穴あき球面**
- target が穴あき Plane + 球冠 Sphere face = genus-0 → **V-E+F=2 を期待**

`validate_manifold` を T03 で確認。

### 退化幾何の扱い

| ケース | 期待 |
|--------|------|
| tangent (`|d| = R`) | `intersect_plane_sphere` → `Ok(vec![])` → Boolean no-op |
| disjoint (`|d| > R`) | 同上 |
| 大円 (`d ≈ 0`) | `KernelError::UnsupportedBooleanCase { reason: "plane through sphere center (great circle)" }` |
| 多重 plane 交差 | `KernelError::UnsupportedBooleanCase { reason: "multi-plane sphere intersection" }` |
| 非 Z-aligned plane | `KernelError::UnsupportedSurfaceIntersection { reason: "non-Z-aligned plane × sphere" }` (既存) |
| sphere が完全 box 内 (全 plane と disjoint) | partition.rs segment 0 件 → no-op、既存 `t14_sphere_face_partition_no_panic` 挙動維持 |

### assemble.rs 球側 face 構成 (R-S1 採用)

球 face に緯度線円 (Curve::Circle) を **inner_loop** として追加。self-adjacent periodic の seam を outer loop に保持し、inner loop = 1HE-circle 構造にする。A1 cylinder の「outer_loop 4HE + inner loop 1HE」と同形。

`build_edge_curve_and_t_range` ヘルパ (`assemble.rs:439-460`) で `Some(Curve::Circle { .. })` が edge へ展開されるはずなので、新規コード不要の見込み。問題が出た場合は sphere ブランチを cylinder 経路に倣って追加。

### tessellation sphere trim 戦略 (R01 採用: 制限 v-range UV グリッド)

earcut は sphere の u=0/2π seam をまたぐ緯度線 hole で断裂するため **使わない**。代わりに:

**制限 v-range UV グリッドテッセレーション**:
1. inner loop の pcurve_on_sph (Curve2D::Line2D, direction=(1,0) なら v=y-component が v_lat) から v_lat を読み出す
   - または inner loop の Curve::Circle 3D edge から `center_z` と sphere surface の UV 逆算でも可
2. trim 方向を判定: `d > 0` (center が plane より上) なら lower cap (v ∈ [-π/2, v_lat]) が残存
3. n_lat = `ANGULAR_SEGMENTS_DEFAULT / 2` (16 rings), n_lon = `ANGULAR_SEGMENTS_DEFAULT` (32) で UV グリッドサンプリング
   - v を v_lat から -π/2 (または π/2) に向かって均等分割
   - 各 (v_i, u_j) を `surface.evaluate(u_j, v_i)` で 3D 変換
4. 隣接リングを四辺形 → 三角形に変換 (strip)
5. pole 付近 (|v| ≈ π/2) は三角形ファンで対応

この戦略は full sphere の `TessellationStrategy::UvSphere` を v 範囲制限した亜種で、seam u=0/2π は各行の始点・終点が同一 3D 点なので自然に閉じる。earcut 不使用のため hole polygon の seam 断裂問題を根本回避する。

新設関数: `tessellate_sphere_face_trimmed(face, surface, v_lat: f64, trim_lower: bool) -> Vec<Triangle>`

### derive 規約

新規 public 型の追加なし。`UnsupportedBooleanCase` variant は `error.rs` の既存 `KernelError` enum へ追加。既存 derive `#[derive(Debug, thiserror::Error)]` を継承。

### workspace.dependencies

変更なし。

## テスト計画（ID 付き）

| ID | 種別 | 内容 | 期待結果 |
|----|------|------|----------|
| T01 | 決定性 | 同一入力で 2 回 `build_feature` → Solid 完全一致 | `assert_eq!` 全フィールド |
| T02 | 正常系・体積 | box(10×10×10) − sphere(center=(5,5,11), R=3): plane z=10, d=1, h=R−d=2 → V_cap=πh²(R−h/3)=28π/3≈29.32, V_result≈970.68 | volume ±1.0 |
| T03 | manifold | T02 結果が `validate_manifold` 緑、V-E+F=2 | `assert!` |
| T04 | edge curve | intersection edge が `Curve::Circle { normal ≈ +Z, .. }` | match パターン |
| T05 | tessellation Plane | 上面 (穴あき Plane) が tessellate → triangle_count > 0、face area ≈ 100 − π·r² (r=√5) | float ±1.0 |
| T06 | tessellation Sphere | trim 後球面が tessellate → area ≈ 2π·R·h (球冠側面積) | float ±2.0 |
| T07 | 退化 tangent | sphere center z=13 (d=R=3, 接触のみ) → `intersect_plane_sphere` が `Ok(vec![])` → no-op | `assert_eq!` target unchanged |
| T08 | 退化 disjoint | sphere center z=14 (d=4>R=3, 完全外側) → no-op | 同上 |
| T09 | 退化 大円 | center z=10 (d=0) → `UnsupportedBooleanCase` | `assert_matches!` |
| T10 | 多重 plane | sphere が box 上下面を貫通 → `UnsupportedBooleanCase` | `assert_matches!` |
| T11 | 非 Z plane | plane normal が X 方向 → `UnsupportedSurfaceIntersection` | `assert_matches!` |
| T12 | golden YAML | A2 結果を serialize → deserialize → serialize が byte-identical | `assert_eq!` |
| T13 | example smoke | `examples/boolean_cut_sphere_dimple.mycad` が `export` で動作、STL > 1KB | `assert!` |

## 幾何的不変条件チェックリスト

- [x] partition 出力の polygon 頂点順と assemble の normal 処理が整合 — A1 と同形、assemble.rs の inner_loop 逆転ロジックを確認
- [x] 各プリミティブ face の outer_loop 2D 向き (CW/CCW) が文書化 — Sphere は UvSphere 既存戦略で CCW (外向き法線)
- [x] flip_normals / same_sense の意味論が明確 — A1 から変更なし
- [x] pslg_subdivide の出力向きが元の outer_loop 向きと整合 — Plane face の inner_loop の CW 向き (hole) を A1 と同じ方式で確認

## 実装順序

1. **STEP α**: `partition.rs:386-390` と `:759-763` の Plane×Sphere skip 削除 + sphere UV クリップ回避 (`!is_sph` 条件追加)。単体テスト `partition_plane_sphere_fragment_counts` を partition.rs 内に追加し緑確認。
2. **STEP β**: partition.rs の `intersect_plane_sphere` 呼び出し直後に大円 (d=0) guard 追加。単体テスト `boolean_cut_great_circle_returns_error` 追加。`intersect_plane_sphere` 自体は変更しない。
3. **STEP γ**: partition.rs 末尾に多重 plane 交差ガード追加。T10 テスト追加。
4. **STEP δ**: `tessellation/mod.rs` full sphere 検出を `inner_loops.is_empty()` で狭め + `tessellate_sphere_face_trimmed` 新設。単体テスト `tessellate_sphere_with_hole_returns_triangles` 追加。
5. **STEP ε**: `cargo test --workspace` が全緑。
6. **STEP ζ**: `crates/mycad-build/tests/feature_dispatcher.rs` に A2 統合テスト T01-T11 追加 (acceptance skeleton の `#[ignore]` 解除を含む)。
7. **STEP η**: `examples/boolean_cut_sphere_dimple.mycad` 追加、T13 緑。
8. **STEP θ**: T12 golden YAML snap。

## Verification

```bash
cargo xtask ci

mycad export examples/boolean_cut_sphere_dimple.mycad -o /tmp/A2.stl
ls -lh /tmp/A2.stl   # > 1KB

mycad view examples/boolean_cut_sphere_dimple.mycad
# 期待: 10×10×10 cube 上面中央に円形穴、穴の下に球冠状の窪み (depth=2)
```
