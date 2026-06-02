issues:
  - id: F01
    severity: high
    file: "docs/decisions"
    line_hint: 1
    finding: "`git diff` に `docs/decisions/*.md` の追加・更新がなく、Issue #42 向けの正式 ADR が差分に存在しない。既存 ADR 規約の `Status` / `Context` / `Decision` / `Rationale` / `Implementation Details` と、要求された背景・決定・代替案・影響・関連 issue / 関連 ADR・deferral 記録を確認できない"
    suggestion: "既存採番に従う正式 ADR を `docs/decisions/` に追加するか既存 ADR を更新し、#42・関連 ADR・先送り事項の後続 issue を相互リンクしたうえで差分に含めること"
  - id: F02
    severity: medium
    file: "features/42-a1-1-plane-cyl-fuse-isect/plan.md"
    line_hint: 3
    finding: "`Non-Goals` の先送り事項が『A2.1 として別 Issue』『A3/A4 として別 Issue』のような曖昧な表現に留まり、具体的な issue 番号/URL が無いため deferral の追跡性と実在確認ができない"
    suggestion: "各先送り事項を実在する issue 番号または URL に置き換え、正式 ADR からも相互リンクして委譲先を固定すること"
  - id: F03
    severity: medium
    file: "features/42-a1-1-plane-cyl-fuse-isect/plan.md"
    line_hint: 20
    finding: "`plan.md` は `crates/mycad-build/tests/plane_cyl_fuse_isect_acceptance.rs` の `#[ignore]` スケルトン追加・T01-T11・既存コード無変更を前提にしている一方、`test-spec.md` は `crates/mycad-build/tests/a1_1_plane_cyl_fuse_isect_acceptance.rs` に T01-T14 実装済みと記載しており、現行 `git diff` も kernel 修正を含む。最終成果物の説明が文書間で一致していない"
    suggestion: "最終状態に合わせて `plan.md` と `test-spec.md` を更新し、実際のファイル名・テスト件数・コード修正有無の記述を揃えること"

verdict: fail