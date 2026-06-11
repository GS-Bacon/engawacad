<!-- STEP 7.5 Codex review の各ラウンドにおける採用・棄却を記録 -->

## STEP 7.5 Round 1 (codex-final.yaml)

- **F01 (high) 採用**: `isAllTodoIgnoreStub` のロジック弱点 (全 fn 数チェック、属性挿入、メッセージ付き macro) → ロジック全面修正 + 回帰テスト 6 件追加。コミット `f4b9a12`。

## STEP 7.5 Round 2 (codex-final-r2.yaml)

- **F01 (high) 部分採用**: 「STEP 5.5 acceptance skeleton を hard-fail させる」懸念について、本 guard は /3ai STEP 8 文脈での厳格判定が設計意図 (その時点では skeleton 実装完了が正常) と明記 + `--allow-skeleton` オプトアウトフラグ追加。コミット `04d46c9`。
- **F02 (medium) 採用**: `#[tokio::test]` / `async fn` 対応。属性 regex を `#[(?:\\w+::)*test]` に、fn を `(async )? fn` に拡張。回帰テスト 4 件追加。同コミット。

## STEP 7.5 Round 3 (codex-final-r3.yaml)

- **F01 (high) 棄却**: Codex は `extrude_negative_direction_acceptance.rs` の削除で「T01〜T04 が diff 内で 1 件も置き換えられていない」と指摘するが、これは事実誤認。削除した 4 関数 (`t01_kernel_neg_depth_determinism` / `t02_kernel_neg_depth_manifold` / `t_boundary_zero_depth_rejected` / `t_degen_nonfinite_depth_rejected`) は `crates/mycad-kernel/src/primitives/extrusion.rs:783-866` のインラインに**同名同ロジックで全 4 件完全実装済み** (#110 で実装、本コミット時点で稼働中)。Codex が見た diff は `tests/` 配下のみで `src/` 配下のインライン実装を参照できなかったため発生した誤判定。テストカバレッジは構造的に保全されている。
- **F02 (medium) 採用**: マクロ delimiter 形式 `todo!{}` / `todo![]` 対応。正規表現を `(?:\\(.*\\)|\\{.*\\}|\\[.*\\])` に拡張。string-literal 内のコメント記号は known limitation として docstring に明記。回帰テスト 3 件追加。
