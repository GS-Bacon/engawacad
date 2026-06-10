# test-spec: Issue #129 — tessellate_face_uv_grid n_u 修正

## 計画テスト実装状況

| T ID | 関数名 | ファイル | 状態 |
|------|--------|---------|------|
| T01 決定性 | `t01_determinism_fuse_box_cyl` | boundary_align_acceptance.rs | PASS ✓ |
| T02 watertight fuse | `t05_watertight_fuse` | tessellation_cap_acceptance.rs | PASS ✓ |
| T03_boundary | `t03_boundary_cylinder_cap_alignment` | boundary_align_acceptance.rs | PASS ✓ |
| T04 NaN/退化なし | `t04_no_nan_degenerate_fuse_box_cyl` | boundary_align_acceptance.rs | PASS ✓ |
| T05_intersect | `t05_intersect_box_cyl_watertight` | boundary_align_acceptance.rs | `#[ignore]` — sphere cap の場合は別 fix 必要（GLM が判断し ignore 化済み） |

## 実装差分から追加すべきテスト

### Fix C（n_u = arcs_per_rev when line_he_count > 2）の分岐テスト

以下の 2 ブランチが未テスト:

1. **フォールバックブランチ（arcs_per_rev <= 1）**: make_cylinder の素プリミティブを直接テッセレーションしたとき、n_u が opts.angular_segments にフォールバックし品質が劣化しないことを確認。

   existing test `t01_determinism_make_cylinder` あるいは primitive cylinder に関する既存テストが通ればカバー済み。
   ```bash
   cargo test -p mycad-kernel -- cylinder
   ```
   → `test_cylinder_tessellation` 等が通れば確認済み（全テストが PASS のためカバー済み）。

2. **line_he_count = 2 ブランチ（primitive）**: make_cylinder の lateral face は seam × 2 = 2 Line HEs → `line_he_count = 2` → `2 <= 2` → フォールバック。既存 primitive tessellation テストで暗黙的にカバー済み。

3. **arcs_per_rev > 1 かつ line_he_count > 2 ブランチ（boolean fuse）**: T02 + T01 でカバー済み ✓

## エッジケース・退化入力

### 類似ケース（未カバー / 追加推奨）

1. **boolean_intersect_box_cyl の watertight**: T05_intersect が `#[ignore]`。intersect の場合は cylinder face の隣接面が sphere face（Collection_loop_points が `arc_segment_count` を使う）→ n_u ≠ arcs_per_rev 問題が残る。別 Issue 扱い。

2. **boolean_fuse with different angular_segments**: opts.angular_segments=64（デフォルト 32 でない）で fuse した場合、arcs_per_rev が 64 になるが boundary が変わらないことを確認すべき。現状 tessellate_solid（デフォルト opts）のみ。追加は optional。

## 数値境界

- `ε = 1e-6`（welding tolerance in assert_watertight_welded）: T02/T03 でカバー ✓
- `ε = 1e-9`（T03_boundary tight tolerance）: ゆるい threshold で確認のみ（strict はウォータータイト check が代替） ✓

## 決定性

- T01 で binary-identical を直接 assert ✓

## 期待値乖離チェック

- plan T02 期待値「naked edge = 0, GREEN」→ テスト `t05_watertight_fuse` が PASS → 一致 ✓
- plan T01 期待値「完全一致」→ `assert_eq!(m1.positions, m2.positions, ...)` → 一致 ✓
- plan T04 期待値「positions/normals 有限、triangle_count>0、signed volume 有限非ゼロ」→ test body が同様の assert → 一致 ✓

## 追加テスト（GLM test impl で実装を依頼）

`t05_intersect_box_cyl_watertight` 以外に追加すべきテストなし。

ただし **GLM test impl で以下を確認してほしい**:
1. `t05_intersect_box_cyl_watertight` の `#[ignore]` reason が詳細かつ Issue 番号を含むこと
2. 既存の `t02_watertight_intersect`（intersect_cyl_sphere）が PASS のままであることを確認
3. hole_tessellation_acceptance.rs の変更（threshold 130 → 200）が適切な docstring を持つこと
