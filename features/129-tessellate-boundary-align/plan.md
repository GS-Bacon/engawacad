## In-Scope / Out-of-Scope

| In-Scope | Out-of-Scope |
|----------|--------------|
| `tessellate_face_uv_grid` の `u_min` を 0.0 ハードコードからループの seam 角度に変更し、boolean 後の円筒キャップ境界と側面境界の3D座標を一致させる | #3 トリム曲面テッセレーション（box_cut / cut_cylinder_hole / box_void / cut_sphere_dimple の三角形欠落）→ 別 Issue |
| `tessellate_face_earcut` の `same_sense=false` 巻き方向反転の不在を是正（副次バグ、naked edge の直接原因ではないが同ファイルの関連バグ） | 頂点溶接ポストプロセスの追加（測定上効果なし） |
| `t05_watertight_fuse`（`tessellation_cap_acceptance.rs:243`）の `#[ignore]` 解除 → GREEN 化 | boolean トポロジー自体の非 manifold（box_fuse 等、曲面なしモデル） |
| 修正範囲の類似ケース追加チェック（`tessellate_face_uv_grid` を呼ぶ全パスを確認） | viewer（レンダリング）側の改修 |

## Non-Goals

- #3 トリム曲面（inner_loop を持つ曲面・トリム円筒側面）のテッセレーション実装（GLM 1 サイクルを超える別機能）。
- 頂点溶接ポストプロセス（`#1` 同一座標重複は測定上不可視で実質無害）。
- B-rep トポロジー（Euler-Poincaré / validate_manifold）の変更。
- ε 境界近傍の境界値テスト（T03_boundary の `1e-9` は実装誤差の上界でなくテスト合否閾値。意図的にズレた入力を生成するテストは別 Issue 扱い）。

## 実装対象

- **Issue**: #129
- **影響クレート/ファイル**:
  - `crates/mycad-kernel/src/tessellation/mod.rs`（主要修正）
  - `crates/mycad-kernel/tests/tessellation_cap_acceptance.rs`（#[ignore] 解除 + 新テスト）
  - `crates/mycad-kernel/tests/boundary_align_acceptance.rs`（新規: T01/T03/T04）

### 修正 A: `tessellate_face_uv_grid` の u_min 算出

**根本原因**: `uv_of` は u を `(-π, π]` で返すため（`surface.rs:144-150`）、`corner_uvs.min(u)` は負になりうり seam 開始角を正しく表さない。その結果、`u_min = 0.0` とハードコードされている（`mod.rs:465`）。boolean 演算後に seam エッジが u≠0 の角度に移動した場合、UV グリッドは u=0 から始まるが、隣接キャップ（`collect_loop_points`）は seam 角から始まり、境界行の3D座標がズレる。

**fix の根拠**: `Curve::Circle::evaluate(t)` と `Surface::Cylinder::evaluate(u,v)` は同一の `orthonormal_basis(axis)` を使用するため、circle の t パラメータ = cylinder の u パラメータ（`curve.rs:22-34` と `surface.rs:60-67` 参照）。したがって seam 開始角 u_min は、外ループの circle edge の t_range 開始値から取得できる。

**BEFORE** (`mod.rs:465`):
```rust
let u_min = 0.0_f64;
```

**AFTER** (`mod.rs:465` 付近、`corner_uvs` 計算の後):
```rust
// Determine the seam starting angle from the first circle edge in the outer loop.
// Curve::Circle and Surface::Cylinder share the same orthonormal_basis(axis),
// so circle-edge t-parameter equals cylinder-surface u-parameter.
// After boolean operations the seam may be at an angle ≠ 0; use the actual loop
// boundary to align the UV grid with adjacent face sampling.
let u_min = outer_loop
    .half_edges
    .iter()
    .find_map(|&he_idx| {
        let he = &solid.half_edges[he_idx];
        let edge = &solid.edges[he.edge];
        if matches!(edge.curve, Curve::Circle { .. }) {
            Some(if he.forward {
                edge.t_range[0]
            } else {
                edge.t_range[1]
            })
        } else {
            None
        }
    })
    .unwrap_or(0.0_f64);
```

> ⚠ GLM 確認事項: 外ループに複数の circle edge がある場合（boolean で seam が分割されたケース）、最初の circle edge の t_range 開始値が全体の開始角と一致するか。`total_circle_span` チェック（`mod.rs:431-449`）が既に保証する全周 2π に対して、円弧列の先頭が loop の順序で正しく並んでいるかを確認すること。

### 修正 B: `tessellate_face_earcut` の same_sense=false 巻き方向反転（副次バグ）

**根本原因**: `tessellate_face_earcut`（`mod.rs:276-357`）には `same_sense=false` に対する winding flip がない。`tessellate_face_fan_from_points`（`mod.rs:167-223`）の `flip` 処理に相当するものが earcut には欠落。naked edge の直接原因ではないが、boolean 後の平面ホール面（`same_sense=false`）で法線が内向きになる視覚バグを引き起こす。

