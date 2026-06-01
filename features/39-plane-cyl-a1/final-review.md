issues:
  - id: F01
    severity: high
    file: "docs/decisions/"
    line_hint: 1
    finding: "レビュー対象の `git diff` に `docs/decisions` 配下の ADR 追加・更新が含まれておらず、宣言した docs/ADR 成果物を確認できない"
    suggestion: "既存体系に合わせて `docs/decisions/006-*.md` などの ADR を差分に含め、少なくとも `## Status` / `## Context` / `## Decision` / 代替案または委譲先 / 影響を明記し、関連 issue・関連 ADR・deferral の記録もその ADR から辿れる形で追加すること"

verdict: fail