missing_refs:
  - ADR-007: `component` を将来 object 候補に含める以上、既存の Component 階層・参照モデルを踏まえた CLI スコープ設計として参照すべき
  - ADR-008: CLI CRUD も committed な Feature 書き込み面なので、`--dry-run`/出力契約は Phase 6 の書き込み経路の先例に結び付けておくべき
  - ADR-010: `sketch` を object 候補に挙げるなら、既存の sketch / RefPlane / `CreateSketch` モデルの先例を参照すべき
  - ADR-014: accepted 本文が「Phase 9 履歴 CRUD で UX 課題化し得る」と明言しており、child component 前提の CLI 拡張余地を無視しないため参照すべき
conflicts:
  - ROADMAP Phase 1/2: draft は既存 CLI を `run` / `convert` と扱っているが、完成済みの公開 surface は `view` / `export`。後方互換の例外対象と mixed-UX の評価前提が誤っている
suggestions:
  - `run` / `convert` の記述を `view` / `export` に置換し、例外対象と混在コストの議論を現行 CLI に合わせて書き直す
  - Related か「既存 ADR との関係」に ADR-007 / ADR-008 / ADR-010 / ADR-014 を追加する
  - `--feature-id` が Phase 9 では `root_component` 限定なのか、将来 `component-path` 系の補助指定と組み合わせるのかを明記する
  - 2 階層上限は「Phase 9 の既定値」と書き、将来の `sketch constraint` / `assembly mate` 系を blanket に縛らない但し書きを入れる
  - `rollback` と unsuppress (`restore` / `unsuppress`) を本規約でどう命名するかを本文例か Matrix に追加する
  - `--dry-run` と `--json` が mutated `.engawa` 全文を返すのか、要約だけ返すのかを固定する
  - 外部 CLI 先例の説明を修正する (`git` / `cargo` は verb-first)
verdict: misaligned