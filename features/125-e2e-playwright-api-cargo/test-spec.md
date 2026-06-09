# Test Spec — #125 e2e-playwright-api-cargo

## 実装済みテスト

| ID | 実装 | ファイル |
|----|------|---------|
| T01 | ✅ `setupPageWithServer()` → goto("/") → canvas → 0 errors | `web/tests/viewer.spec.ts` |
| T02_degen_no_route_mock | ✅ helpers.ts に page.route() なし（コード構造で保証） | `web/tests/helpers.ts` |
| T03_degen_acceptance_exit0 | ✅ cargo xtask acceptance に Playwright 追加済み | `crates/xtask/src/main.rs` |

## エッジケース・退化入力

- Node.js がない場合は WARNING でスキップ（exitcode 0 維持）
- `setupPageWithServer()` はモックなしのため、実 API が落ちていると canvas が現れず timeout で fail

## 決定性

web テストのため Rust 決定性は非該当。Playwright テスト自体は実 API サーバのデータに依存する。
