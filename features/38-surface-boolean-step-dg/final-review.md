issues:
  - id: F01
    severity: high
    file: "docs/decisions/005-topological-naming.md"
    line_hint: 240
    finding: "「後続実装 issue への影響」表が `#22 / 専用 issue` や匿名の issue 名のままで、今回本文で明示した `#27` / `#31` 分割に追随していない。`EntityRef` enum 化や load path validation の委譲先が誤ったままで、先送り事項の追跡先を一意に辿れない。"
    suggestion: "表の担当 issue を実際の後続先に更新し、少なくとも `EntityRef` enum 化・validation 系は `#27`、pcurve/数値モデル系は `#31` に揃えること。将来 issue 未作成の項目は「Phase 着手時に issue 化」と明記すること。"
  - id: F02
    severity: medium
    file: "docs/decisions/005-topological-naming.md"
    line_hint: 111
    finding: "`Phase 4 実装詳細は Issue #27 を参照。` が残っており、下部で追加した `#31` への分割と整合していない。本文の相互参照が片側だけ旧状態のままになっている。"
    suggestion: "`#27` 単独参照を `#27 / #31` へ更新するか、`#27` が担当する範囲だけを明示した文に書き換えること。"
  - id: F03
    severity: medium
    file: "docs/decisions/005-topological-naming.md"
    line_hint: 281
    finding: "`Phase 5 へ延期` の項目は deferral であることは読めるが、実在 issue へのリンクも、ADR-002/ADR-003 で使っているような「着手時に issue 化する」旨の明示も無く、引き継ぎ先の記録として弱い。"
    suggestion: "Phase 5 の実在 issue を参照するか、少なくとも `Issue 化は Phase 5 着手時` と追記して deferred item の扱いを既存 ADR の運用に合わせること。"

verdict: fail