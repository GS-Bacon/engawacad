# Test Spec — Issue #50 box − sphere (TrimmedFaceUnsupported)

## 不足テスト（plan 計画分）

plan の T01〜T07 はすべて実装済み:
- T01 決定性: `box_sphere_void_acceptance.rs::t01_determinism` — mesh + seam Circle 一致 ✓
- T02 回帰: `t02_regression_tessellate_ok` ✓
- T03 watertight: `t03_watertight` (directed edge quantize 法) ✓
- T04 void 法線内向き: `t04_void_normals_inward` ✓
- T05 符号付き体積: `t05_signed_volume` — `vol > expected` かつ `diff < 8.0` ✓
- T06 トポロジー: `t06_topology` — face 数 7, same_sense=false, seam Circle{-Y}, span=π ✓
- T07 example smoke: `examples_smoke.rs::boolean_cut_sphere_dimple_tessellate` ✓

不足テストなし。

## 実装差分から追加すべきテスト

partition.rs の変更 (`seam_curves` carry) は 1 箇所のみで副作用の余地が狭い。
ただし以下の追加テストが有益:

- **EC01 既存 boolean パスの非退行**: `boolean_cut_sphere_dimple_tessellate` が pass することで確認済み。
  加えて `cargo xtask ci` で全テストが green であることを確認済み。

- **EC02 sphere void 面の Euler-Poincaré**: void 球面 shell の V-E+F=2 (V=2,E=1,F=1) が成立すること。
  → T06 で face 数・seam 構造を確認しているため暗黙的に成立。明示テストは GLM に委譲。

## エッジケース・退化入力

| ケース | 観察 | 対応 |
|--------|------|------|
| sphere origin = box corner 付近 | sphere が部分的に箱外に出ると intersect あり → degenerate 分岐を通らない。本 fix の対象外。 | 既存テスト group でカバー |
| sphere radius > box half-size | sphere が箱を完全に内包 → 別 Boolean path (target 内包 tool ではなく tool が target を内包)。本 fix 対象外。 | 別 Issue |
| sphere radius = 0 | make_sphere が KernelError を返す。boolean 前にエラー。 | make_sphere テストで既にカバー |

## 数値境界

- T05 の `< 8.0` 閾値は n_u=16 (デフォルト `angular_segments`) での完全球 faceting 誤差 ≈ 4〜6 を想定。
  より粗い設定では増大する可能性があるが、デフォルト設定でのみ保証する。

## 決定性

T01 で直接検証済み（mesh 全体 + seam Circle パラメータ）。`IdGenerator::new(0)` 固定で 2 回実行が完全一致。
