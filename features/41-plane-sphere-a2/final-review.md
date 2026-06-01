issues:
  - id: F01
    severity: high
    file: "docs/decisions"
    line_hint: 1
    finding: "`git diff` に `docs/decisions/*.md` の追加・更新がなく、Issue #41 向けの正式 ADR が差分に存在しない。既存 ADR 運用に沿った Status・背景/決定/代替案/影響・関連 issue / 関連 ADR・deferral 記録を確認できない"
    suggestion: "既存採番に従う正式 ADR を `docs/decisions/` に追加するか既存 ADR を更新し、#41・関連 ADR・先送り事項の後続 issue を相互リンクしたうえで差分に含めること"
  - id: F02
    severity: medium
    file: "features/41-plane-sphere-a2/plan.md"
    line_hint: 3
    finding: "`Non-Goals` の先送り事項が『次 Issue (A2.1 相当)』『A4で対応』のような曖昧な表現に留まっており、後続 issue 番号/URL が特定できない"
    suggestion: "各 deferral を実在する issue 番号または URL に置き換え、正式 ADR からも相互リンクして追跡可能にすること"
  - id: F03
    severity: medium
    file: "features/41-plane-sphere-a2/plan.md"
    line_hint: 25
    finding: "`plan.md` では `crates/mycad-build/tests/surface_boolean_a2_acceptance.rs` を `#[ignore]` スケルトン扱いしている一方、`test-spec.md` では同ファイルに T01-T13 が実装済み・passing と記載しており、最終成果物の状態説明が不一致"
    suggestion: "最終状態に合わせて `plan.md` の将来形/スケルトン記述を削除し、テスト実装状況の記述を `test-spec.md` と一致させること"

verdict: fail