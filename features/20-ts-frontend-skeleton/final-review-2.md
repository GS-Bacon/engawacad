issues:
  - id: F01
    severity: critical
    file: "crates/xtask/src/main.rs"
    line_hint: 138
    finding: "`ci()` が `web/package-lock.json` と `npm ci`/`vitest`/`vite build` を必須化している一方、今回の diff では `web/package.json`・`web/package-lock.json`・`web/index.html`・`web/src/*.ts`・`web/tsconfig.json`・`web/vite.config.ts` が untracked のままで含まれていない。diff 単体では変更セットが自己完結しておらず、クリーン checkout で `cargo xtask ci` を実行すると web ステップが即 failure になる。"
    suggestion: "web フロント一式を同じ変更セットに含めてから `xtask ci` を更新するか、web チェック導入を feature #20 のコミットに分離する。"
  - id: F02
    severity: medium
    file: "crates/mycad-format/src/document.rs"
    line_hint: 146
    finding: "`test_ts_derive_backward_compat` は全 `examples/*.mycad` を roundtrip (`yaml1 == yaml2`) で見るだけで、byte-identical な golden 比較は `simple_box.mycad` にしか無い。設計レビューで合意した T08 の『examples/*.mycad の wire YAML 固定』をまだ満たしておらず、`cylinder.mycad` と `assembly.mycad` の serialize 回帰を取りこぼす。"
    suggestion: "`cylinder.mycad` と `assembly.mycad` も fixture 本文または個別 golden と `assert_eq!` し、全 example を byte-identical に固定する。"
  - id: F03
    severity: medium
    file: "crates/xtask/src/main.rs"
    line_hint: 364
    finding: "`t06_gen_ts_produces_all_files` は `export_to_temp_dir()` を直接呼んで一時ディレクトリの内容だけを見ており、実際の `gen_ts()` / `cargo xtask gen-ts` 経路を通していない。`workspace_root()` 解決、`web/src/generated/` の掃除、実出力先への書き込みが壊れても T06 は green のままになる。"
    suggestion: "`gen_ts()` の出力先を注入可能にして本体を直接テストするか、`cargo xtask gen-ts` を実行する統合テストを追加する。"
  - id: F04
    severity: medium
    file: "web/src/mesh.ts"
    line_hint: 11
    finding: "`validateMesh()` は有限値と index 範囲しか見ておらず、coincident vertices / zero-area triangle / zero-length edge を検出しない。実際に `web/src/mesh.test.ts:171` では全頂点一致の退化三角形を `not.toThrow()` として固定しており、レビュー観点の『退化幾何は検出してエラー化』に反している。"
    suggestion: "各三角形について重複頂点と面積ゼロを検査して `MeshValidationError` を返し、対応テストも reject 期待に更新する。"

verdict: fail