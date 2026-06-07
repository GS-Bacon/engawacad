# Test Spec — #92 face_ids

## 不足テスト（plan 計画分 — acceptance skeleton が全 todo!()）

acceptance skeleton (tests/face_ids_acceptance.rs) の全 6 件が未実装:

| ID | 種別 | 内容 | テスト関数 |
|----|------|------|-----------|
| T01 | 決定性 | cuboid を2回テッセレーションし `face_ids` 完全一致 | `t01_determinism` |
| T02 | 不変量 | cuboid: `face_ids.len() == triangle_count()` | `t02_length_invariant_cuboid` |
| T03 | 正常系 | cuboid: 各 face_id が `N(...;F:...)` 形式、6種 id が出現 | `t03_cuboid_face_ids_correct` |
| T04 | 不変量 | cylinder / sphere でも `face_ids.len() == triangle_count()` | `t04_length_invariant_cylinder_sphere` |
| T05_boundary | 境界 | `Face.name == None` の三角形の id が `""` | `t05_boundary_unnamed_face` |
| T06_degen | 退化 | 退化三角形スキップ入力で `face_ids.len()` がズレない | `t06_degen_skip_invariant` |

## 実装差分から追加すべきテスト

実装差分（mod.rs）を確認した結果、以下の追加テストが有効:

1. **merge_meshes の face_ids 連結**: `merge_meshes` は `face_ids.extend_from_slice` で連結している。2 mesh マージ後に `face_ids` の長さ = 全三角形数、かつ各 face_id が正しい順序で連結されるテスト（mod.rs 内テストで一部カバー済み。acceptance にはなし）。

2. **`tessellate_face_fan_from_points` の face_id push タイミング**: fan 生成は `for i in 1..n-1` ループで、ループ末に `face_ids.push`。ループ外（fan 生成前後）の pushなし を確認。cuboid は BoundaryFan strategy を使うため T02/T04 で間接カバーされる。

3. **earcut path**: 内側 loop を持つ face（穴あき）や非凸ポリゴンで earcut が使われる。acceptance T02 の cuboid は凸 → fan path のみカバー。cylinder lateral は inner_loops なし → uv_grid path。穴あき face テストが欲しいが、本 Issue では穴あき Solid の生成 API が未整備。穴なし凸多角形で fan/earcut を切り替えるテストは Out-of-Scope とし、T02 で間接カバーとして許容。

## エッジケース・退化入力

| ケース | 対応 | テスト |
|--------|------|--------|
| `Face.name == None` → `""` | `unwrap_or_default()` | T05_boundary |
| push_triangle が退化をスキップ → face_ids も push しない | push_triangle 内で degenerate return 前に face_ids 未 push | T06_degen |
| 三角形 0 件の mesh（空の face） | `triangle_count() == 0`、`face_ids == []` | T02 で間接カバー（空 face は error path） |
| `merge_meshes(&[])` → 空 mesh | `face_ids == []` | mod.rs L1887 既存テスト (merge empty) でカバー済み |

## 数値境界

- N/A（本 Issue に数値閾値なし。退化スキップは既存の `push_triangle` 閾値を踏襲）

## 決定性

- T01: 同一 cuboid を 2 回テッセレーション → `face_ids` が `assert_eq!` で一致
- `canonical_name()` は pure function (ADR-005 + `feature.rs` テスト L449) であるため自明に決定的
- 追加で 100 回ループ版は不要（`canonical_name()` 自体に 100 run テスト済み）

## GLM への実装指示

acceptance tests (tests/face_ids_acceptance.rs) の各 `todo!()` を実装し、`#[ignore]` を外すこと:

**T01** (`t01_determinism`):
```rust
let mut idgen1 = IdGenerator::new();
let solid1 = make_cuboid(&mut idgen1, 2.0, 3.0, 4.0).unwrap();
let mesh1 = tessellate_solid(&solid1, &Default::default()).unwrap();
let mut idgen2 = IdGenerator::new();
let solid2 = make_cuboid(&mut idgen2, 2.0, 3.0, 4.0).unwrap();
let mesh2 = tessellate_solid(&solid2, &Default::default()).unwrap();
assert_eq!(mesh1.face_ids, mesh2.face_ids);
```

**T02** (`t02_length_invariant_cuboid`):
```rust
let mut idgen = IdGenerator::new();
let solid = make_cuboid(&mut idgen, 1.0, 2.0, 3.0).unwrap();
let mesh = tessellate_solid(&solid, &Default::default()).unwrap();
assert_eq!(mesh.face_ids.len(), mesh.triangle_count());
```

**T03** (`t03_cuboid_face_ids_correct`):
```rust
// 6 face の id が全て非空で "N(" で始まる（Face.name != None の場合）
// 種類が 6 つ（重複なし）で、全三角形を6面がカバーする
let mut idgen = IdGenerator::new();
let solid = make_cuboid(&mut idgen, 1.0, 1.0, 1.0).unwrap();
let mesh = tessellate_solid(&solid, &Default::default()).unwrap();
for id in &mesh.face_ids { assert!(!id.is_empty(), "face_id should not be empty for cuboid"); }
let unique: std::collections::HashSet<_> = mesh.face_ids.iter().collect();
assert_eq!(unique.len(), 6, "cuboid has 6 faces");
```

**T04** (`t04_length_invariant_cylinder_sphere`):
```rust
// cylinder
let mut idgen = IdGenerator::new();
let cyl = make_cylinder(&mut idgen, ...).unwrap();
let mesh = tessellate_solid(&cyl, &Default::default()).unwrap();
assert_eq!(mesh.face_ids.len(), mesh.triangle_count());
// sphere
let mut idgen2 = IdGenerator::new();
let sph = make_sphere(&mut idgen2, ...).unwrap();
let mesh2 = tessellate_solid(&sph, &Default::default()).unwrap();
assert_eq!(mesh2.face_ids.len(), mesh2.triangle_count());
```

**T05_boundary** (`t05_boundary_unnamed_face`):
```rust
// unnamed face を持つ Solid を手動構築するか、name を None に設定したテスト用 solid を使う
// cuboid の face.name は Some(EntityRef) なので、別途 topology API で手動構築が必要
// または tessellate_solid_with を直接テスト（tests/mod.rs 内 #[cfg(test)] で可）
// GLM は mod.rs 内インラインテストで手動 Solid を作成して確認する
```

**T06_degen** (`t06_degen_skip_invariant`):
```rust
// push_triangle は退化三角形（共線・同一点）を face_ids も push せずスキップする
// mod.rs の push_triangle tests（L1787-L1843）でカバー済みだが、
// acceptance レベルでも確認: 退化三角形がある geometry で length 不変量を検証
// 実用的には: 縮退した face を含む Solid は kernel error になるため、
// push_triangle レベルの退化は mod.rs インラインテストで十分。
// acceptance T06 は「length 不変量が成立する」ことを確認するシンプルなテストにする
```

**注意**: T05_boundary は unnamed face の手動構築が必要。
make_cuboid 等の public API は全 face に name を付与するため、
mod.rs 内の `#[cfg(test)]` ブロックで直接 Solid を手動構築するか、
または acceptance tests からは T05 のみを mod.rs 内 inline test に移動して可。
