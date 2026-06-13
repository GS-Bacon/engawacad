# ADR-009 抜粋 (本 Issue #143 が参照する箇所)

> 本 Issue は ADR-009 §Implementation Outline Phase 3 の即時実施分の片方 (死に分岐削除)。
> Phase 3 の残り (共有境界の直接比較テスト追加) は別 Issue で対応する。

## Context (本 Issue が問題視する構造)

> `tessellation/mod.rs:649-680` の `adj_is_sphere` 分岐は両腕とも `arcs_per_rev` を返す**死に分岐**として痕跡が残っている (#131 試行錯誤跡)。

## Decision (採用案 A の含意)

> 案 A を採用する: 曲面同士の Boolean 交線円を 1 円 = 1 周期エッジ (または分断点で分けた少数の弧) として B-rep に保持し、弦への離散化はテッセレーション層に遅延する。

→ ADR-009 Phase 2 で `tessellation/mod.rs` の `n_u` ヒューリスティクス全体 (`arcs_per_rev` / `adj_is_sphere` 等) を撤去する予定。

## Implementation Outline Phase 3 — 即時起票分

> 即時起票: Implementation Outline の Phase 3 のうち、ADR 待ち不要な「死に分岐削除 + 共有境界の直接比較テスト追加」のみ。Phase 7 foundation 作業として消化可能。

本 Issue #143 はこのうち「死に分岐削除」のみ。「共有境界の直接比較テスト追加」は別 Issue。

## 本 Issue で守るべき ADR-009 整合性

1. `n_u` ヒューリスティクス本体 (`arcs_per_rev` 計算と if/else) は撤去しない (Phase 2 の責務)。
2. `adjacent_face_idx` 関数本体は撤去しない (Phase 2 で再利用候補)。本 Issue では `#[allow(dead_code)]` を付与し、関数とテストを温存する。
3. ADR-009 で問題視されている「両側が独立に同じサンプル列を再導出する規約」自体は本 Issue で構造的に解消しない (Phase 1/2 の責務)。本 Issue は読解コスト軽減のみ。
4. `Curve::Circle` / `IntersectionLoop` / `circle_curve_for_edge` / `partition.rs:11` の `ANGULAR_SEGMENTS_DEFAULT` 周辺は触らない (Phase 1 の責務)。
