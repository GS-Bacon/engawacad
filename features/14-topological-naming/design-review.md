issues:
  - id: R06
    severity: high
    section: "設計方針 > Decision 2 / 6"
    finding: "`feature_id` 単独を identity anchor にしているが、既存 format には `Component` 階層と外部 `ref` があり、子コンポーネント配下や同一部品の複数 occurrence を `<feature_id>;<kind>:<role>` だけでは一意に識別できない。後で component/occurrence path を足すと、将来 seed にすると明言した stable name を書き換えることになる。"
    suggestion: "ADR でスコープを『単一 Component 内の参照のみ』と明示して assembly/occurrence 越し参照を禁止するか、今のうちに `(component/occurrence path, feature_id, kind, role)` を stable name / `EntityRef` に含めること。子 Component・複数インスタンスの受け入れテストも予約すること。"
  - id: R07
    severity: high
    section: "設計方針 > Decision 2 / 6、Implementation Details / 影響範囲"
    finding: "`FormatError` で `Feature.id` 重複や charset 違反を弾く方針だが、現行公開 API は `Document::from_yaml() -> Result<_, serde_yaml::Error>` で、`serde_yaml::from_str` 直呼びも可能。検証をどこに差し込むか未定のままだと、命名の前提不変条件が簡単にバイパスされる。"
    suggestion: "`mycad-format` に単一の validated load path を定義し、`from_yaml` を `FormatError` 化するか `Document::validate()` を必須で呼ぶ契約を ADR に明記すること。`from_path` / `from_yaml` / 直接 deserialize の各経路で同じ validation が効くテストを追加すること。"
  - id: R08
    severity: high
    section: "設計方針 > Decision 5 / 将来方針の記録"
    finding: "『要素マップ + 履歴ハッシュ』を最終形として採る一方、複数の source name から 1 つの derived name を作るときの canonical ordering が未定義。Boolean/fillet で親エンティティ集合の列挙順や `HashMap` 反復順が seed に混ざると、同一入力でも履歴ハッシュ名が変わる。"
    suggestion: "今の ADR で『derived hash の入力は stable name を canonical order に正規化してから連結/ハッシュする』『unordered container を seed に直接使わない』を必須化すること。Phase 4 Issue に multi-parent seed 決定性テストを明記すること。"
  - id: R09
    severity: high
    section: "検証計画 > 後続実装 issue に予約する acceptance tests"
    finding: "Decision 10 では退化入力を `KernelError` にすると決めているのに、acceptance は『`KernelError` または name 集合に不在』になっている。これだとゼロ長エッジや面積ゼロ面を含む不正 B-rep を成功として通し、名前だけ付けない実装が許される。"
    suggestion: "退化入力の期待結果は `KernelError` に統一し、そのうえで失敗時に name が生成されないことを別 assertion に分けること。`validate_manifold()` / Euler 検証が退化成功経路を許さないことも確認すること。"
  - id: R10
    severity: medium
    section: "検証計画 > 後続実装 issue に予約する acceptance tests"
    finding: "canonical grammar と charset を決めたのに、負例テストが `Feature.id` 重複しかない。`feature_id` / `role` / sketch element id に `;` `:` や許可外文字が混入した場合、missing `kind` や不正 grammar が来た場合の rejection が未テスト。"
    suggestion: "`FormatError` 負例として reserved separator、許可外文字、空 segment、missing `kind` を個別に固定すること。sketch element id についても #22 の acceptance に同等の grammar validation を入れること。"
  - id: R11
    severity: medium
    section: "Implementation Details / 影響範囲、検証計画"
    finding: "`EntityRef` は `JsonSchema` / `TS` 生成対象だが、計画の後続テストは YAML golden roundtrip しか見ていない。`kind` 追加や enum 化で `EntityRef.ts` / schema が壊れても、web/API 契約の回帰を拾えない。"
    suggestion: "後続 issue の acceptance に `EntityRef` の schema/TS golden を追加し、`JsonSchema, TS` derive 維持を ADR に明記すること。YAML だけでなく生成された `EntityRef.ts` の exact 比較も行うこと。"

verdict: fail