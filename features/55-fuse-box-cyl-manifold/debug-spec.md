# debug-spec.md — #55 Fuse(box+cyl) manifold fix

## 仮説（Claude が確定した根本原因）

### 間違っていた仮説
CW/CCW 向きの不一致 → 補正コード(ring_inner_poly_3d)を追加したが **is_ccw が常に false**
(z=-5 の circle normal=(0,0,-1) に対し ascending atan2 = CCW from +Z → dot with cn < 0 → false)
→ 補正コードは完全に no-op で、バグを直していない。

### 正しい根本原因: 離散化の不一致
- **band 境界円**: `lo_circle_3d.evaluate(k * dt)` / `hi_circle_3d.evaluate(k * dt)` k=0..63
  (64点均等分割、`partition.rs:1367-1384`)
- **ring 内ループ**: `collect_ordered_circle_polygon` が交線セグメント端点を収集してから
  atan2 ソートして unproject する `inner_poly_2d → inner_poly_3d` (変個点)
  (`partition.rs:576-581`)

この2種類の点集合は**完全に独立に計算**されるため、座標が `1e-9` 以内に一致しない。
→ `assemble.rs` の vertex_map でマージされない → 別エッジ → 各エッジが HE 1本 → manifold 違反。

DIAG 確認:
```
DIAG edge#141 HEs=1: v0=(0.0, 2.0, -5.0) v1=(-0.196034, 1.990369, -5.0) fwd=[true]
```
HEs=1 かつ fwd=[true] → band のみが寄与、ring 内ループの対応辺は別エッジになっている。

## 関連ファイル
- `crates/mycad-kernel/src/booleans/partition.rs:576-620` — 修正箇所
- `crates/mycad-kernel/src/brep/topology.rs:237-249` — 計装 eprintln! (削除必須)

## 修正方針

### 手順1: topology.rs の eprintln! を削除
`topology.rs:237-249` にある以下のブロックを削除する（元の `if forwards.len() != 2` のみ残す）:
```rust
// 削除するブロック:
if forwards.len() != 2 {
    let edge = &self.edges[edge_idx];
    let v0 = &self.vertices[edge.vertices[0]];
    let v1 = &self.vertices[edge.vertices[1]];
    eprintln!(
        "DIAG edge#{} HEs={}: ...",
        ...
    );
}
// ここまで削除。以下の if forwards.len() != 2 { return Err(...) } は残す
```

### 手順2: partition.rs の ring 内ループを band と同一離散化に揃える

**問題のあるコード (partition.rs:576-620) を以下に置き換える:**

```rust
// before: collect_ordered_circle_polygon の可変個端点 → unproject
let (inner_poly_2d, inner_partners, inner_curves, inner_tr, inner_pca, inner_pcb) =
    collect_ordered_circle_polygon(&circle_segs, surface);
let inner_poly_3d: Vec<Point> = inner_poly_2d
    .iter()
    .map(|(u, v)| unproject_from_face_uv(surface, *u, *v))
    .collect();

// CW/CCW fix (no-op な補正コード) を含む ring_inner_poly_3d の計算 全体を削除

// inner_polygons_3d: vec![ring_inner_poly_3d]
```

