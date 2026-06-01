# Debug Spec — Issue #31 (GLM attempt 3 failure)

## 仮説

`crates/mycad-build/tests/feature_dispatcher.rs:1501` で E0716 が発生している。
`result2.unwrap()` の戻り値が一時値として即 drop される一方、`.all()[0].solid` への borrow がその後も続くため lifetime が合わない。
rustc の提案通り `let` バインディングに分割すれば解決する。

## 関連ファイル

- `crates/mycad-build/tests/feature_dispatcher.rs:1498-1510` — 問題箇所
  ```
  let solid2 = &result2.unwrap().all()[0].solid;   // ← ここが E0716
  ```

## 修正方針

該当箇所を以下のように 2 行に分割する（コンパイラの help 通り）:

```rust
// 変更前
let solid2 = &result2.unwrap().all()[0].solid;

// 変更後
let binding2 = result2.unwrap();
let solid2 = &binding2.all()[0].solid;
```

変数名は `binding2` (同テスト内の他の変数名と衝突しなければ `owned2` でも可)。

他に同様のパターンが同ファイルに存在する場合は同じ修正を適用すること。

## 追加で書いてほしいテスト

なし — E0716 はコンパイルエラーであり、テストの内容ではなく構文の問題。修正後 `cargo xtask ci` が green になれば OK。
