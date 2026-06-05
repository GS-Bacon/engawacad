# debug-spec for Issue #48 core implementation

## 仮説
CI 失敗原因は `cargo fmt --check` によるフォーマット違反のみ。コンパイル・テストはパス済み。
GLM が生成したコードの一部がフォーマット基準（`cargo fmt --all`）を満たしていない。

## 関連ファイル
- `crates/mycad-api/src/error.rs:32` — `InvalidPosition` のマッチアームが複数行に展開されているが、1 行フォームが期待される
- `crates/mycad-build/src/lib.rs:3` — import 順が `use` の並び順規約（アルファベット順）と異なる
- `crates/mycad-build/src/lib.rs:145` — `make_sphere(...)` 呼び出しが複数行展開されているが、短い 1 行フォームが期待される可能性

## 修正方針
`cargo fmt --all` を実行するのみで全修正可能。GLM は以下を実行すること:
1. `cargo fmt --all` — フォーマット修正
2. `cargo xtask ci` — CI グリーン確認

## 試した修正と結果
- [ ] GLM run 1: make_cylinder/make_sphere の引数追加 + FormatError::InvalidPosition 追加 → コンパイル通過、fmt 失敗
- [ ] GLM run 2: mycad-api/src/error.rs の InvalidPosition arm 追加 → fmt 違反のみ残存

## 次にやること
`cargo fmt --all` → `cargo xtask ci` で green を確認。

## 追加で書いてほしいテスト
なし（STEP 6 コアのみ、テストは STEP 6.6 で対応）。
