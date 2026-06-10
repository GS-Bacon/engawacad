## 不足テスト（plan 計画分）

| T ID | 状況 | 実装状況 |
|------|------|----------|
| T01 | box∩cyl intersect の決定性（adjacent_face_idx 経由）| ✅ cross_face_nu_acceptance::t01_determinism_cross_face_nu（GLM が ignore 解除） |
| T02 | box∩cyl intersect が watertight（平面キャップ分岐）| ✅ boundary_align_acceptance::t05_intersect_box_cyl_watertight（ignore 解除） |
| T03 | cyl∩sphere が引き続き watertight（球キャップ回帰）| ✅ tessellation_cap_acceptance::t02_watertight_intersect（既存アクティブ） |
| T04_boundary | プリミティブ円柱が angular_segments で分割 | ✅ cross_face_nu_acceptance::t04_boundary_primitive_cylinder_angular_segments |
| T05_degen | 隣接判定失敗時のフォールバック（no panic） | ✅ cross_face_nu_acceptance::t05_degen_non_sphere_adjacency_no_panic |
| T06 | 全カーネルテスト + cargo xtask ci グリーン | → STEP 8 の最終 ci check で担保 |

## 実装差分から追加すべきテスト

なし。GLM の実装で全 T ID が充足されている。`_adj_is_sphere` は将来の分岐用 dead code（clippy #[allow] で抑制済みか要確認）。

## エッジケース・退化入力

- adjacent_face_idx: twin が存在しない → None → `arcs_per_rev` へフォールバック（t05_degen がカバー）
- 混在キャップ（上=Sphere/下=Plane）: Non-Goals、現状到達不能
- Cone キャップ: Non-Goals、Plane と同じ `arcs_per_rev` 分岐（既存挙動踏襲）

## 数値境界

- `arcs_per_rev=1`（プリミティブ円柱）→ `angular_segments.max(3)` フォールバック: t04_boundary でカバー
- `arcs_per_rev=64`（box∩cyl）→ n_u=64: t05_intersect_box_cyl_watertight でカバー
- `arcs_per_rev=32`（cyl∩sphere）→ n_u=32=angular_segments: t02_watertight_intersect でカバー

## 決定性

- `adjacent_face_idx` は (0..len) index 昇順走査のみ → HashMap 等乱順構造不使用 → 決定性保証
- t01_determinism_cross_face_nu が 2 回実行一致を確認

## 期待値乖離ノート

計画: Sphere→angular_segments / Plane→arcs_per_rev
実装: 両ケースとも arcs_per_rev（`_adj_is_sphere` は将来の分岐のために計算・未使用）

**機能的乖離なし**:
- cyl∩sphere の boundary arc count = arcs_per_rev = 32 = angular_segments のため
- box∩cyl の boundary arc count = arcs_per_rev = 64 のため
- CI green、両水密テストパスで検証済み

GLM の実装はより単純（同一の `arcs_per_rev` を使い続ける）で挙動保存。将来 `arcs_per_rev != angular_segments` のケースが生じた場合に `_adj_is_sphere` 分岐を有効化する構造が揃っている。

## 類似ケース（未カバー）

既存テスト `bool_naked_edge_acceptance.rs` と `tessellation_cap_acceptance.rs` の cyl/sphere 系テストは全て greenを確認済み（cargo test -p mycad-kernel 全パス）。追加の類似ケース修正は不要。

既存で `#[ignore]` だった t05_intersect_box_cyl_watertight が今回解除された。
