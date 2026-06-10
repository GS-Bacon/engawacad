# Debug Spec — Round 1

## 失敗テスト
`t02_volume_sphere_dimple` (crates/mycad-build/tests/surface_boolean_a2_acceptance.rs:90)
- 期待: volume ≈ 970.68
- 実際: volume ≈ 958.18 (差 -12.5 cm³)

## 根本原因

球面トリム face のトポロジーは **inner_loop = カッティング円（実際の外側境界）、outer_loop = シームエッジ（自己隣接）** という構造。

```
boolean_cut_sphere_dimple → face[6] Sphere:
  outer_he=2, inner_loops=1, circle_span=6.2832
```

- `outer_loop` (2 HE, span=2π): 球の経線シーム（self-adjacent meridian）
- `inner_loop[0]` (≥2 HE): カッティング円（box top face との交差円）

`tessellate_trimmed_uv_face` は `outer_loop` を earcut のポリゴン境界として、`inner_loop` を穴として扱う。
しかし球面では roles が逆転しているため、体積計算が 12.5 cm³ ずれる（cap volume の約43%）。

さらに、球面極 (v = ±π/2) で UV が縮退し、earcut に不安定な入力が渡るリスクもある。

## 修正方針

**球面の inner_loop ケースは `tessellate_sphere_face_trimmed` を使い続ける**。

ただし naked edge 問題（n_u=32 vs 隣接面 64 点）を修正するため、boundary ring の生成を
`collect_loop_points` ベースに変更する:

### 変更箇所: `tessellate_sphere_face_trimmed`（mod.rs ~line 956）

```rust
// 現状（問題）
let n_u = opts.angular_segments.max(3);
let n_v = (n_u / 2).max(2);
let du = 2.0 * PI / n_u as f64;
// ... for iu in 0..n_u { let u = du * iu as f64; ... }

// 修正後
// inner_loop の実際の境界点を使う（隣接面と一致するため watertight になる）
let boundary_pts = collect_loop_points(solid, il, opts.angular_segments.max(3), face_idx)?;
let n_u = boundary_pts.len().max(3);
let n_v = (n_u / 2).max(2);

// boundary ring: boundary_pts を直接使用（uniform sampling ではなく実トポロジー由来）
let ring_start = mesh.positions.len() as u32;
for p in &boundary_pts {
    let normal = face.surface.normal_at_point(p);
    mesh.positions.push([p.x, p.y, p.z]);
    mesh.normals.push(normal_arr(&normal, face.same_sense));
}
// その後は既存の internal rings + pole fan をそのまま使用（n_u が変わっただけ）
```

### 変更箇所: `tessellate_face_sphere`（inner_loops ≥ 1 のディスパッチ）

```rust
// 現状（GLM が変更）
if !face.inner_loops.is_empty() {
    return tessellate_trimmed_uv_face(solid, face, face_idx, opts, mesh, face_id);
}

// 修正後（tessellate_sphere_face_trimmed に戻す）
if !face.inner_loops.is_empty() {
    return tessellate_sphere_face_trimmed(solid, face, face_idx, opts, mesh, face_id);
}
```

### `tessellate_sphere_face_trimmed` のシグネチャ変更

`face_idx: usize` パラメータを追加（`collect_loop_points` に渡す）:
```rust
fn tessellate_sphere_face_trimmed(
    solid: &Solid,
    face: &crate::brep::topology::Face,
    face_idx: usize,  // ← 追加
    opts: &TessellationOptions,
    mesh: &mut TriangleMesh,
    face_id: &str,
) -> Result<(), TessellationError>
```

### `#[allow(dead_code)]` 削除

GLM が `tessellate_sphere_face_trimmed` に `#[allow(dead_code)]` を追加したが、
修正後は再び使われるので削除すること。

## ⚠️ Round 2 追加の問題: t03_outward_normals_intersect も失敗

### 原因
`collect_loop_points` が返す境界点の「順序」が uniform u sampling（`u = du * iu`）と逆の場合がある。
`tessellate_sphere_face_trimmed` の内部リングは `u = du * iu`（昇順）で生成されるが、
boundary ring が `collect_loop_points` 順だと winding が逆になり法線が反転する。

### 修正方針の修正（重要）

境界リングのサンプリング方法を変えず、**n_u の値だけ** を `collect_loop_points` から求める。

```rust
// ① collect_loop_points を使って点数（count）だけ取得
let boundary_count = collect_loop_points(solid, il, opts.angular_segments.max(3), face_idx)?.len();
let n_u = boundary_count.max(3);
let n_v = (n_u / 2).max(2);

// ② 境界リングは従来どおり uniform u sampling（winding を崩さない）
let ring_start = mesh.positions.len() as u32;
let du = 2.0 * PI / n_u as f64;
for iu in 0..n_u {
    let u = du * iu as f64;
    let p = face.surface.evaluate(u, v_boundary);
    let normal = face.surface.normal_at(u, v_boundary);
    mesh.positions.push([p.x, p.y, p.z]);
    mesh.normals.push(normal_arr(&normal, face.same_sense));
}
// 内部リング・ポールファンは現状のまま（n_u が変わっただけ）
```