```rust
// after: band と同じ Circle::evaluate(k*dt) 64点で ring 内ループを生成
// provenance (partners/curves/tr) は collect_ordered_circle_polygon から流用し点数を 64 に揃える
let (inner_poly_2d, inner_partners_proto, inner_curves_proto, inner_tr_proto, inner_pca_proto, inner_pcb_proto) =
    collect_ordered_circle_polygon(&circle_segs, surface);
// provenance は引き続き collect_ordered_circle_polygon から得るが、3D 点は band と揃える
let _ = inner_poly_2d; // UV 座標は使わない（3D を直接生成）

let n_inner = ANGULAR_SEGMENTS_DEFAULT; // 64
let inner_poly_3d: Vec<Point> = {
    // circle 曲線を circle_segs から取得
    let circle_opt = circle_segs
        .first()
        .and_then(|s| s.source_curve_3d.as_ref());
    if let Some(circle) = circle_opt {
        let dt = 2.0 * std::f64::consts::PI / n_inner as f64;
        // band の hi circle は k=0,1,...,63 の順 (band の多角形辺は seam_hi→k1→...→k63→seam_hi)
        // ring 内ループは逆順 (k=0, k=63, k=62, ..., k=1) にして逆向きの HE を生成
        let mut pts = Vec::with_capacity(n_inner);
        pts.push(circle.evaluate(0.0)); // k=0 (seam と同じ位置)
        for k in (1..n_inner).rev() {
            pts.push(circle.evaluate(k as f64 * dt));
        }
        pts
    } else {
        // フォールバック: 旧来の unproject を使用
        inner_poly_2d
            .iter()
            .map(|(u, v)| unproject_from_face_uv(surface, *u, *v))
            .collect()
    }
};

// provenance を 64 点に揃える (collect_ordered_circle_polygon はプロトタイプのみ使用)
let first_partner = inner_partners_proto.first().and_then(|p| p.clone());
let first_curve = inner_curves_proto.first().and_then(|c| c.clone());
let first_pca = inner_pca_proto.first().and_then(|c| c.clone());
let first_pcb = inner_pcb_proto.first().and_then(|c| c.clone());
let inner_partners: Vec<Option<EntityRef>> = (0..n_inner).map(|_| first_partner.clone()).collect();
let inner_curves: Vec<Option<Curve>> = (0..n_inner).map(|_| first_curve.clone()).collect();
let inner_tr: Vec<[f64; 2]> = (0..n_inner).map(|_| [0.0, 2.0 * std::f64::consts::PI]).collect();
let inner_pca: Vec<Option<Curve2D>> = (0..n_inner).map(|_| first_pca.clone()).collect();
let inner_pcb: Vec<Option<Curve2D>> = (0..n_inner).map(|_| first_pcb.clone()).collect();

// ring fragment: outer ループはそのまま、inner ループに 64点逆順円を使用
let n_outer = polygon_3d.len();
target_fragments.push(FaceFragment {
    ...
    inner_polygons_3d: vec![inner_poly_3d],
    ...
    inner_boundary_partners: vec![inner_partners],
    inner_boundary_curves: vec![inner_curves],
    inner_boundary_t_ranges: vec![inner_tr],
    inner_boundary_pcurves_a: vec![inner_pca],
    inner_boundary_pcurves_b: vec![inner_pcb],
});
```

**正確性の保証:**
- `circle.evaluate(k*dt)` の呼び出しは band と全く同じ浮動小数点計算 → bit-exact に一致 → vertex_map でマージされる
- ring 内ループが k=0,63,62,...,1 の逆順 → band の k=0→1→...→63→0 と逆方向 → 各辺で forward+reverse の 2 HE が生成 → manifold OK

**注意:** disc fragment (`traversal_index: 1`) の `polygon_3d` は引き続き旧来の unproject で生成されているが、disc は `InsideOther` (Fuse では非選択) なので assemble に渡らない。変更不要。

## 試した修正と結果
- [x] CW/CCW 向き補正 (ring_inner_poly_3d with reversal): **失敗** — is_ccw=false で no-op。離散化不一致の根本原因は未解決。
  DIAG: `edge#141 HEs=1 fwd=[true]`
  
## 次にやること
1. topology.rs の eprintln! を削除
2. partition.rs の ring 内ループを上記 64点逆順 Circle::evaluate に置き換える
3. `cargo test -p mycad-build --test examples_smoke boolean_fuse_box_cyl -- --nocapture` で DIAG が出ないことを確認
4. `cargo xtask ci` で green を確認

## 追加で書いてほしいテスト
- acceptance テスト T01-T07 は test-spec.md を参照（既存スケルトン）。
- T08: is_ccw=false (ring が元から正しい向き) のケース確認 — box+cyl の二重貫通 Fuse で manifold OK なら T08 相当の確認は T02 で代替可。

