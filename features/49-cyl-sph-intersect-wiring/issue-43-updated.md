# タイトル: Cylinder×Sphere Boolean A3: 同軸 Intersect 統合テスト hardening (Phase 4 #34 sub-step)

## 現在の Phase と完了条件への寄与

**現在の active Phase は Phase 4**。完了条件: `Cut / Fuse / Intersect が .mycad から動作する`。

本 Issue は「Cylinder × Sphere 同軸 Intersect のパイプラインが動作することを T18/T20/T33/T34/T22b/T23 で hardening 検証する」。

> **blocked**: 実装 Issue #49（partition trim/エラー伝播 配線）が land するまで着手不可。

## 概要

`intersect_cylinder_sphere` の幾何コア（`surface_intersect.rs:251-337`）は実装済み、かつ #49 で partition skip 除去・両面 trim・非同軸エラー伝播の配線が完了した後に、本 Issue のテスト群（tessellation・manifold 検証・Euler-Poincaré・ループ幾何・決定性・YAML golden）を追加してパイプライン全体を hardening する。

## スコープ

| テスト ID | 内容 | 完了判定 |
|---|---|---|
| T18 | A2 結果の `tessellate_solid` が `Ok`、三角形数 > 0 | `cargo test` pass |
| T20 | A2 結果の `validate_manifold` が `Ok(())` | `cargo test` pass |
| T33 | A2 で `V - E + F - L_inner = 2`（seam 考慮） | `cargo test` pass |
| T34 | `intersect_surfaces` 返り値で `loops[0].center.z < loops[1].center.z`、両 `normal == (0,0,1)`、両 `t_range == [0.0, 2*PI]` | `cargo test` pass |
| T22b (A2) | A2 を angular_segments=8 と 64 で build → `assert_solids_equal_with_names` で全 entity name 一致 | `cargo test` pass |
| T23 | `examples/boolean_intersect_cyl_sphere.mycad` を YAML round-trip → byte-identical | `cargo test` pass |

## スコープ外

- A2/A4b（基本動作確認）→ #49
- Cyl×Sph の Cut / Fuse（Phase 4 完了条件は他ペアでカバー済み）
- 非軸整列 cyl×sph（MVP 外、reject のまま）
- ビューア目視確認（#35）

## 依存

- **#49**（Cyl×Sph Intersect partition trim/エラー伝播 配線）— **blocked until merged**
- #38/#41/#42/#44（全 closed）

## 完了後の後処理

- 本 Issue close 後、#34（親）を close。
