# タイトル: Cylinder×Sphere Boolean A3: Intersect の partition trim/エラー伝播 配線 (Phase 4 #34 sub-step)

## 現在の Phase と完了条件への寄与

**現在の active Phase は Phase 4**。完了条件: `Cut / Fuse / Intersect が .mycad から動作する`。

本 Issue は「Cylinder × Sphere 同軸配置での Intersect が Feature レベルで動作する」ことを A2/A4b で確認し、Phase 4 完了条件の最後の形状ペアを埋める。

## 概要

曲面 Boolean の最後の未配線セル。幾何コア `intersect_cylinder_sphere`（`surface_intersect.rs:251-337`）は実装済みだが、Boolean パイプラインは cyl×sph 面ペアを `partition.rs:298,787` の `continue` で **skip** しており、`Feature::Intersect(cyl,sph)` を通しても交線が PSLG に投入されず面が trim されない。本 Issue でその skip を除去し、曲面×曲面の交線投入・両面 trim・非同軸エラー伝播を配線して A2/A4b を緑にする。

## スコープ

| 作業内容 | 完了判定 |
|---|---|
| `partition.rs:298,787` の cyl×sph skip `continue` 除去 | A2 pass |
| cyl×sph 交線 2 円の PSLG 投入（cylinder lateral + sphere の両 UV へ） | A2 で両面 inner loop 生成 |
| trimmed cylinder/sphere face の pcurve 後付け（既存 `attach_pcurves_for_trimmed_faces` 流用） | `validate_manifold` Ok |
| 非同軸時の `UnsupportedSurfaceIntersection` を boolean エラーとして伝播（cyl×sph ペア限定。他ペアの `else { continue }` は維持） | A4b pass |

## スコープ外

- 統合テスト hardening（T18/T20/T33/T34/T22b/T23）→ #43
- Cyl×Sph の Cut / Fuse（Phase 4 完了条件は他ペアでカバー済み）
- 非軸整列 cyl×sph（axis≠±Z、MVP 外、reject のまま）
- ビューア目視確認（#35）
- ADR-004 への追記（別途）

## Acceptance tests

- **A2**: sphere center=(0,0,0) r=5 ∩ cylinder axis=+Z origin=(0,0,-10) r=3 h=20 の `Feature::Intersect` → 結果 manifold、z=±4 に 2 個の交線 Circle、cylinder lateral / sphere face が trim される。
- **A4b**: sphere center=(1,0,0) と cylinder axis=+Z（非同軸）の Intersect → `Err(UnsupportedSurfaceIntersection { reason: "non-coaxial cylinder × sphere" })` が boolean から伝播。

## 数値モデル

- tolerance: `Tolerance::DEFAULT`（`LENGTH_TOLERANCE = 1e-9`）
- 同軸/tangent 判定は幾何コア既存ロジックに委譲（`r_sq = sph_r²−cyl_r²` の符号、tangent は `Ok(vec![])`）
- ADR-004 準拠: tolerant 方式継続（#31 で確立）

## ADR-006 §1 粒度チェック

- [x] 1 軸 × 1 op（Cyl×Sph × Intersect）に収まるか
- [x] ADR 決定と実装が混在していないか（ADR 参照のみ）
- [x] 完了条件が計測可能か（A2/A4b の `cargo test` pass）
- [x] 前提 Issue が closed か（#38/#41/#42/#44 全 closed）

## 依存

- #38/#41/#42/#44（全 closed）
- trim タイル化 `tessellate_sphere_face_trimmed`（#41）/ cylinder trim（#39）を再利用
- `partition.rs:298,787` の skip は `// deferred to #39` コメント付きだが #39 でも先送りされた未解消コード

## 完了後の後処理

- 本 Issue close 後、#43（hardening: T18/T20/T33/T34/T22b/T23）が unblock される
- #43 も close したら #34（親 Issue）を close
