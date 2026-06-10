# Issue #130 — トリム曲面のテッセレーション実装

## In-Scope / Out-of-Scope
| In-Scope | Out-of-Scope |
|----------|--------------|
| `boolean_box_cut` naked edge=0 修正（円筒側面・部分スパン） | #131（intersect_box_cyl の n_u cross-face adjacency） |
| `boolean_box_void` naked edge=0 修正（球面 inner_loop） | watertight 以外のメッシュ品質向上 |
| `boolean_cut_sphere_dimple` naked edge=0 修正（球面部分スパン） | 新プリミティブ追加 |
| `boolean_cut_cylinder_hole` の `#[ignore]` 2 件解除 | TS KNOWN_NAKED_EDGE_EXAMPLES から boolean_intersect_box_cyl を外すこと |
| 新ヘルパー `tessellate_trimmed_uv_face`（cylinder/sphere 共通） | count_naked_edges / assert_watertight_welded の共有モジュール化 |
| viewer.spec.ts KNOWN_NAKED_EDGE_EXAMPLES から本件 4 モデルを削除 | |

## Non-Goals
- #131（full 円筒 intersect の `n_u` cross-face adjacency）は別件
- `boolean_intersect_box_cyl` は TS の skip リストから外さない
- `count_naked_edges` / `assert_watertight_welded` の共有モジュール化は本件では行わない
- 極（v=±π/2）を含むディンプルへの対応（4 モデル対象外）

## Context（なぜやるか）

Phase 4 Boolean の負債。曲面同士の Cut で生まれる「穴あき曲面」を現状テッセレータが扱えず、
面を 1 枚も生成せず `TrimmedFaceUnsupported` を返すため、4 モデルでメッシュに穴が開く
（naked edge > 0、非 watertight）。

| モデル | 現状 naked | 欠落面 | 演算 |
|--------|-----------|--------|------|
| `boolean_box_cut` | 192 | 貫通穴の円筒側面（部分円スパン） | box(4³) − cyl(r1,h6) |
| `boolean_cut_cylinder_hole` | 0 ✅ | 既に通過（ignore 解除のみ） | box(10³) − cyl(r2,h6) |
| `boolean_box_void` | 96 | 球面ディンプル（inner_loop 付き球面） | box(6³) − sphere(r2) |
| `boolean_cut_sphere_dimple` | 96 | トリム球面キャップ | box(10³) − sphere(r3) |

ゴール: 上記 4 モデルの naked edge = 0、`#[ignore]` テスト 2 件を GREEN 化、`cargo xtask ci` green。

## 実装対象
<!-- Issue: #130 -->

対象ファイル:
- `crates/mycad-kernel/src/tessellation/mod.rs` — コア実装
- `crates/mycad-kernel/tests/tessellation_cap_acceptance.rs` — `#[ignore]` 解除
- `crates/mycad-kernel/tests/bool_naked_edge_acceptance.rs` — `#[ignore]` 解除
- `web/tests/viewer.spec.ts` — KNOWN_NAKED_EDGE_EXAMPLES 更新

### 診断結果（再現ファースト済み）

```
boolean_box_cut: naked@1e-10=192, naked@1e-6=192
boolean_cut_cylinder_hole: naked@1e-10=0 ✅
boolean_box_void: naked@1e-10=96
boolean_cut_sphere_dimple: naked@1e-10=96
```

面トポロジー:
- boolean_box_cut → face[6] Cylinder: circle_he=128, line_he=2, arcs_per_rev=64, span_ok=true
  - 問題: inner_loop なし、フル周回、line_he=2 が `line_he > 2` を満たさない → n_u=32 になり隣接面(64点)と不一致
  - 実際の問題: boolean_box_cut の Cylinder は `部分スパン` ではなくフル周回。ただし n_u ミスマッチ。
  - `collect_loop_points` で実際の boundary 点数に合わせる
- boolean_box_void → face[6] Sphere: outer_he=2, inner_loops=1, circle_span=6.2832
  - 球面の inner_loop が circle（緯度平行カット）
  - `tessellate_sphere_face_trimmed` が n_u=32 固定で生成 → 隣接面(64点)と不一致
- boolean_cut_sphere_dimple → face[6] Sphere: outer_he=2, inner_loops=1, circle_span=6.2832
  - 同上

## 設計方針: 境界駆動の UV-earcut

既存の `tessellate_face_earcut`（`mod.rs:276-363`）が平面トリム面で確立した「outer+inner ループ → 2D 射影
→ `earcutr::earcut(flat, hole_indices, 2)` → same_sense で winding 反転」を、**射影平面の代わりに曲面の
UV パラメータ空間 (u,v) で**実行する。

watertight 化の鍵は解像度合わせ（#129 の `n_u` ヒューリスティック）ではなく**境界点列の共有**:
トリム面の境界を `collect_loop_points`（`mod.rs:367-416`、pcurve とアーク分割に対応済み）で生成すると、
隣接面（平面キャップ等）が同一エッジから得る点列と一致し、ウェルド後に各辺がちょうど 2 三角形で共有される。

