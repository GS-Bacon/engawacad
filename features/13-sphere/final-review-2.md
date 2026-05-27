issues:
  - id: F01
    severity: high
    file: "crates/mycad-kernel/src/primitives/mod.rs"
    line_hint: 3
    finding: "`mod sphere;` と sphere 向けの build/API/CLI テスト参照は tracked diff に入っていますが、依存する `crates/mycad-kernel/src/primitives/sphere.rs`・`examples/sphere.mycad`・`crates/mycad-api/tests/fixtures/extrude.mycad` は現状 untracked です。`git diff` 相当の差分だけを適用/commit すると、モジュール解決と fixture 読み込みが壊れて CI が落ちます。"
    suggestion: "新規 `sphere.rs`・`sphere.mycad`・`extrude.mycad` を同じ commit に含め、review 対象 diff を自己完結させる。"
  - id: F02
    severity: medium
    file: "crates/mycad-kernel/src/brep/topology.rs"
    line_hint: 224
    finding: "`validate_manifold()` は `lp.half_edges[i]` を直接 index して outer loop しか検証していません。loop 内の out-of-bounds half-edge / start_vertex では `Err` ではなく panic し、inner loop の破損は素通りします。新しく追加した topology validator として不完全です。"
    suggestion: "outer/inner loop の両方について half-edge・edge・vertex index を先に検証し、壊れた topology でも panic せず `Err` を返すようにする。"
  - id: F03
    severity: medium
    file: "crates/mycad-format/src/document.rs"
    line_hint: 173
    finding: "新規 `examples/sphere.mycad` / `create_sphere` の wire YAML は exact golden で固定されておらず、現状は全 example に対する `yaml1 == yaml2` roundtrip と `simple_box.mycad` だけの golden 比較しかありません。serializer/schema drift が起きても sphere のフォーマット回帰を検出できません。"
    suggestion: "`examples/sphere.mycad` か `Feature::CreateSphere` について、`to_yaml()` を fixture 本文または個別 golden と byte-identical 比較するテストを追加する。"
  - id: F04
    severity: medium
    file: "crates/mycad-kernel/src/primitives/sphere.rs"
    line_hint: 84
    finding: "T01 の決定性テストが `Solid` 全体を固定できていません。現在比較しているのは vertex/edge/face の一部だけで、`solid.id`、half_edges、loops、shells、face の loop/same_sense、edge の curve/t_range などの非決定化は見逃します。最重要観点の決定性テストとして弱いです。"
    suggestion: "`Solid` の全トポロジー配列と関連フィールドを field-by-field で比較し、同一 `IdGenerator` 初期値で完全一致することを検証する。"

verdict: fail