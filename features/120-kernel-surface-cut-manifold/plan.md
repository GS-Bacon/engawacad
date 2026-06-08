## In-Scope / Out-of-Scope

| In-Scope | Out-of-Scope |
|----------|--------------|
| `partition_faces` にて、線分が target 面の内側で閉じたループを形成する場合の ring/disc fragment 生成 | 円弧・円とのハイブリッドケース（line+circle 混在内部ループ） |
| `chain_segments_into_polygon` ヘルパ関数の追加 | tool ループ側の同類修正（surface cut の基本ケースでは不要） |
| `boolean_proptest.rs` T02 の `#[ignore]` 解除 | `pslg_subdivide` 内部の DCEL アルゴリズム変更 |
| 回帰: 既存 void cut・void fuse・通常 cut テストが引き続き green | 複数閉内部ループ（多重穴）への対応 |

## Non-Goals

- 円弧セグメントと直線セグメントが混在して閉内部ループを形成するケースの修正
- tool ループ側（tool 面が target 面の内部で閉ループを形成するケース）の修正
- STEP ファイル export における pcurve/t_range の厳密な provenance 伝播
- edge naming の provenance 精度向上（inner_boundary_partners は None で OK）

## 実装対象

**Issue**: #120  
**影響クレート**: `crates/mycad-kernel/src/booleans/partition.rs`  
**変更内容**:
1. 新規関数 `chain_segments_into_polygon` の追加
2. `partition_faces` のターゲット面処理ループに新分岐追加（circle interior ケースの直後）
3. `crates/mycad-kernel/tests/boolean_proptest.rs` の T02 から `#[ignore]` を削除

### 修正箇所 before / after

**Before** (`partition_faces` の circle interior ケースの直後、else分岐):
```rust
} else {
    // Standard PSLG subdivision (line segments or circle segments crossing boundary)
    let sub_faces = pslg_subdivide(polygon_2d, &segments, plane_data);
    for (idx, (sub_poly_2d, edge_partners)) in sub_faces.into_iter().enumerate() {
        let poly_3d: Vec<Point> = sub_poly_2d
            .iter()
            .map(|(u, v)| unproject_from_face_uv(surface, *u, *v))
            .collect();
        let n = poly_3d.len();
        target_fragments.push(FaceFragment { ... });
    }
}
```

