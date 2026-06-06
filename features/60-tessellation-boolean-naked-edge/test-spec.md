# test-spec.md — Issue #60

## 不足テスト（plan 計画分）

全 T ID が実装済み:

| ID | 関数名 | 実装状況 |
|----|--------|---------|
| T01_determinism | `t01_determinism` | ✅ `#[ignore]` 解除済み |
| T02_box_sphere_void_naked_edge | `t02_box_sphere_void_naked_edge` | ✅ `#[ignore]` 解除済み |
| T03_cyl_sph_intersect_naked_edge | `t03_cyl_sph_intersect_naked_edge` | ✅ `#[ignore]` 解除済み |
| T04_boundary_degen_cut_cyl | `t04_boundary_degen_cut_cyl` | ✅ `#[ignore = "known seam mismatch"]` |

## 実装差分から追加すべきテスト

`count_naked_edges` は union-find ベースの位置ウェルディングを使う。以下のケースを追加してもよい:

- `t05_box_sphere_fuse_naked_edge`: box ∪ sphere の naked_edge = 0（現状は実装なし）
- `t06_shallow_dimple_naked_edge`: box − sphere(center=(0,0,8)) の naked_edge = 0

ただし、本 Issue のスコープ内で必須ではない（GLM がステップ6.6で判断して追加可）。

## エッジケース・退化入力

- box − cylinder は seam mismatch で naked_edge > 0（T04: ignored）
- 空メッシュ（triangle_count = 0）の場合 count_naked_edges = 0 になることを確認

## 数値境界

- `eps = 1e-10` での頂点ウェルディング — テッセレーション精度（1e-10 以下のズレ）で隣接頂点が正しくウェルドされる

## 決定性

- T01 で 2 回実行の mesh が byte-identical であることを確認済み