**BEFORE** (`mod.rs:350-355` の triangle push):
```rust
for chunk in indices.chunks(3) {
    mesh.indices.push(base_idx + chunk[0] as u32);
    mesh.indices.push(base_idx + chunk[1] as u32);
    mesh.indices.push(base_idx + chunk[2] as u32);
    mesh.face_ids.push(face_id.to_string());
}
```

**AFTER**:
```rust
for chunk in indices.chunks(3) {
    let (a, b, c) = if face.same_sense {
        (chunk[0], chunk[1], chunk[2])
    } else {
        (chunk[0], chunk[2], chunk[1])   // flip winding for reversed face
    };
    mesh.indices.push(base_idx + a as u32);
    mesh.indices.push(base_idx + b as u32);
    mesh.indices.push(base_idx + c as u32);
    mesh.face_ids.push(face_id.to_string());
}
```

> ⚠ GLM 確認事項: earcutr が出力する三角形が常に CCW（2D 投影で）と仮定しているが、outer_loop の点が CW で与えられる場合は既に CW 三角形が出る可能性がある。事前に `is_polygon_convex`（`mod.rs:226-273`）で使っている投影軸（`u_idx, v_idx`）と法線の向きを確認し、flip の方向が正しいかを確認。また inner_loop（hole）がある場合の earcutr の巻き順仕様も確認すること。

## 設計方針

- **決定性**: `orthonormal_basis` は引数ベクトルのみに依存する決定的関数（`math.rs:50-61`）。修正 A は `edge.t_range[0/1]` から u_min を取り出すだけで新規 random 要素なし。IdGenerator 不使用。
- **B-rep トポロジー妥当性**: テッセレーション層のみの修正。B-rep 構造（Vertex/Edge/Face/Loop）は変更しない。Euler-Poincaré は影響なし。
- **退化幾何**: `push_triangle` の面積フィルター（`mod.rs:881-897`）は引き続き有効。修正 A で u_min が変わっても退化三角形は既存コードで弾かれる。
- **derive 規約**: 新規型なし。既存 `TriangleMesh` の `Debug/Clone/Serialize/Deserialize` に変更なし。
- **エラーハンドリング**: circle edge が見つからない場合は `unwrap_or(0.0)` で現状の挙動にフォールバック（素プリミティブは u_min=0 で正しい）。

### 数値モデル

- **ε_snap / ε_len の新規定義なし**: 本修正はサンプリング起点を揃える構造変更であり、tolerance を用いた点の同一判定や退化判定を新たに導入しない。既存の `LENGTH_TOLERANCE`（`geometry/math.rs` 定義済み）に依存する部分は変更しない。ADR-004 既定義の epsilon をそのまま使用する。
- `ε_match = 1e-6`（`assert_watertight_welded` の許容値）: 修正後の境界点が隣接面の境界点と 1e-6 以内で一致することを要件とする。修正 A は同一の数式評価パスを通るため、理想的には差分 = 機械精度（< 1e-14）。
- `ε_direct = 1e-9`（T03_boundary テストの合否閾値）: 同一評価式の浮動小数点丸め誤差上界。境界値テスト（ε 近傍の意図的ズレ入力）は本 Issue のスコープ外（Non-Goals に記載）。
- ADR-004 準拠: 内部計算は exact（特別な tolerance 判定なし）。サンプリング点の一致は等価な評価式を使うことで保証。

## テスト計画（ID 付き）

| ID | 種別 | 内容 | 期待結果 |
|----|------|------|----------|
| T01 | 決定性 | `boolean(box, cyl, Fuse)` を2回テッセレーションし positions/normals/indices が完全一致 | `assert_eq!` |
| T02 | 回帰・主目標（既存解除） | `t05_watertight_fuse` の `#[ignore]` を外す: box(10³)∪cyl(r2,h15,z=-7.5) で `assert_watertight_welded(.., 1e-6)` | naked edge = 0, GREEN |
| T03_boundary | 境界整合直接アサート | 円筒側面テッセレーションの v_min/v_max 行の各点が隣接キャップ面の境界点と 1e-9 以内で一致 | `assert!(dist < 1e-9)` |
| T04 | 退化なし | fuse 結果メッシュで positions/normals 有限・triangle_count>0・signed_volume 有限非ゼロ | 各 assert! |
| T05_intersect | 類似ケース追加 | `boolean(box, cyl, Intersect)` で `assert_watertight_welded(.., 1e-6)` — 修正 A が intersect 系にも効くか確認 | naked edge = 0 |

（`t04_watertight_cut_hole` / `t04_boundary_degen_cut_cyl` は **据え置き**: #3 トリム曲面が原因で今回の修正範囲外）

## 幾何的不変条件チェックリスト

- [x] partition 出力の polygon 頂点順と assemble の normal 処理が整合しているか — N/A（テッセレーション層のみ、partition/assemble は変更しない）
- [x] 各プリミティブの face ごとの outer_loop 2D 向き（CW/CCW）が文書化されているか — N/A（既存文書化なし、本 Issue では変更不要）
- [x] flip_normals / same_sense の意味論が明確か — 修正 B で earcut の same_sense=false winding flip を追加（既存 fan 実装に揃える）
- [x] pslg_subdivide の出力向きが元の outer_loop 向きと整合しているか — N/A（pslg は booleans 層、本 Issue は tessellation 層のみ）