**After** (新分岐を circle interior と標準 PSLG の間に挿入):
```rust
} else if !line_segs.is_empty()
    && circle_segs.is_empty()
    && segments_are_interior(&line_segs, polygon_2d)
{
    // Surface cut: all line segments form an interior closed loop.
    // pslg_subdivide cannot produce the ring region for disconnected PSLGs,
    // so we detect and handle this case explicitly.
    if let Some((inner_poly_2d, inner_partners)) =
        chain_segments_into_polygon(&line_segs, len_eps)
    {
        // Ring fragment: outer polygon with the interior polygon as a hole.
        let ring_inner_poly_3d: Vec<Point> = inner_poly_2d
            .iter()
            .rev()  // reverse = CW winding for the hole
            .map(|(u, v)| unproject_from_face_uv(surface, *u, *v))
            .collect();
        let n_inner = ring_inner_poly_3d.len();
        let n_outer = polygon_3d.len();
        target_fragments.push(FaceFragment {
            source_face_index: fi,
            polygon_3d: polygon_3d.clone(),
            inner_polygons_3d: vec![ring_inner_poly_3d],
            surface: surface.clone(),
            parent_name: name.clone(),
            traversal_index: 0,
            is_tool_side: false,
            boundary_partners: vec![None; n_outer],
            boundary_curves: vec![None; n_outer],
            boundary_t_ranges: vec![[0.0, 1.0]; n_outer],
            boundary_pcurves_a: vec![None; n_outer],
            boundary_pcurves_b: vec![None; n_outer],
            inner_boundary_partners: vec![vec![None; n_inner]],
            inner_boundary_curves: vec![vec![None; n_inner]],
            inner_boundary_t_ranges: vec![vec![[0.0, 1.0]; n_inner]],
            inner_boundary_pcurves_a: vec![vec![None; n_inner]],
            inner_boundary_pcurves_b: vec![vec![None; n_inner]],
        });

        // Disc fragment: the interior polygon itself (classified InsideOther → not selected for Cut).
        let inner_poly_3d: Vec<Point> = inner_poly_2d
            .iter()
            .map(|(u, v)| unproject_from_face_uv(surface, *u, *v))
            .collect();
        let n_disc = inner_poly_3d.len();
        target_fragments.push(FaceFragment {
            source_face_index: fi,
            polygon_3d: inner_poly_3d,
            inner_polygons_3d: vec![],
            surface: surface.clone(),
            parent_name: name.clone(),
            traversal_index: 1,
            is_tool_side: false,
            boundary_partners: inner_partners,
            boundary_curves: vec![None; n_disc],
            boundary_t_ranges: vec![[0.0, 1.0]; n_disc],
            boundary_pcurves_a: vec![None; n_disc],
            boundary_pcurves_b: vec![None; n_disc],
            inner_boundary_partners: vec![],
            inner_boundary_curves: vec![],
            inner_boundary_t_ranges: vec![],
            inner_boundary_pcurves_a: vec![],
            inner_boundary_pcurves_b: vec![],
        });
    } else {
        // Cannot chain into a single closed loop — fall back to pslg_subdivide.
        let sub_faces = pslg_subdivide(polygon_2d, &segments, plane_data);
        for (idx, (sub_poly_2d, edge_partners)) in sub_faces.into_iter().enumerate() {
            let poly_3d: Vec<Point> = sub_poly_2d
                .iter()
                .map(|(u, v)| unproject_from_face_uv(surface, *u, *v))
                .collect();
            let n = poly_3d.len();
            target_fragments.push(FaceFragment {
                source_face_index: fi,
                polygon_3d: poly_3d,
                inner_polygons_3d: vec![],
                surface: surface.clone(),
                parent_name: name.clone(),
                traversal_index: idx as u32,
                is_tool_side: false,
                boundary_partners: edge_partners,
                boundary_curves: vec![None; n],
                boundary_t_ranges: vec![[0.0, 1.0]; n],
                boundary_pcurves_a: vec![None; n],
                boundary_pcurves_b: vec![None; n],
                inner_boundary_partners: vec![],
                inner_boundary_curves: vec![],
                inner_boundary_t_ranges: vec![],
                inner_boundary_pcurves_a: vec![],
                inner_boundary_pcurves_b: vec![],
            });
        }
    }
} else {
    // Standard PSLG subdivision
    // ... (既存コードそのまま)
}
```

### `chain_segments_into_polygon` 実装仕様

```rust
/// Try to chain all intersection segments into a single closed ordered CCW polygon.
/// Returns None if segments cannot be chained into exactly one closed loop.
fn chain_segments_into_polygon(
    segs: &[&IntersectionSegment],
    eps: f64,
) -> Option<(Vec<(f64, f64)>, Vec<Option<EntityRef>>)> {
    if segs.is_empty() { return None; }
    
    let mut pts: Vec<(f64, f64)> = Vec::new();
    let mut partners: Vec<Option<EntityRef>> = Vec::new();
    let mut used = vec![false; segs.len()];
    
    used[0] = true;
    pts.push(segs[0].p_start);
    partners.push(Some(segs[0].partner.clone()));
    let mut cur = segs[0].p_end;
    
    for _ in 1..segs.len() {
        // Find next segment matching cur (try both directions)
        let found = segs.iter().enumerate().find(|(i, s)| {
            !used[*i]
                && (dist_2d(s.p_start, cur) < eps || dist_2d(s.p_end, cur) < eps)
        });
        let Some((i, seg)) = found else { return None; };
        used[i] = true;
        if dist_2d(seg.p_start, cur) < eps {
            pts.push(seg.p_start);
            partners.push(Some(seg.partner.clone()));
            cur = seg.p_end;
        } else {
            pts.push(seg.p_end);
            partners.push(Some(seg.partner.clone()));
            cur = seg.p_start;
        }
    }
    
    // Must close back to start
    if dist_2d(cur, pts[0]) >= eps { return None; }
    if pts.len() < 3 { return None; }
    
    // Normalize to CCW (positive signed area)
    if signed_area_2d(&pts) < 0.0 {
        pts.reverse();
        partners.reverse();
    }
    
    Some((pts, partners))
}

fn dist_2d(a: (f64, f64), b: (f64, f64)) -> f64 {
    ((a.0 - b.0).powi(2) + (a.1 - b.1).powi(2)).sqrt()
}
```

## 設計方針

### 根本原因の分析