### 新ヘルパー `tessellate_trimmed_uv_face`（cylinder/sphere 共通）

`crates/mycad-kernel/src/tessellation/mod.rs` に追加:

```rust
fn tessellate_trimmed_uv_face(
    solid: &Solid,
    face: &Face,
    face_idx: usize,
    opts: &TessellationOptions,
    mesh: &mut TriangleMesh,
) -> Result<(), TessellationError>
```

手順:
1. outer_loop を `collect_loop_points` で 3D サンプル → `surface.uv_of(p)` で (u,v) 化
2. u の周期境界を `unwrap_periodic_uv`（`math.rs:76-86`）でアンラップ
3. 各 inner_loop も同様に (u,v) 化 → 穴（hole）
4. `tessellate_face_earcut` と同じ要領で flat 座標配列＋`hole_indices` を組み、`earcutr::earcut` を呼ぶ
5. 三角形を mesh に追加。法線は `surface.normal_at(u,v)` から 3D 点で計算、winding は `face.same_sense` で反転

### 既存ディスパッチの差し替え

**円筒側面** `tessellate_face_uv_grid`（`mod.rs:419-564`）:
- `before`:
  ```rust
  // mod.rs:426-428 inner_loop 早期 return
  if !face.inner_loops.is_empty() {
      return Err(TessellationError::TrimmedFaceUnsupported);
  }
  // mod.rs:436-454 部分スパン拒否
  if !span_ok {
      return Err(TessellationError::TrimmedFaceUnsupported);
  }
  ```
- `after`:
  ```rust
  // inner_loop あり or 部分スパン → UV-earcut へ委譲
  if !face.inner_loops.is_empty() || !span_ok {
      return tessellate_trimmed_uv_face(solid, face, face_idx, opts, mesh);
  }
  ```

**球面** `tessellate_face_sphere`（`mod.rs:571-774`）:
- inner_loop 時の委譲先 `tessellate_sphere_face_trimmed`（`mod.rs:781-917`）を
  任意ディンプルを扱える `tessellate_trimmed_uv_face` ベースへ置き換え
- `before`: `tessellate_sphere_face_trimmed(solid, face, il_idx, opts, mesh)`
- `after`: `tessellate_trimmed_uv_face(solid, face, face_idx, opts, mesh)`

### 数値モデル
- ε_snap = 1e-10（naked edge カウント用）
- ε_area = 1e-12（earcut 三角形面積ゼロ除外）
- UV 周期: u ∈ [0, 2π]、アンラップは `unwrap_periodic_uv` を使用

## テスト計画（ID 付き）
| ID | 種別 | 内容 | 期待結果 |
|----|------|------|----------|
| T01 | 決定性 | boolean_box_cut を 2 回実行し全座標・法線・インデックスが一致 | assert_eq! |
| T02 | 正常系 | boolean_box_cut naked_edge = 0 | 0 |
| T03 | 正常系 | boolean_box_void naked_edge = 0 | 0 |
| T04 | 正常系 | boolean_cut_sphere_dimple naked_edge = 0 | 0 |
| T05 | ignore 解除 | t04_watertight_cut_hole GREEN | PASS |
| T06 | ignore 解除 | t04_boundary_degen_cut_cyl GREEN | PASS |
| T07_degen_inner | 境界 | inner_loop が 3 点以下のポリゴン（earcut が退化三角形を出す場合） | クラッシュしない |
| T08_boundary_seam | 境界 | seam 跨ぎ円筒（u = 0/2π 境界）が正しく UV アンラップされる | naked_edge = 0 |

## 幾何的不変条件チェックリスト
- [x] partition 出力の polygon 頂点順と assemble の normal 処理が整合しているか（`same_sense` で winding 反転済み）
- [x] 各プリミティブの face ごとの outer_loop 2D 向き（CW/CCW）が文書化されているか（`collect_loop_points` 依存）
- [x] flip_normals / same_sense の意味論が明確か（頂点順を変えることで法線を反転）
- N/A pslg_subdivide（本件非該当）

## Reuse（既存資産）
- `earcutr` クレート: workspace 依存済み
- `tessellate_face_earcut`（`mod.rs:276-363`）: flat+hole_indices+winding の雛形
- `collect_loop_points`（`mod.rs:367-416`）: pcurve/アーク分割対応の境界サンプラ
- `Surface::uv_of` / `evaluate` / `normal_at_point`
- `unwrap_periodic_uv` / `arc_segment_count`（`geometry/math.rs`）

## Risks / Edge cases
- **u seam 跨ぎ**: 周期アンラップを誤ると earcut が自己交差ポリゴンを受け取り破綻
- **球の極特異点** (v=±π/2): 4 モデルのディンプルは極を含まない（球心オフセット）
- **inner_loop 小型ポリゴン**: earcut が 3 点以下を受け取ると退化三角形を出す可能性
