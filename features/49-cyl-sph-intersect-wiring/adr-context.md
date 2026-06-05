# ADR 抜粋（#49 設計レビュー用）

## ADR-004 §数値モデル: トレラント方式採用（Phase 4 #31 で確定）
- Phase 4 は per-entity トレラント方式。比較は両端 Tolerance の max。
- グローバル定数 `LENGTH_TOLERANCE = 1e-9 (mm)`（距離・座標の絶対比較）、`ANGLE_TOLERANCE`、`RELATIVE_TOLERANCE` は `Tolerance::DEFAULT` として残る。
- #34（曲面 Boolean MVP）: Plane×Cylinder / Plane×Sphere / **Cylinder×Sphere の軸整列 Boolean**。per-entity tolerance field 埋め込みは Non-Goal（別 Issue / Phase 5 候補）。
- 一括公差差し替えは決定性回帰リスクが高いため新規コード経路から順次移行。本 Issue は新規 cyl×sph 経路 → `Tolerance::DEFAULT` / `LENGTH_TOLERANCE` 直接使用で可。

## ADR-006 §1 粒度・§2 設計実装分離
- 本 Issue は 1 軸×1 op（Cyl×Sph × Intersect）。ADR 変更なし（参照のみ）。
- 「Issue 本文の処方（attach_pcurves_for_trimmed_faces 流用 / 両面 inner loop）」は実コードと不一致 → plan.md で正確化済み（円柱=帯/球=キャップ、2 円トリム新規経路）。