`partition_faces` でターゲット面の処理時、tool が target 面を貫通する「surface cut」ケースでは:
1. ターゲット x=5 面に 4 本の交差線分が生成される（tool の 4 つの側面からの交差）
2. これら 4 本の線分の端点はすべて x=5 面の外側境界内部にある（境界上にない）
3. `segments_are_interior` は true を返すが、この関数は circle にしか使われていない
4. `pslg_subdivide` に渡すと、非連結 PSLG（外側多角形と内側矩形が独立）のため:
   - 外側多角形の走査 → 外側境界面（面積最大、フィルタで除去）
   - 内側矩形の走査 × 2 方向 → 2 つの内側矩形ファセット
   - **リング領域（外側 - 内側矩形）は生成されない**
5. 結果: 交差辺（4 辺）に tool 側 HE しかなく、ring 側 HE が存在しない → manifold 違反

### 修正の正当性

修正後の fragment 生成:
- **Ring fragment** (OutsideOther → Cut で selected):
  - `polygon_3d`: 外側多角形（20×30 矩形、CCW）
  - `inner_polygons_3d[0]`: 内側矩形（2×2、CW = reversed）
  - `classify.rs` の `get_fragment_interior_point`: outer_pt × 0.9 + inner_pt × 0.1 → tool 外部 → OutsideOther ✓
- **Disc fragment** (InsideOther → Cut で not selected):
  - `polygon_3d`: 内側矩形（2×2、CCW）
  - tool 内部の内点 → InsideOther ✓

HalfEdge 整合:
- 交差辺 E1 (例: x=5, z=1, y ∈ [-1,1]):
  - ring inner loop (CW): (5,1,1) → (5,-1,1) → HE forward=false
  - tool 前面 inner fragment: (5,-1,1) → (5,1,1) → HE forward=true
  - → 逆方向 2 HE → manifold ✓

### 決定性

- `chain_segments_into_polygon` はセグメントリストを前から走査するため同一入力で同一出力
- `signed_area_2d` の正負で CCW/CW を決定的に判定

### B-rep トポロジー妥当性

修正後:
- 各辺に forward HE 1 + reverse HE 1 = 計 2 HE → `validate_manifold` の "must have exactly 2 half-edges" を満たす
- ring fragment の outer loop と隣接 target 面の shared edges も変わらず 2 HE を満たす

### 退化幾何の扱い

- `chain_segments_into_polygon` が None を返す場合（閉ループ非形成）→ 既存の `pslg_subdivide` にフォールバック
- 内側多角形が < 3 頂点の場合 → `chain_segments_into_polygon` が None を返すため安全

## テスト計画（ID 付き）

| ID | 種別 | 内容 | 期待結果 |
|----|------|------|----------|
| T01_determinism | 決定性 | surface cut を同一 seed で 2 回実行し頂点数・辺数・面数が一致 | assert_eq! |
| T02_surface_cut_manifold | 正常系 | proptest: x_offset ∈ [4.1, 5.4], 32 cases で surface cut 成功・valid manifold | validate_manifold().is_ok() |
| T02_volume_reduced | 正常系 | surface cut 後の体積が cut 前より小さい | vol_c < vol_t |
| T02_boundary_large_offset | 境界系 | x_offset=5.3 (tool が大きく貫通) でも manifold valid | validate_manifold().is_ok() |
| T02_degen_flush | 退化系 | x_offset=4.0 (tool 右面が target 面に flush) → Ok or 適切な Err | パニックしない |

**備考**: T02 は `boolean_proptest.rs` の既存 `t02_prop_surface_cut_reduces_volume` の `#[ignore]` を解除することで対応。  
T01, T02_boundary_large_offset, T02_degen_flush は `kernel_surface_cut_manifold_acceptance.rs` に新規追加。

## 幾何的不変条件チェックリスト

- [x] partition 出力の polygon 頂点順と assemble の normal 処理が整合しているか
  - ring outer: CCW（元 target 面と同じ）→ flip_normals なし → same_sense=true ✓
  - ring inner: CW（逆順）→ assemble の inner loop HE が逆方向 → tool 側と対向 ✓
- [x] 各プリミティブの face ごとの outer_loop 2D 向き（CW/CCW）が文書化されているか
  - `signed_area_2d` で CCW を明示的に正規化
- [x] flip_normals / same_sense の意味論が明確か
  - ring fragment: is_tool_side=false → flip_normals 適用なし → same_sense=true
  - disc fragment: is_tool_side=false, InsideOther → not selected → 影響なし
- [x] pslg_subdivide の出力向きが元の outer_loop 向きと整合しているか
  - 新分岐では pslg_subdivide を使わず、outer polygon をそのまま polygon_3d に使用 ✓
