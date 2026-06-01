<!-- Round ごとに以下の形式で追記すること -->
<!-- ## Round N -->
<!-- - R0X: 「<指摘の要点>」を棄却 — <理由（scope 外、Non-Goals に記載済み、次 issue で対応 等）> -->

## Final Review Round (F01 rejection)

**F01** (high): Missing ADR in `docs/decisions/` for Issue #41

**棄却理由**: 既存 ADR (001-005) は B-rep 表現・ロードマップ管理・ビューア・幾何など「プロジェクト横断の主要アーキテクチャ判断」を対象としている。Issue #41 は Phase 4 の Boolean サブ Issue であり、個別 feature 実装は ADR の採番対象外。設計判断は `features/41-plane-sphere-a2/plan.md` に詳細を記録済み。実装前の design-review (Codex ラウンド 1) も通過。
