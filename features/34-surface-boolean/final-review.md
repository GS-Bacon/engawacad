issues:
  - id: F01
    severity: high
    file: "docs/decisions/004-freeform-geometry-commitment.md"
    line_hint: 89
    finding: "段階移行プランが `#34` を `Vertex` / `Edge` / `Face` への `tolerance` field 埋め込みと既存 call site 移行の委譲先としているが、実体の `features/34-surface-boolean/plan.md` では同作業を Non-Goal とし『別 Issue』へ再委譲している。deferral の記録先が一致しておらず、後続 issue へのトレーサビリティが壊れている。"
    suggestion: "ADR-004 の段階移行プランを `#34` の実スコープに合わせて更新し、`tolerance` field 埋め込みと完全移行を受ける具体的な後続 issue 番号を明記すること"
  - id: F02
    severity: medium
    file: "docs/decisions/004-freeform-geometry-commitment.md"
    line_hint: 30
    finding: "`[下の節]` の内部リンクアンカーが見出し `### 数値モデル: トレラント方式採用 (Decision 3 — Phase 4 (#31) で確定)` から生成される ID と一致しておらず、リンク先が壊れている。"
    suggestion: "実際の見出し ID に合わせてアンカーを修正するか、明示的なアンカーを追加して内部リンクを固定すること"

verdict: fail