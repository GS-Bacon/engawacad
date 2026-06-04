# test-spec.md — Issue #43

## 不足テスト（plan 計画分）

計画の T18/T34/T22b/T23 は GLM (STEP 6) で全て実装・`#[ignore]` 解除済み。不足なし。

| ID | 実装先 | 状態 |
|----|--------|------|
| T18 | `crates/mycad-build/tests/cyl_sph_intersect_acceptance.rs::t18_tessellate_triangle_count` | 実装済み（#[ignore] 解除） |
| T34 | `crates/mycad-kernel/tests/cyl_sph_surface_intersect.rs::t34_intersect_surfaces_loop_geometry` | 実装済み（#[ignore] 解除） |
| T22b | `crates/mycad-build/tests/cyl_sph_intersect_acceptance.rs::t22b_determinism_across_tessellation_resolution` | 実装済み（#[ignore] 解除） |
| T23 | `crates/mycad-build/tests/cyl_sph_intersect_acceptance.rs::t23_yaml_round_trip_example` | 実装済み（#[ignore] 解除） |

## 実装差分から追加すべきテスト

GLM が実装した T23 は YAML round-trip の idempotency (yaml1 == yaml2) に加えて、
example ファイルのフィーチャー内容（cylinder/sphere/intersect の id/radius/height/origin）も
assertion している。plan 計画より検証が充実しており、追加テストは不要。

## エッジケース・退化入力

plan の Non-Goals 通り、以下は本 Issue 対象外（#49 でカバー済みか、MVP 外）:
- 非軸整列 cyl×sph: t06_non_coaxial_errors (#49) でエラー伝播確認済み
- 接線ケース (r_sph == r_cyl): t07_tangent_no_panic (#49) でパニックなし確認済み
- r_sq < 0（交差なし）: `intersect_surfaces` が `Ok(vec![])` を返す。本 Issue ではカバー不要（幾何コア変更なし）

新たに追加すべきエッジケーステストなし。

## 数値境界

- T34 の tolerance: `LENGTH_TOLERANCE = 1e-9`（ADR-004 準拠）
- T22b の tessellation: angular_segments 8（最小有意） / 64（標準の2倍）
- T23: `yaml1 == yaml2` は文字列完全一致（floating-point 表現は serializer が決定論的に出力）

## 決定性

- T01 (#49 実装済み): `IdGenerator::new(0)` で2回ビルド → 全 id・座標一致
- T22b (#43 本 Issue): 同一 features を2回ビルド → `assert_solids_equal_with_names` で名前込み一致
  - `angular_segments` は `.mycad` schema に存在せず tessellation 時のみ作用。Solid トポロジーは解像度非依存であることを実証。
