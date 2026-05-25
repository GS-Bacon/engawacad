issues:
  - id: F01
    severity: critical
    file: "crates/mycad-api/src/lib.rs"
    line_hint: 4
    finding: "`pub mod transport;` と `pub use transport::{...}` を追加している一方で、`crates/mycad-api/src/transport/*.rs` はまだ untracked で `git diff` に含まれていない。差分だけを適用すると `transport` モジュール不在でビルド不能になる。"
    suggestion: "`crates/mycad-api/src/transport/mod.rs`・`mesh_request.rs`・`error_response.rs` を `git add` して差分に含める。"
  - id: F02
    severity: high
    file: "crates/xtask/src/main.rs"
    line_hint: 104
    finding: "TS drift check は tracked な `web/src/generated/*` が 1 つも無いと丸ごと skip するが、現状その生成物自体が untracked のままなので、T06/T07 の成果物なしで CI が green になってしまう。"
    suggestion: "`cargo xtask gen-ts` の出力を `git add web/src/generated/*.ts` で差分に含め、bootstrap 後は committed 生成物に対して drift を常時検証する。"
  - id: F03
    severity: medium
    file: "crates/mycad-format/src/document.rs"
    line_hint: 173
    finding: "`test_ts_derive_backward_compat` の exact golden は `simple_box.mycad` だけで、`cylinder.mycad` と `assembly.mycad` は変更後 YAML の roundtrip (`yaml1 == yaml2`) しか見ていない。これでは T08 の『examples/*.mycad の wire YAML が不変』を満たせず、他 fixture の serde 回帰を取りこぼす。"
    suggestion: "各 example について `to_yaml()` を fixture 本文または個別 golden と比較し、全 `.mycad` で byte-identical を固定する。"
  - id: F04
    severity: medium
    file: "crates/xtask/src/main.rs"
    line_hint: 315
    finding: "`t06_gen_ts_produces_all_files` は `gen_ts()` / `cargo xtask gen-ts` を通さず temp dir への直接 export しか確認していない。`workspace_root()`、`web/src/generated` の掃除、実 CLI 経路が壊れてもこのテスト群では検出できない。"
    suggestion: "`gen_ts()` の出力先を注入可能にして直接テストするか、`cargo xtask gen-ts` を叩く統合テストを追加する。"

verdict: fail