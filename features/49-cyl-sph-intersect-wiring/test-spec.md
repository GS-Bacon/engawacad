# test-spec.md — Issue #49 Cyl×Sph Intersect

## 不足テスト（plan 計画分 — T01-T07、全件 todo!() のまま）

| ID | 関数名 | 実装内容 |
|----|--------|---------|
| T01 | `t01_determinism` | 同一 `cyl_sph_features()` を `IdGenerator::new(0)` で 2 回 build → `assert_eq!(solid1, solid2)` |
| T02 | `t02_manifold` | `build_intersect().validate_manifold().expect("manifold")` |
| T03 | `t03_two_circle_edges_at_z_pm4` | 結果 solid の edges を走査し `Curve::Circle { radius } if (radius - 3.0).abs() < 1e-6` が 2 本かつ center.z が ±4.0 付近（`abs < 1e-6`）を確認 |
| T04 | `t04_euler_poincare` | `solid.compute_euler()` を手計算または直接 V−E+F で 2 の近傍（`==2 || ==1`）をアサート |
| T05 | `t05_closed_shell_three_faces` | faces.len()==3（cyl lateral 1 + sphere caps 2）、shells.len()==1 |
| T06 | `t06_non_coaxial_errors` | sphere center=(1.0,0.0,0.0) で Intersect → `build_bodies_from_features(...).is_err()` |
| T07 | `t07_tangent_no_panic` | sph_r==cyl_r==3.0 で Intersect → panic しない（`build_...` が ok か err かは問わない） |

## 実装差分から追加すべきテスト
実装を読んで plan 計画外で生じた分岐:
- `circle_vs.is_empty()` パス（交線なし → pass-through）: T07 が接線ケースでこのパスを通る想定。
- `cyl_sph_sphere_processed` / `cyl_sph_cyl_target_processed` HashSet による二重生成防止: T05（face 数=3）で間接検証。
- target ループと tool ループで cyl×sph の role が逆転する対称性: A2 テスト（T02-T05）が target=cyl/tool=sph で通れば主要パスはカバー。逆順（target=sph/tool=cyl）は今 Issue の Out-of-Scope ではないが、将来の hardening（#43）で確認予定。

## エッジケース・退化入力
- 非同軸（T06）: center=(1,0,0) で `UnsupportedSurfaceIntersection` が伝播すること。
- tangent（T07）: sph_r==cyl_r で `Ok(vec![])` が返り panic なし。
- cyl が球を完全に内包するケース（cyl_r > sph_r）は T07 と同様に交線なし → pass-through 扱い。hardening は #43。

## 数値境界
- 交線 z=±4.0 の許容誤差: `abs < 1e-6`（`LENGTH_TOLERANCE=1e-9` よりも余裕を持たせる）。
- circle radius=3.0 の許容誤差: `abs < 1e-6`。

## 決定性
T01: `IdGenerator::new(0)` で 2 回 build → full `assert_eq!`（PartialEq が実装されていない場合は edge count・face count・vertex count の一致で代替）。
