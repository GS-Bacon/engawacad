# Test Spec — #137 trimmed sphere tessellation circ_normal 一般化

STEP 6 (GLM core) で T01/T02 は実装完了。残り 6 テストの実装仕様。

## 不足テスト（plan 計画分）

### T03 `t03_arbitrary_axis_circ_normal`

任意軸 `circ_normal` を持つ trim circle で `tessellate_sphere_face_trimmed` が正しく動作することを検証。

**セットアップ**:
- `make_sphere(radius=5.0, center=Origin)` で原点中心 R=5 の球を作る
- `Curve::Circle { center: Point::new(0.0, 0.0, 0.0), normal: Vec3::new(1.0, 0.0, 0.0), radius: <X> }` のエッジを 1 inner_loop として持つ Face を直接構築 (`Surface::Sphere`)
  - circ_normal が +X 方向 → 軸 = +X
  - 例えば signed_offset = 3.0 (球中心からエッジまで X 方向 3.0) で circ_radius = sqrt(R² - 3²) = 4.0
- inner_loop の HalfEdge を 1 本構築 (test 用 minimal — circle full sweep)

**assertion**:
- `tessellate_sphere_face_trimmed` が `Ok(())` で帰る (`InvalidTrimCircle` でない)
- 生成された頂点 (mesh.positions[base..]) が全て球面上: `|position - sphere_center| ∈ [R - LENGTH_TOLERANCE, R + LENGTH_TOLERANCE]`
- pole 頂点 (最後の頂点) が circ_normal 方向に配置: `position = sphere_center ± R * circ_normal.normalize()`
- triangle 数 > 0

**ヒント**: 直接 Face を構築する例は `crates/mycad-kernel/src/primitives/sphere.rs` の make_sphere 内コードを参考に。

### T_boundary_tangent_circle `t_boundary_tangent_circle_degenerate_mesh` (※関数名で確定挙動を明示)

接円 (`signed_offset == sphere_radius`) ケース。STEP 6 の実装挙動を観察し、以下のどちらを採用したかで関数名を確定:

**挙動 (a)**: 縮退三角形 0 件のメッシュ
- 関数名: `t_boundary_tangent_circle_produces_empty_mesh`
- assertion: `tessellate_sphere_face_trimmed` が `Ok(())`、`mesh.triangle_count() == 0` または極のみ 1 頂点

**挙動 (b)**: reject
- 関数名: `t_boundary_tangent_circle_rejects_as_unsupported`
- assertion: `Err(TessellationError::TrimmedFaceUnsupported)` または `InvalidTrimCircle`

実装が clamp(-1, 1) を通過して v_lat = ±π/2 になるため (a) になる可能性が高い。確認して関数名を確定。

### T_boundary_great_circle `t_boundary_great_circle_axis_z`

大円 (`signed_offset == 0`、circ_radius == sphere_radius) で半球メッシュ。

**セットアップ**:
- R=5 sphere、circ_center = sphere_center (signed_offset = 0)
- circ_normal = +Z

**assertion**:
- `Ok(())` で帰る
- mesh.positions 全頂点が球面上
- pole は (0, 0, ±R) (trim_lower で決まる side)
- triangle 数が「半球らしい」値: 概ね `n_u * n_v / 2` 以上 (厳密値は実装依存だが下限 assert)

### T_degen_out_of_sphere `t_degen_signed_offset_exceeds_radius_returns_error`

球外円ケース。

**セットアップ**:
- R=5 sphere、center origin
- circ_center = (10, 0, 0) (signed_offset = 10 > 5 + ε)
- circ_normal = +X
- circ_radius = 任意 (実際は球と接しないので意味なし)

**assertion**:
```rust
let result = tessellate_sphere_face_trimmed(...);
assert!(matches!(result, Err(TessellationError::InvalidTrimCircle { .. })));
```

### T_degen_zero_axis `t_degen_zero_circ_normal_returns_error`

退化 axis ケース (circ_normal がゼロベクトル)。

**セットアップ**:
- R=5 sphere
- circ_normal = `Vec3::zeros()` の Curve::Circle

