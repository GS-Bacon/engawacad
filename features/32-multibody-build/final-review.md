issues:
  - id: F01
    severity: medium
    file: "crates/xtask/src/main.rs"
    line_hint: 428
    finding: "新規公開 DTO の `BodyMesh.ts` が 100-run drift 比較と exact golden の両方から漏れており、合意済み T15 が未実装です。現状の CI は `BodyMesh.ts` の存在確認しかしていないため、ts-rs 出力が変わっても API 契約の退行を検出できません。"
    suggestion: "`BodyMesh.ts` を `reference_files` に追加し、`ErrorResponse.ts` と同様の exact golden テストも追加する"
  - id: F02
    severity: low
    file: "crates/mycad-cli/src/main.rs"
    line_hint: 74
    finding: "assembly/reference 文書を fail-closed で拒否する新規分岐に対する CLI 側テストが追加されておらず、合意済み T16 が未実装です。条件式や呼び出し経路が崩れて部分 STL を再び出力する回帰が入っても、この差分のテスト群では検出できません。"
    suggestion: "既存の `assembly.mycad` fixture を使って `export` がエラーになることを確認する CLI テストを追加する"

verdict: pass