issues:
  - id: F01
    severity: high
    file: "crates/mycad-format/src/document.rs"
    line_hint: 111
    finding: "`validate_component` は `ComponentRef::File(\"\")` と `ComponentRef::StdLib(\"\")` しか拒否していないため、公開 enum を直接組み立てた `ComponentRef::File(\"stdlib://...\")` を `Document::to_yaml()` が通してしまう。出力 YAML は再読込時に `StdLib` へ化けるか、`stdlib://` 単体なら読込エラーになり、フォーマット層の round-trip が壊れる。"
    suggestion: "`reference.to_string().parse::<ComponentRef>()` で再検証するか、`File` variant 側でも `stdlib://` 接頭辞を明示的に拒否して、`to_yaml()` が wire-format とメモリ表現の不整合を出力しないようにする。"
  - id: F02
    severity: medium
    file: "crates/mycad-format/src/feature.rs"
    line_hint: 27
    finding: "`EntityRef` の入力制約は `validate()` / `Deserialize` にしか掛かっておらず、公開 variant を直接構築した invalid 値を `Serialize` と `canonical_name()` がそのまま受け入れる。結果として、この crate 自身が再読込不能な wire データを出力でき、区切り文字を含む未検証値では `canonical_name()` の一意性前提も崩れる。"
    suggestion: "公開 constructor で常に検証するか、`Serialize` 側でも検証を強制し、`canonical_name()` では未検証文字列を拒否またはエスケープして不変条件を API で保証する。"

verdict: fail