# test-spec.md — Issue #143

## 不足テスト (plan 計画分)

全 ID 実装済み (`crates/mycad-kernel/tests/cyl_lateral_deadbranch_acceptance.rs`):
- T01 `t01_determinism_cyl_sphere_intersect` ✓
- T02 `t02_primitive_cylinder_uses_angular_segments` ✓
- T03 `t03_cyl_sphere_intersect_no_naked_edge` ✓
- T04 `t04_cyl_cuboid_intersect_no_naked_edge` ✓
- T_DEG `t_deg_boundary_primitive_cyl_min_angular_segments` ✓

## 実装差分から追加すべきテスト

該当なし — 本 Issue は純粋なクリーンアップ (死に分岐削除 + コメント簡潔化 + `#[allow(dead_code)]` 付与) であり、新規分岐・ケースは生まれていない。

## エッジケース・退化入力

- T_DEG: `TessellationOptions::new(1, 2)` のように `angular_segments = 1` を指定しても `.max(3)` で 3 に補正される境界ケースをカバー済み

## 数値境界

該当なし (本 Issue は数値モデルセクションなし、tolerance は変更なし)

## 決定性

T01 がカバー (cyl∩sphere intersect の tessellation を 2 回実行し、positions / normals / indices の bit-identical を verify)

## 期待値表記差 (乖離ではなく精緻化)

### T02 — `t02_primitive_cylinder_uses_angular_segments`

| 観点 | plan | 実装 |
|---|---|---|
| 主旨 | `n_u == angular_segments` 経路を通る | 同 |
| 検証手法 | 頂点数 = `(angular+1)*(axial+1)` (lateral のみの表記) | 頂点数 = `(angular+1)*(axial+1) + 2*angular_segments` (lateral + bottom cap + top cap) |
| 実装側コメント根拠 | — | コメント 127-130 行に内訳明記 (lateral 51 + bottom 16 + top 16 = 83) |

**判定**: `tessellate_solid` は solid 全体 (lateral + 2 caps) のメッシュを返す。plan の T02 期待値表記は lateral 限定で書いており、caps の頂点を含む全体表記に実装側で補完した。**両者は値こそ異なるが「primitive cylinder fallback で n_u==angular_segments の経路を通る」という同じ主旨を別レベルで検証している**。実装の方が包括的で誤検出が少ない (lateral だけでは caps の整合性が見えない)。乖離ではなく検証手法の精緻化と判定し、**実装を採用**する。

plan 側の表記は test-spec.md の本記録で訂正されたとみなす (plan.md の T02 行は historical 価値があるため改変しない)。

## 実装の整合性確認

- `cargo xtask ci` green (GLM コア実装で確認済み)
- 既存 cylinder/Boolean テスト全 pass (回帰なし)
- 既存 inline テスト `test_adjacent_face_idx_sphere_cap` / `_plane_cap` は `#[allow(dead_code)]` 付与後も pass し続ける (`adjacent_face_idx` 関数本体は変更なし)
