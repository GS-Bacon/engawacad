# Test Spec — Issue #144

## 不足テスト（plan 計画分）

なし。plan の T01-T06 は全件実装されており、全件 pass している:

| T ID | 関数名 | 状態 |
|---|---|---|
| T01 | `t01_determinism` | passed (Cut fixture 2 run → byte-identical) |
| T02 | `t02_cut_box_sphere_shared_boundary` | passed (Circle edge 検査 ≥ 1) |
| T03 | `t03_intersect_cyl_sphere_shared_boundary` | passed (Circle edge 検査 ≥ 1) |
| T04 | `t04_fuse_box_cyl_shared_boundary` | passed (Circle edge 検査 ≥ 1) |
| T05 | `t05_degen_self_adjacent_seam_no_panic` | passed (戻り値 = 0) |
| T06 | `t06_boundary_pure_cuboid_no_circle_edges` | passed (戻り値 = 0) |

## 実装差分から追加すべきテスト

実装過程で発見された design refinement (`vertices_on_circle_sorted` の seam dedup) が plan に反映済み。本機能の retgression を別途検証するテストを 1 件追加すべき:

### TX01 — seam vertex dedup の正常動作確認

UV グリッド面 (e.g. cylinder lateral) の seam vertex を 2 回ストアしても、`vertices_on_circle_sorted` が 1 件に dedup することの直接検証。

- 入力: `make_box_fuse_cyl` の特定 Circle edge で、cyl lateral 側のサンプル数を取得
- 期待: dedup 後の長さが cap 側 (boundary fan) の長さと一致
- 既に T04 で間接的にカバーされているため新規テストは不要 (T04 が dedup 動作の上に成立)

→ **追加テスト不要**: T04 が seam dedup の正常動作を間接検証している。

## エッジケース・退化入力

| 観点 | 既存カバレッジ | 追加要否 |
|---|---|---|
| seam edge (self-adjacent) | T05 で sphere 単体を検証 | 不要 |
| Circle edge ゼロ件 | T06 で cuboid 単体を検証 | 不要 |
| サンプル数不一致時の panic メッセージ | 仕様化済み (plan §「失敗時診断メッセージ仕様」)、現実装で発火条件なし | 不要 (ADR-009 実装本体時に自然検証) |
| `Curve::Circle.normal` 非単位の取扱い | `project_to_circle` で `normal.normalize()` 後 `orthonormal_basis` 呼び出し | 不要 (実装内で対応済み) |

## 数値境界

| 観点 | 検証 |
|---|---|
| LENGTH_TOLERANCE (1e-9 mm) の境界 | T03/T04 で実際の Boolean fixture が bitwise 一致レベルで pass しており、境界探索は不要 |
| 円周期 `[0, 2π]` の境界 | `[t_start, t_start + 2π)` 正規化で seam 頂点が collision、`vertices_on_circle_sorted` で位置 dedup により安全 |
| `atan2` 不連続 (±π) | 正規化で吸収 (前述) |

## 決定性

T01 で `make_box_cut_sphere` を 2 回ビルドして positions/normals/indices が byte-identical を assert。既存パターン (`bool_naked_edge_acceptance::t01_determinism`, `boundary_align_acceptance::t01_determinism_fuse_box_cyl` 等) に倣う。

決定性 100 回 (e.g. `t06_determinism_100_runs` 相当) は本 Issue では Out-of-Scope (既存テストで十分検証されている)。

## 期待値乖離

なし。実装中に判明した seam dedup の必要性は plan に追記済み (主要リスクと対応表に「UV グリッド面の seam 重複」項を追加)。fixture / テスト計画 / 数値モデル / 失敗時メッセージ仕様の plan 記述と実装は一致。

## 類似ケース追加チェック

本 Issue は `type: foundation` (bug ラベルなし) のため類似バグケース探索は対象外 (STEP 6.5 手順 3.5 はバグ修正 Issue のみ)。
