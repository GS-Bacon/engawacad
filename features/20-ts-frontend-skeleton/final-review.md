issues:
  - id: F01
    severity: critical
    file: "crates/xtask/src/main.rs"
    line_hint: 138
    finding: "`ci()` が `web/package-lock.json` を必須化し、その後 `npm ci` / `vitest` / `vite build` まで実行するようになっているが、現時点の差分には `web/package.json`・`web/package-lock.json`・`web/index.html`・`web/src/main.ts`・`web/tsconfig.json`・`web/vite.config.ts` が含まれていない（untracked）。この diff だけを適用すると、Node が入っている環境では `cargo xtask ci` が必ず壊れる。"
    suggestion: "フロントの最小一式を同じ差分に含めるか、この変更から web チェックを外して #20 側でまとめて導入する。"
  - id: F02
    severity: high
    file: "crates/xtask/src/main.rs"
    line_hint: 133
    finding: "`which(\"node\")` の結果で web チェック全体を丸ごと skip しており、同じコミットでも `node` が PATH にある環境では失敗し、無い環境では成功する。`xtask ci` の検証内容がホスト依存になっており、決定的な CI ゲートになっていない。"
    suggestion: "Node/npm/npx を `xtask ci` の前提として明示し、欠けていれば fail-fast するか、web チェックを別タスクに分離して条件付き skip をやめる。"
  - id: F03
    severity: medium
    file: "crates/mycad-format/src/document.rs"
    line_hint: 173
    finding: "`test_ts_derive_backward_compat` は `examples/*.mycad` 全体を roundtrip (`yaml1 == yaml2`) で見るだけで、exact golden 比較は `simple_box.mycad` にしか無い。設計時に合意した T08 の『examples/*.mycad の wire YAML を固定する』要件を満たしておらず、`cylinder.mycad` や `assembly.mycad` の serialize 回帰を取りこぼす。"
    suggestion: "各 example ごとに `to_yaml()` を fixture 本文または個別 golden と `assert_eq!` し、roundtrip ではなく byte-identical を固定する。"
  - id: F04
    severity: medium
    file: "crates/xtask/src/main.rs"
    line_hint: 362
    finding: "`t06_gen_ts_produces_all_files` は `export_to_temp_dir()` を直接呼ぶだけで、実際の `gen_ts()` / `cargo xtask gen-ts` 経路を通していない。`workspace_root()` の解決、`web/src/generated/` の掃除、実出力先への書き込みが壊れても T06 は green のままになる。"
    suggestion: "`gen_ts()` の出力先を注入可能にして本体を直接テストするか、`xtask gen-ts` を実行する統合テストを追加する。"

verdict: fail