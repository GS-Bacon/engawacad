# Debug Spec — clippy unused variable errors

## 仮説
cargo xtask ci が "FAILED: Running clippy" で落ちている。
フォーマット問題は修正済み（`cargo fmt` 通過）。
残る問題は `crates/mycad-kernel/src/booleans/partition.rs` の unused variable 4 件。

## 関連ファイル
- `crates/mycad-kernel/src/booleans/partition.rs`（行 823-824 と行 1345-1346）

## 修正方針
以下の 4 箇所で変数名を `_` プレフィックスに変える（これのみ）:

1. `partition.rs:823` — `let is_bottom = band == 0;` → `let _is_bottom = band == 0;`
2. `partition.rs:824` — `let is_top = band == boundaries.len() - 2;` → `let _is_top = band == boundaries.len() - 2;`
3. `partition.rs:1345` — `let is_bottom = band == 0;` → `let _is_bottom = band == 0;`
4. `partition.rs:1346` — `let is_top = band == boundaries.len() - 2;` → `let _is_top = band == boundaries.len() - 2;`

他に変更しない。

## 試した修正と結果
- [ ] 上記 4 箇所を `_` プレフィックスに変更

## 次にやること
`cargo xtask ci` を実行して全 green を確認する。

## 追加で書いてほしいテスト
なし（コア実装は完了済み）。
