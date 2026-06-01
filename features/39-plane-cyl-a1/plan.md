## Non-Goals
<!-- スコープ外を必ず列挙。該当なしの場合も "- 該当なし" と書くこと（空欄禁止）。 -->
- **Plane×Sphere の Circle PSLG 投入**: Sphere の tessellation trim が必要 → 次 Issue (#40 相当)
- **Cylinder×Sphere face pair の取り扱い** (`partition.rs:232,390` の `continue`): 別 Issue (#41 相当, STEP G+A2)
- **A3 のメッシュ可視化**: 既に shipped 済み (sphere 全周 — trim 不要)
- **edge naming の golden / 100-run 決定性** (T22/T23/T30): 別 Issue (#42 相当, STEP H+I)
- **ADR-004 / CLAUDE.md docs 追記** (STEP I): 別 Issue (#42 相当)
- **Cone surface**: 触らない。`UnsupportedSurfaceIntersection` のまま
- **per-entity tolerance 移行**: Phase 5 foundation 候補、別 Issue
- **viewer 目視確認**: #35 に委譲 (本 Issue では STL export 出力のみ確認)
- **Cylinder 側面に穴がある場合のトリム** (inner_loop): A1 では発生しないため非対象
- **複数の内側ループ (Plane trim with 2 穴以上)**: A1 では 1 穴のみ。2 穴以上は `TrimmedFaceUnsupported` で reject

## 実装対象
<!-- Issue: #39 -->
<!-- 影響クレート/ファイル: -->
- `crates/mycad-kernel/src/booleans/partition.rs` — IntersectionSegment / FaceFragment 拡張、Circle 交線の chord サンプル化・interior loop 検出・PSLG 投入
- `crates/mycad-kernel/src/booleans/assemble.rs` — intersection edge の `Curve::Circle` 復元、`inner_loops` 付き Face 構成、pcurve attach
- `crates/mycad-kernel/src/tessellation/mod.rs` — Plane/Cylinder の trimmed face 対応 (穴あき平面 `earcutr`、Cylinder 部分高さ)
- `crates/mycad-kernel/src/geometry/math.rs` — `unwrap_periodic_uv` ヘルパー追加
- `Cargo.toml` (workspace) + `crates/mycad-kernel/Cargo.toml` — `earcutr` 追加
- `crates/mycad-build/tests/feature_dispatcher.rs` — A1 統合テスト (T01-T16)
- `examples/boolean_cut_cylinder_hole.mycad` — 新規サンプル

<!-- 変更する型・関数のシグネチャ (R02 採用後の確定版) -->
```rust
// partition.rs
/// Per-chord (or per-line-segment) intersection data with 3D curve + 2D pcurve provenance.
pub(crate) struct IntersectionSegment {
    pub p_start: (f64, f64),                 // face A の UV 座標
    pub p_end: (f64, f64),
    pub partner: EntityRef,                   // 相手 face の name
    // NEW: analytical source curve + per-chord parameter range
    pub source_curve_3d: Option<Curve>,       // Some(Circle{..}) for circle, None for Line
    pub curve_3d_t_range: [f64; 2],           // この chord の t range (circle: ta..tb)
    pub pcurve_on_a: Option<Curve2D>,         // face A 上の 2D curve (chord 区間)
    pub pcurve_on_a_t_range: [f64; 2],
    pub pcurve_on_b: Option<Curve2D>,         // face B 上の 2D curve (chord 区間)
    pub pcurve_on_b_t_range: [f64; 2],
}

/// Face fragment from partition — may have inner loops (holes).
pub(crate) struct FaceFragment {
    pub source_face_index: usize,
    pub polygon_3d: Vec<Point>,               // outer loop points
    pub inner_polygons_3d: Vec<Vec<Point>>,   // NEW: inner loop points (holes)
    pub surface: Surface,
    pub parent_name: EntityRef,
    pub traversal_index: u32,
    pub is_tool_side: bool,
    // outer boundary (R01 R2: 対称フィールドを追加)
    pub boundary_partners: Vec<Option<EntityRef>>,
    pub boundary_curves: Vec<Option<Curve>>,
    pub boundary_t_ranges: Vec<[f64; 2]>,           // NEW R01 R2: per outer edge t range
    pub boundary_pcurves_a: Vec<Option<Curve2D>>,   // NEW R01 R2: face A pcurve per outer edge
    pub boundary_pcurves_b: Vec<Option<Curve2D>>,   // NEW R01 R2: face B pcurve per outer edge
    // inner loops (holes)
    pub inner_boundary_partners: Vec<Vec<Option<EntityRef>>>,
    pub inner_boundary_curves: Vec<Vec<Option<Curve>>>,
    pub inner_boundary_t_ranges: Vec<Vec<[f64; 2]>>,
    pub inner_boundary_pcurves_a: Vec<Vec<Option<Curve2D>>>,
    pub inner_boundary_pcurves_b: Vec<Vec<Option<Curve2D>>>,
}

// assemble.rs
fn build_inner_loop(
    solid: &mut Solid,
    id_gen: &mut IdGenerator,
    inner_pts: &[Point],
    inner_partners: &[Option<EntityRef>],
    inner_curves: &[Option<Curve>],
    inner_pcurves_a: &[Option<Curve2D>],
    inner_pcurves_b: &[Option<Curve2D>],
    edge_map: &mut HashMap<[usize; 2], usize>,
    intersection_edge_names: &HashMap<[usize; 2], EntityRef>,
) -> usize; // returns loop_idx

// geometry/math.rs
pub fn unwrap_periodic_uv(u_list: &mut [f64]);
```

## 設計方針

### 1. IntersectionSegment の拡張 (R02 採用)

各 chord segment に per-chord データを持たせる:
- `source_curve_3d`: 元の解析的 3D 曲線 (Circle or None)
- `curve_3d_t_range`: この chord の t 区間 `[ta, tb]`
- `pcurve_on_a / _on_b`: 元の IntersectionLoop の `Curve2D` をそのまま保持 (全 chord が同じ Curve2D オブジェクトを共有 — t_range だけ chord ごとに異なる)
- `pcurve_on_a_t_range / _on_b_t_range`: chord に対応する t 区間

Line 交線は: source_curve_3d = None, curve_3d_t_range はクリップ後の linear t, pcurve は同様に保持。

### 2. FaceFragment の拡張 (R01 + R03 採用)

`pub(crate)` に変更し、inner loop 用フィールドを追加:
- `inner_polygons_3d`: 各 inner loop の 3D 頂点列
- `inner_boundary_partners / curves / pcurves_a / pcurves_b`: inner loop 各エッジの provenance

### 3. partition.rs の Circle 経路実装 — interior loop 検出 (R01 採用)

```rust
Curve::Circle { .. } => {
    // Plane×Sphere は本 Issue 非対象 — surface が Sphere なら skip
    if is_sphere_involved(surface, tool_surface) { continue; }

    let iloop_circle = &iloop.curve_3d;
    let t0 = iloop.t_range[0];
    let t1 = iloop.t_range[1];
    let dt = (t1 - t0) / ANGULAR_SEGMENTS_DEFAULT as f64;
    for k in 0..ANGULAR_SEGMENTS_DEFAULT {
        let ta = t0 + k as f64 * dt;
        let tb = t0 + (k + 1) as f64 * dt;
        let p3a = iloop_circle.evaluate(ta);
        let p3b = iloop_circle.evaluate(tb);
        let s_a = project_to_face_uv(surface, &p3a);
        let s_b = project_to_face_uv(surface, &p3b);
        // chord 長チェック、両面クリップ (既存 Line 経路と同様)
        // push IntersectionSegment { .., source_curve_3d: Some(iloop_circle.clone()),
        //   curve_3d_t_range: [ta, tb], pcurve_on_a: Some(iloop.pcurve_on_a.clone()),
        //   pcurve_on_a_t_range: [ta, tb], pcurve_on_b: ..., .. }
    }
}
```

segments 収集後、**interior loop 検出**:

```rust
fn segments_are_interior(
    segments: &[IntersectionSegment],
    outer_loop_2d: &[(f64, f64)],
) -> bool {
    // circle chord endpoint が全て outer_loop 境界上にない → interior
    segments.iter().all(|seg| {
        !point_on_polygon_boundary(seg.p_start, outer_loop_2d)
            && !point_on_polygon_boundary(seg.p_end, outer_loop_2d)
    })
}
```

interior の場合: `pslg_subdivide` を呼ばず、chord 頂点を atan2 順でソートして inner_polygon を組み立て、**元の face の polygon を outer_polygon のまま保持した FaceFragment を 1 個**生成:

```rust
if !segments.is_empty() && segments_are_interior(&segments, polygon_2d) {
    // 1) 外側 fragment (outer loop = original, inner loop = circle)
    let inner_poly_2d: Vec<(f64,f64)> = collect_ordered_circle_polygon(&segments);
    let inner_poly_3d: Vec<Point> = inner_poly_2d.iter()
        .map(|(u,v)| unproject_from_face_uv(surface, *u, *v))
        .collect();
    target_fragments.push(FaceFragment {
        source_face_index: fi,
        polygon_3d: polygon_3d.clone(),           // outer stays
        inner_polygons_3d: vec![inner_poly_3d],   // hole = circle
        surface: surface.clone(),
        parent_name: name.clone(),
        traversal_index: 0,
        is_tool_side: false,
        boundary_partners: vec![None; polygon_3d.len()],
        boundary_curves: vec![None; polygon_3d.len()],
        inner_boundary_partners: vec![...],       // circle edge partners
        inner_boundary_curves: vec![...],         // Curve::Circle{..}
        // ... pcurves from segment provenance ...
    });
} else if !segments.is_empty() && plane.is_some() {
    // standard pslg_subdivide (既存)
}
```

`collect_ordered_circle_polygon`: circle chord の endpoint を重複除去し、同一 Circle の atan2 順でソートして closed polygon を構成。

### 4. assemble.rs の Face 構成 (R01 採用)

FaceFragment の `inner_polygons_3d` が空でない場合、outer_loop + inner_loops で Face を構成:

```rust
// inner loops を先に build
let mut inner_loop_indices = Vec::new();
for (ip_idx, inner_pts) in cf.fragment.inner_polygons_3d.iter().enumerate() {
    let inner_loop_idx = build_inner_loop(
        &mut solid, id_gen, inner_pts,
        &cf.fragment.inner_boundary_partners[ip_idx],
        &cf.fragment.inner_boundary_curves[ip_idx],
        &cf.fragment.inner_boundary_pcurves_on_a[ip_idx],
        edge_map, &intersection_edge_names,
    );
    inner_loop_indices.push(inner_loop_idx);
}

let face_idx = solid.add_face(
    id_gen.next(),
    frag_surface,
    loop_idx,           // outer_loop
    inner_loop_indices, // NEW: inner loops
    same_sense,
    face_name,
);
```

`build_inner_loop` は既存の edge 構成ロジック (edge_map, he_forward/reverse_created を共有) を inner loop エッジに適用。HE への pcurve attach も here で実施 (provenance が揃っているので可能):

```rust
// inner loop edge の pcurve attach
solid.half_edges[he_idx].pcurve = inner_boundary_pcurves_on_a[ip_idx][k].as_ref().map(|c| {
    Pcurve {
        curve_2d: c.clone(),
        t_range: inner_boundary_t_ranges[ip_idx][k].1,
    }
});
```

### 5. assemble.rs の edge curve 復元 (R02 採用)

`boundary_curves[k]` を参照して edge curve を復元:
```rust
let edge_curve = match &cf.fragment.boundary_curves[k] {
    Some(c @ Curve::Circle { .. }) => c.clone(),
    _ => Curve::Line { origin: p0, direction: p1 - p0 },
};
```

`[t0, t1]` は `boundary_t_ranges[k]` から取得 (IntersectionSegment の curve_3d_t_range から FaceFragment に伝播)。

### 6. tessellation/mod.rs — Plane trim 分岐 (穴あき face)

`face.inner_loops` が空でない Plane face は `tessellate_face_planar_with_holes` へ:
1. outer_loop → UV 多角形 (project_to_face_uv の逆経路)
2. 各 inner_loop → UV 多角形
3. `earcutr` で穴あき多角形を三角形分割
4. UV → 3D → `mesh` に追加
5. inner_loop が 2 つ以上 → `TrimmedFaceUnsupported`

### 7. tessellation/mod.rs — Cylinder trim 分岐 (R03 R2 採用: 発火条件修正)

**発火条件**: 「HE 数 > 4」は廃止 (A1 の cylinder lateral は 4 HE だが trimmed のため不適切)。代わりに outer_loop HE の pcurve から v_range を計算し、`v_max − v_min < height − ε` の場合に trimmed と判定する。

実装:
1. outer_loop の各 HE の pcurve を走査し、`Curve2D::Line { direction: (1.0, 0.0), .. }` の形式 (u 方向の円 pcurve → v=const) から v_const を収集
2. `v_min = min(v_consts)`, `v_max = max(v_consts)`
3. `v_max − v_min < height − LENGTH_TOLERANCE` なら trimmed → 既存グリッドを `v_min..v_max` 範囲に縮小して再生成
4. seam 越えは `unwrap_periodic_uv` で正規化
5. inner_loop が存在する場合 → `TrimmedFaceUnsupported`

A1 の cylinder 側面: outer_loop = 4 HE (seam_up, top_circle_rev, seam_dn, bot_circle_fwd)。
top_circle の pcurve: v=10 (const)、bot_circle の pcurve: v=0 (const)。height=12。
`v_max−v_min = 10 < 12` → trimmed 分岐に正しく入る。✅

### 8. unwrap_periodic_uv

```rust
pub fn unwrap_periodic_uv(u_list: &mut [f64]) {
    use std::f64::consts::PI;
    for i in 1..u_list.len() {
        while u_list[i] - u_list[i-1] > PI  { u_list[i] -= 2.0 * PI; }
        while u_list[i] - u_list[i-1] < -PI { u_list[i] += 2.0 * PI; }
    }
}
```

`crates/mycad-kernel/src/geometry/math.rs` に `pub fn` として追加。

### 9. 決定性要件

- chord 頂点の `collect_ordered_circle_polygon` は atan2 昇順でソート → 決定的
- pslg_subdivide の既存動作は変えない
- inner loop エッジの `edge_map` 共有 → vertex index は決定的
- earcutr: 同一入力頂点順で同一三角形 → 決定的

### 10. 退化幾何の扱い

- Circle 半径 < `LENGTH_TOLERANCE` → `intersect_plane_cylinder` 段で `UnsupportedSurfaceIntersection`
- chord 長 < `LENGTH_TOLERANCE` → segment skip (既存 Line 経路と同じロジック)
- cylinder 軸が非垂直 → 既存 alignment 判定で `UnsupportedSurfaceIntersection`
- inner_loop が 2 つ以上 → `TrimmedFaceUnsupported` (scope 防衛)

### 11. derive 規約 (R03 採用)

- `IntersectionSegment`, `FaceFragment` を `pub(crate)` に変更
- `Curve2D` は既に `Clone` → 追加 derive 不要
- `IntersectionSegment` は serialize 不要 (内部作業用)
- `FaceFragment` も serialize 不要

### 12. workspace.dependencies

- `earcutr = "0.4"` を `[workspace.dependencies]` に追加
- `mycad-kernel` Cargo.toml で `earcutr = { workspace = true }`
- ADR-004 トレランスとの整合: earcutr は f64 ベース、閾値フリー → 整合

## テスト計画（ID 付き）
| ID | 種別 | 内容 | 期待結果 |
|----|------|------|----------|
| T01 | 決定性 | box 10³ Cut cyl r=2 h=12 を 2 回 build し Solid が byte-identical YAML | assert_eq! |
| T02 | 正常系 | A1 体積: box 貫通ケース (r=2 h=12, 上下両端 trim) ≈ 1000 − π·4·10 ≈ 874.34 | volume ±1.0 |
| T03 | 正常系 | A1 blind hole (r=2 h=6 で box 内に収まる) 体積 ≈ 1000 − π·4·6 ≈ 924.62 | volume ±1.0 |
| T04a | manifold | A1 blind hole (h=6): validate_manifold 緑、V-E+F = 2 (genus-0) | assert! |
| T04b | manifold | A1 thru-hole (h=12): validate_manifold 緑、V-E+F = 0 (genus-1 = solid torus) | assert! |
| T05 | edge curve | A1 結果の intersection edge が `Curve::Circle` を持つ | edge.curve が Circle |
| T06 | tessellation | A1 の top face (穴あき Plane) が tessellate でき三角形数 > 0、area ≈ 87.43 | |
| T07 | tessellation | A1 の cylinder 側面が tessellate でき area ≈ 2π·2·h_trim | |
| T08 | unit | `partition_faces` 後に cylinder の交線 segment に `source_curve_3d = Some(Circle)` がある | |
| T09 | 既存 | `intersect_plane_cylinder` 既存テスト緑 | no regression |
| T10 | unit | `unwrap_periodic_uv` 単体テスト (seam 跨ぎ → 連続出力) | |
| T11 | 既存反転 | A1 で発生する Plane trim (1 穴) / Cylinder partial height の期待値を Ok に反転 | |
| T12 | scope 防衛 | Plane trim with 2 穴は依然 `TrimmedFaceUnsupported` | Err |
| T13 | 正常系 | A1 同形状で cylinder 軸を Y 方向 (非 Z) — alignment 判定で `UnsupportedSurfaceIntersection` | Err |
| T14 | エラー | cylinder 軸が plane normal と 45° → `UnsupportedSurfaceIntersection` | Err |
| T15 | CLI | `mycad export examples/boolean_cut_cylinder_hole.mycad -o /tmp/x.stl` でファイル出力 > 0 bytes | file size > 0 |
| T16 | golden | A1 結果 Solid を YAML serialize → deserialize → serialize が byte-identical (Curve::Circle + Pcurve を含む新経路を固定) | assert_eq! |

## 幾何的不変条件チェックリスト
- [x] partition 出力の polygon 頂点順と assemble の normal 処理が整合: inner_polygon は atan2 CCW で組み立て、assemble は inner_loop の toward = outer_loop と逆
- [x] 各プリミティブの face outer_loop 2D 向き: Plane CCW、Cylinder UV CCW
- [x] flip_normals / same_sense: assemble の既存ロジック継承。Cylinder cut 側は flip。inner_loop は flip に関わらず outer の逆向き
- [x] pslg_subdivide の出力向き: 既存と同じ。interior circle 経路は pslg_subdivide を使わない (別経路)

## 実装順序

各 STEP 完了後に `cargo xtask ci` グリーンを期待。

1. **STEP α (型拡張 + pub(crate))** — `IntersectionSegment` に `source_curve_3d`, `curve_3d_t_range`, `pcurve_on_a/b`, `pcurve_on_a/b_t_range` 追加。`FaceFragment` に `inner_polygons_3d` と関連フィールド追加。両型を `pub(crate)` に変更。Line 経路で `None / [0,1]` を埋める。全既存テスト緑。
2. **STEP β (Circle PSLG 投入 + interior 検出)** — `partition.rs:319,474` の `continue` を chord サンプル化 + interior 検出に置換。`collect_ordered_circle_polygon` 実装。T08 緑。Sphere face pair は明示的 skip。
3. **STEP γ (edge curve 復元 + inner_loop 構成)** — `assemble.rs` の edge curve 参照変更、`build_inner_loop` 実装、inner loop の Face `inner_loops` 組み立て。T05 緑。全 Plane×Plane テスト緑。
4. **STEP δ (unwrap_periodic_uv)** — `geometry/math.rs` に追加 + T10。
5. **STEP ε (earcutr 追加 + Plane trim tessellation)** — workspace.dependencies 更新、`tessellate_face_planar_with_holes` 実装。T06 + T11 (Plane) + T12 緑。
6. **STEP ζ (Cylinder trim tessellation)** — Cylinder partial height 分岐実装 (pcurve から v 範囲取得、unwrap_periodic_uv)。T07 + T11 (Cylinder) 緑。
7. **STEP η (pcurve attach を inner loop HE に)** — `build_inner_loop` 内で pcurve provenance から HE.pcurve を設定。T16 golden roundtrip に必要。
8. **STEP θ (A1 acceptance + example)** — `examples/boolean_cut_cylinder_hole.mycad` 追加、T01-T07 + T15 緑。T16 golden snap。
9. **STEP ι (scope 防衛 + 最終 CI)** — T12-T14 + 既存 reject テスト緑確認。`cargo xtask ci` 最終グリーン化。

## Verification

```bash
cargo test --workspace
cargo clippy --workspace -- -D warnings
cargo fmt --all -- --check
cargo xtask ci
mycad export examples/boolean_cut_cylinder_hole.mycad -o /tmp/A1.stl
ls -lh /tmp/A1.stl
```
