# debug-spec (test mode) — Issue #48

## 仮説
CI 失敗原因は `cargo fmt --check` によるフォーマット違反のみ。
`position_params_acceptance.rs:573` で `Feature::CreateSphere { id, radius, center }` のパターンが複数行展開されているが 1 行フォームが期待される。

## 関連ファイル
- `crates/mycad-build/tests/position_params_acceptance.rs:573` — match arm が複数行

## 修正方針
`cargo fmt --all` を実行するのみで全修正可能。以下を実行すること:
1. `cargo fmt --all`
2. `cargo xtask ci`

## 試した修正と結果
- [ ] GLM test run 1: テスト実装完了・fmt 違反のみ残存

## 次にやること
`cargo fmt --all` → `cargo xtask ci` で green 確認 → commit。
