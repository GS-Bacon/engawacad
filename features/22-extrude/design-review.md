issues:
  - id: R01
    severity: high
    section: "テスト計画 > ワークスペース回帰"
    finding: "`extrude` を成功機能に変えるのに、既存 `crates/mycad-api/tests/mesh_api.rs` の T07/T18 と `crates/mycad-api/tests/fixtures/extrude.mycad` はまだ『unsupported extrude』前提のまま。計画どおり実装しても `cargo test --workspace` / `cargo xtask ci` が落ちる。"
    suggestion: "`mesh_api.rs` を extrude 成功系へ更新し、fixture も `create_sketch` + `extrude` 形へ差し替えること。"
  - id: R02
    severity: high
    section: "API・型設計 > TS/Schema"
    finding: "`Feature` に `CreateSketch` を足すと `crates/xtask/src/main.rs` の `FEATURE_GOLDEN` と `web/src/generated/Feature.ts` は必ず変わる。影響ファイルに `xtask` / generated TS が入っておらず、TS golden / drift 対応が未固定なので `cargo xtask ci` を満たせない。"
    suggestion: "`crates/xtask/src/main.rs` の golden を更新し、`cargo xtask gen-ts` の出力差分を影響ファイルとして明示すること。"
  - id: R03
    severity: medium
    section: "退化幾何 > 数値境界"
    finding: "`|signed_area| <= 面積公差` と自己交差拒否は書かれているが、その公差の定義元と境界規則が未記載。非隣接辺の端点接触、重複頂点、ほぼ共線の交差をどう扱うかが実装者依存になる。"
    suggestion: "`LENGTH_TOLERANCE` からの導出式か専用定数を明記し、segment intersection の epsilon と shared-endpoint の扱いまで仕様化すること。"
  - id: R04
    severity: medium
    section: "アーキテクチャ整合性 > Feature History"
    finding: "設計本文は `features` を Vec 順に走査するとしているが、テスト計画に forward reference 禁止がない。後続 `CreateSketch` を参照する `Extrude` を誤って許す実装に後退しても、現行 T10 では検出しきれない。"
    suggestion: "`extrude` が後続 sketch を参照した場合は `SketchNotFound` になる専用回帰テストを追加し、順序依存を仕様として固定すること。"

verdict: fail