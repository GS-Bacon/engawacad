<!-- Round ごとに以下の形式で追記すること -->

## Round 1

- SC01 (scope/medium): 棄却 → Issue 本文の ADR-014→ADR-015 参照誤り。plan は既に ADR-015 を参照済み、Issue 本文修正は scope 外 (rejection.md §Round 1)。
- invariant: issues none (verdict=pass)
- ambig: issues none (verdict=pass)

Severity: critical=0, high=0, medium=1 (棄却 1), low=0

## Round 2

- IN01 (invariant/critical): 棄却 → engawa-format 層に IdGenerator は無い (kernel 概念)。Variable.name はユーザー定義文字列で ID 生成しない。verdict も pass で finding 内容と矛盾 (false positive)。rejection.md §Round 2 参照。
- scope: issues none (verdict=pass)
- ambig: issues none (verdict=pass)

Severity (round 2): critical=1 (棄却 1, false positive), high=0, medium=0, low=0
