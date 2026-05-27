issues:
  - id: F01
    severity: high
    file: "docs/decisions/005-topological-naming.md"
    line_hint: 67
    finding: "F02 はデシリアライズ経路の封鎖しか扱っておらず、現在の `Document` / `Component` が持つ公開可変フィールド経由の不正状態注入を塞げていない。`Document::new()` 後に重複 `Feature.id` や不正 charset を追加できるため、『構築済みの `Document` は常に valid』という不変条件をこの方針だけでは保証できない。"
    suggestion: "validated load path だけでなく、`Document` / `Component` のフィールド非公開化と検証付き builder/setter まで ADR に含め、未検証状態を公開 API から完全に排除する。"
  - id: F02
    severity: medium
    file: "docs/decisions/005-topological-naming.md"
    line_hint: 112
    finding: "`EntityRef::Named { feature_id, kind, role }` という構造化表現と、`<feature_id>;<kind>:<role>` という単一文字列 grammar のどちらを `.mycad` / TS / JsonSchema の wire format にするかが未確定。T04/T05 の golden を固定できず、後続実装ごとに互換性のない表現が出る。"
    suggestion: "on-disk 表現を ADR で 1 つに固定する。文字列 grammar を内部 canonical name としてのみ使うならその旨を明記し、serde/TS/schema は構造化表現に統一する。"
  - id: F03
    severity: medium
    file: "docs/decisions/005-topological-naming.md"
    line_hint: 254
    finding: "T11 が `from_path` / `from_yaml` / direct deserialize の 3 経路すべてで validation 発火を必須化しており、F02 で許容している『raw/validated を分けて unvalidated 型を公開しない』案と整合していない。validated newtype 案では direct deserialize 経路自体を消すのが正解になる。"
    suggestion: "T11 は『公開 API から未検証 `Document` を取得できないこと』を検証対象に書き換えるか、逆に custom `Deserialize` 案へ設計を固定する。"

verdict: fail