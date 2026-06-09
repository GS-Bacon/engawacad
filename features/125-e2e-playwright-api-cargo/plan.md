## In-Scope / Out-of-Scope
| In-Scope | Out-of-Scope |
|----------|--------------|
| `setupPageWithServer()` ヘルパー関数の追加（モックなし実サーバ接続） | 新規テストシナリオの実装 |
| viewer.spec.ts の T01 を `setupPageWithServer()` 使用に更新 | CI/GitHub Actions への組み込み |
| `cargo xtask acceptance` を Playwright 実行も含むよう拡張 | タイル動画・fuzz テスト（別 Issue） |
| 既存 `setupPageWithFixture()` の互換性維持 | API ポート変更 |

## Non-Goals
- 新規テストケースの追加（T01 リファクタのみ）
- CI パイプライン組み込み
- API ポート変更（playwright.config.ts の webServer 設定は既存のまま）

## 実装対象
<!-- Issue: #125 -->
影響ファイル:
- `web/tests/helpers.ts`: 新規作成 — `setupPageWithServer()` と共通ユーティリティをエクスポート
- `web/tests/viewer.spec.ts`: T01 を `setupPageWithServer()` 使用に更新（既存 T01 はそのまま残し T01_server を追加）
- `crates/xtask/src/main.rs`: `acceptance()` に Playwright 実行を追加

注記: `playwright.config.ts` は既に webServer 設定済み（変更不要）。

## 設計方針
### setupPageWithServer()
- `setupPageWithFixture()` と同じシグネチャ: `(page: Page) => Promise<string[]>`
- `page.route()` 呼び出しなし（実 API サーバに全リクエストが届く）
- コンソールエラー収集のみ行う

### T01_server
- T01 "Console no error - simple_box" の実サーバ版
- `setupPageWithServer()` → `page.goto("/")` → canvas 確認 → コンソールエラーなし
- 実サーバは `examples/simple_box.mycad` を返すので simple_box 用テストが通る

### cargo xtask acceptance 拡張
- 既存: Rust acceptance tests (`post_features_acceptance`)
- 追加: `npx playwright test` を `web/` ディレクトリで実行
- Node.js がない場合は警告してスキップ（ローカルのみ対象）
- `--fuzz` フラグは Rust fuzz tests のみ（Playwright は常時実行）

**既存関数の修正:**
`crates/xtask/src/main.rs` の `acceptance()` 関数に Playwright 実行ブロックを追加:

Before:
```rust
fn acceptance() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(2).collect();
    let fuzz = args.iter().any(|a| a == "--fuzz");
    // ... cargo test post_features_acceptance
    // ... if fuzz { cargo test fuzz_features }
    println!("\n=== Acceptance tests passed ===");
    ExitCode::SUCCESS
}
```

After:
```rust
fn acceptance() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(2).collect();
    let fuzz = args.iter().any(|a| a == "--fuzz");
    // ... cargo test post_features_acceptance (既存)
    // ... if fuzz { cargo test fuzz_features } (既存)
    // NEW: Playwright E2E
    println!("\n=== Running Playwright E2E tests ===");
    if which("npx").is_some() {
        // npx playwright test --project chromium
    } else {
        eprintln!("WARNING: npx not found, skipping Playwright tests");
    }
    println!("\n=== Acceptance tests passed ===");
    ExitCode::SUCCESS
}
```

## テスト計画（ID 付き）
| ID | 種別 | 内容 | 期待結果 |
|----|------|------|----------|
| T01 | 決定性 | `setupPageWithServer()` → goto("/") → canvas visible, 0 console errors | consoleErrors.length == 0 |
| T02_degen_no_route_mock | 退化/境界 | helpers.ts の setupPageWithServer が page.route() を呼ばないこと | コード検査 (TS 型検査で確認) |
| T03_degen_acceptance_exit0 | 退化/境界 | cargo xtask acceptance が exit 0 で完了 | exit code 0 |

## 幾何的不変条件チェックリスト
- N/A（テスト基盤 Issue）
- N/A
- N/A
- N/A