境界リング点の位置:
- 均一 u サンプリング: `evaluate(du*iu, v_boundary)` で `iu=0,1,...,n_u-1`
- 隣接 box 面の inner_loop: 64 個の等間隔 arc HE（それぞれ 2π/64 間隔）
- 同じ n_u=64 かつ同じ間隔 → 位置が一致 → weld 後に naked edge = 0

## 試した修正と結果
- Round 0: `tessellate_trimmed_uv_face` を使用 → volume regression (-12.5)
- Round 1 (debug-spec R1): `tessellate_sphere_face_trimmed` に戻す + `collect_loop_points` で boundary ring の点列を使用
  → t02 volume PASS, t03 outward normals FAIL（boundary ring の winding 崩れ）

## 次にやること（Round 3）
1. `tessellate_face_sphere` の inner_loops ディスパッチを `tessellate_sphere_face_trimmed` に戻す（Round 2 は正しい）
2. `tessellate_sphere_face_trimmed` に `face_idx: usize` を追加し、boundary ring の **点数のみ** を `collect_loop_points` から取得 — サンプリング自体は `evaluate(du*iu, v_boundary)` の uniform 方式を維持
3. `cargo xtask ci` green を確認
4. naked_edge=0 と volume≈970.68 を両方確認

## 追加で書いてほしいテスト
なし（既存テストが両方の要件をカバー済み）

## 保持すべき変更（戻さないこと）
- `tessellate_trimmed_uv_face` 関数の追加（cylinder trimmed に必要）
- `tessellate_face_uv_grid` の inner_loops + 部分スパン → `tessellate_trimmed_uv_face` へのディスパッチ
- `ignore` 解除に関連する変更

---

# Debug Spec — Codex F02 (STEP 7.5)

## 失敗テスト
なし（現在のテストスイートでは再現しないが、複数 inner_loop で最初が退化する場合に誤った三角形が生成される）

## 根本原因

`tessellate_trimmed_uv_face` 内の `all_points` 構築で **全ての** inner_loop の 3D 点を収集しているが、
`flat_coords` 構築ループは退化 inner_loop（`il_3d.len() < 3`）を `continue` でスキップしている。

結果: earcut の頂点インデックスは `flat_coords` の並び（退化 loop を除外）に基づくが、
`all_points` には退化 loop の点も含まれ、インデックスがずれる。

## 問題箇所

`crates/mycad-kernel/src/tessellation/mod.rs` 内の `tessellate_trimmed_uv_face`:

```rust
// 現状（バグ）: face.inner_loops を再収集 → 退化 loop も含む
let all_points: Vec<Point> = outer_3d
    .into_iter()
    .chain(face.inner_loops.iter().flat_map(|&il_idx| {
        let il = &solid.loops[il_idx];
        collect_loop_points(solid, il, segments, face_idx).unwrap_or_default()
    }))
    .collect();
```

## 修正方針

flat_coords 構築ループで実際に追加した inner_loop の 3D 点を追跡し、`all_points` にはその点列のみ使用する。

### 変更箇所

`tessellate_trimmed_uv_face` の `hole_starts` ループの冒頭と `all_points` 構築:

```rust
// ①: ループ変数追加
let mut hole_starts: Vec<usize> = Vec::new();
let mut inner_3d_added: Vec<Vec<Point>> = Vec::new();  // ← 追加

for &il_idx in &face.inner_loops {
    let il = &solid.loops[il_idx];
    let il_3d = collect_loop_points(solid, il, segments, face_idx)?;
    if il_3d.len() < 3 {
        continue;
    }
    // ... UV 処理 ...
    hole_starts.push(flat_coords.len() / 2);
    for (u, v) in &il_uv {
        flat_coords.push(*u);
        flat_coords.push(*v);
    }
    inner_3d_added.push(il_3d);  // ← 追加: 実際に追加した loop の 3D 点を保存
}

// ②: all_points を inner_3d_added で構築
let all_points: Vec<Point> = outer_3d
    .into_iter()
    .chain(inner_3d_added.into_iter().flatten())  // ← 変更: face.inner_loops 再収集 → 保存済み点を使用
    .collect();
```

## 試した修正と結果
- Round 0: バグを発見（Codex review）

## 次にやること
1. `tessellate_trimmed_uv_face` の `inner_3d_added` 追加と `all_points` 修正
2. `cargo xtask ci` green 確認

## 追加で書いてほしいテスト
なし（既存テストが回帰を捕捉）
