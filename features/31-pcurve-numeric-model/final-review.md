issues:
  - id: F01
    severity: high
    file: "docs/decisions/004-freeform-geometry-commitment.md"
    line_hint: 87
    finding: "`#34` を後続 issue として明示しているが、repo 内の issue 記録（`features/34-*/plan.md` 相当）が存在せず、ADR-002 の運用では deferral の引き継ぎ先を辿れない"
    suggestion: "`#34` の実体を先に用意し、内容を示す issue/plan 記録を追加したうえで ADR から明示的に参照できる形にすること"
  - id: F02
    severity: medium
    file: "docs/decisions/004-freeform-geometry-commitment.md"
    line_hint: 88
    finding: "`| 以降 | 完全移行 | ... |` が匿名プレースホルダのままで、最終移行の委譲先が具体的な issue に結び付いていない"
    suggestion: "完全移行を受ける具体的な issue 番号に置き換えるか、issue 未作成ならこの行を外して作成時に追記すること"
  - id: F03
    severity: medium
    file: "docs/decisions/004-freeform-geometry-commitment.md"
    line_hint: 30
    finding: "`#31` の plan では ADR-004 に「トレラント方式採用」と「段階移行プラン」の節を置く前提だが、現行差分では前者が Decision 箇条書き内に埋まっており独立した節として参照できない"
    suggestion: "`### トレラント方式採用` の節を立てて Decision 3 の確定内容を移すか、plan/T21 の期待値を同じ差分で現行表記に合わせて更新すること"

verdict: fail