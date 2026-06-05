<!-- Round ごとに以下の形式で追記すること -->
<!-- ## Round N -->
<!-- - R0X: 「<指摘の要点>」を棄却 — <理由（scope 外、Non-Goals に記載済み、次 issue で対応 等）> -->

## Round 1-2 scope 防衛メモ (full-adoption-warning 対応)
両 round とも全 4 ペルソナ (scope/invariant/ambig/numeric) が issue 0 件で pass。
棄却 0 は「全採用」ではなく「指摘ゼロ」に由来する。
scope 防衛状況:
- plan に Non-Goals 5 項目 (axis / Translate / Component.transform / rotation / ADR-005追記) を明記済み。
- scope ペルソナが 2 round 連続 pass し、In-Scope/Out-of-Scope 表の妥当性を確認。
- 本 Issue は既存フィールド (Surface.origin/center) への配線 + serde default 後方互換という限定スコープ。