**assertion**:
- `Err(TessellationError::InvalidTrimCircle { .. })`
- signed_offset フィールドは NaN または 0 (実装どちらでも可、本テストは error variant を check するのみ)

### T04 `t04_shared_boundary_with_cyl_lateral`

`boolean_cut_sphere_dimple` の Solid 内で、trimmed sphere face と隣接 cyl lateral face の共有境界 ring 頂点を直接比較。

**セットアップ** (T02 と同じ):
```rust
let mut gen = IdGenerator::new(0);
let box_solid = make_cuboid(10.0, 10.0, 10.0, &mut gen).expect("cuboid");
let sphere = make_sphere(3.0, Point::new(0.0, 0.0, 6.0), &mut gen).expect("sphere");
let result = boolean(&box_solid, &sphere, BooleanOp::Cut, &mut gen).expect("cut");
let mesh = tessellate_solid(&result).expect("tessellate");
```

**手順**:
1. `result.faces` から `Surface::Sphere` 型の trimmed face を探す (inner_loops 非空)
2. その face の inner_loop の `half_edges[0]` を `he_a` とする
3. `he_a.twin` で隣接面の HalfEdge `he_b` を取得
4. `he_b` を含む face を取得 (隣接 cyl/plane face)
5. tessellate 結果の mesh.positions / mesh.face_ids (もし face id がある) から両 face の境界 ring 頂点を抽出
6. index 順に position を比較し、全頂点で `(p_a - p_b).norm() <= LENGTH_TOLERANCE`

**注意**: TriangleMesh が face id 付き情報を持たない場合は、boundary edge を辿る方法を検討する。`TriangleMesh::face_ids` フィールドが存在するか確認。

**assertion**:
```rust
for k in 0..n_u {
    assert!((boundary_ring_sphere[k] - boundary_ring_cyl[k]).norm() <= LENGTH_TOLERANCE,
        "shared boundary mismatch at index {k}");
}
```

face_ids やインデックス取得方法が複雑な場合、シンプルなバリエーション:
- `count_naked_edges(&mesh, LENGTH_TOLERANCE) == 0` を共有境界整合の代理 assertion とする (より弱いが汎用)。

## 実装差分から追加すべきテスト

GLM 実装 (mod.rs:1014-1059) のレビューから:

- **追加テスト案**: `t_invariant_normal_consistency_with_evaluate`
  - eval_axis(u, v) の normal_axis(u, v) が `(eval_axis(u, v) - sphere_center).normalize()` と一致することを単体的に検証 (法線の内部整合性)
  - これは plan のテストリストにない invariant test だが、改修時の guard として有用

省略可。GLM が判断。

## エッジケース・退化入力

- circ_normal が ±Z 以外 (T03 でカバー)
- signed_offset が球外 (T_degen_out_of_sphere)
- circ_normal がゼロ (T_degen_zero_axis)
- 接円 / 大円 (T_boundary_*)

すべて plan 内でカバー済み。

## 数値境界

- `signed_offset == sphere_radius + LENGTH_TOLERANCE` ちょうど → 許容 (接円扱い)
- `signed_offset == sphere_radius + LENGTH_TOLERANCE * 2` → エラー (T_degen_out_of_sphere の派生)

## 決定性

T01 が covered。他テストでは決定性 assert は省略可 (T01 が代表)。

## まとめ

GLM STEP 6.6 が実装すべきは 6 関数:
1. `t03_arbitrary_axis_circ_normal` — 任意軸機能テスト
2. `t_boundary_tangent_circle_*` — 接円挙動確定 (関数名は実装挙動次第)
3. `t_boundary_great_circle_axis_z` — 大円
4. `t_degen_out_of_sphere` (リネーム: `t_degen_signed_offset_exceeds_radius_returns_error`)
5. `t_degen_zero_axis` (リネーム: `t_degen_zero_circ_normal_returns_error`)
6. `t04_shared_boundary_with_cyl_lateral`

すべて `#[ignore]` を外して実装する。
